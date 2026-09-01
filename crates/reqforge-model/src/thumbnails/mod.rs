//! Thumbnail pipeline (Phase 5c).
//!
//! Implements: ART006
//!
//! A thumbnail is a 512 px-longest-edge PNG keyed on the source
//! blob's sha256 content hash. Each supported input format is
//! serviced by a [`ThumbnailProvider`]; the registry walks providers
//! in registration order and returns the first one whose
//! [`ThumbnailProvider::accepts`] returns `true` for the media
//! type.
//!
//! The code *always* compiles even when `soffice`, `vips`, and
//! `magick` are absent: providers probe their binary at
//! registration time and only install themselves when the probe
//! succeeds. On a dev machine with none of the tools installed the
//! registry is empty and the Phase 5c `GET /thumbnail` endpoint
//! returns a structured 404 (`no-thumbnailer-for-format`), which
//! is the user-visible signal the frontend renders the
//! icon + size + download fallback for.

pub mod cache;
pub mod libreoffice;
pub mod libvips;

use std::path::{Path, PathBuf};
use std::sync::Arc;

use thiserror::Error;
use tokio::sync::{OnceCell, Semaphore};

pub use cache::{ThumbnailCache, ThumbnailCacheConfig};

/// Output PNG dimension for all providers — longest edge in pixels.
/// Fixed so the cache key is stable (the content hash does not
/// encode the requested dimension).
pub const THUMBNAIL_LONGEST_EDGE_PX: u32 = 512;

/// Global cap on in-flight thumbnail generation processes. Two
/// matches the Phase 5 design decision; larger values can starve
/// a small-container CPU budget when a bulk upload lands.
pub const THUMBNAIL_CONCURRENCY: usize = 2;

/// A single format handler. Implementations *must* be
/// feature-detected — the constructor returns `None` when the
/// underlying tool is missing from `$PATH`.
pub trait ThumbnailProvider: Send + Sync {
    /// Provider name for logging and diagnostics.
    fn name(&self) -> &'static str;

    /// Does this provider generate thumbnails for the given media
    /// type? Called in registration order; the first `true` wins.
    fn accepts(&self, media_type: &str) -> bool;

    /// Produce a 512 px-longest-edge PNG at `output` from `input`.
    /// The caller already holds the concurrency semaphore.
    fn generate(&self, input: &Path, output: &Path) -> Result<(), ThumbnailError>;
}

/// Shared outcome of an in-flight generator. Stored in the
/// coalescing map so that the second caller for the same
/// `content_hash` awaits the same future instead of spawning a
/// duplicate `soffice` / `vips` process.
type InFlightCell = OnceCell<Result<(), String>>;

/// Bag of installed providers plus the concurrency guard and the
/// in-flight coalescer. Cheap to clone — the expensive state lives
/// behind `Arc`s internally.
#[derive(Clone)]
pub struct ThumbnailRegistry {
    providers: Arc<Vec<Arc<dyn ThumbnailProvider>>>,
    semaphore: Arc<Semaphore>,
    in_flight: Arc<dashmap::DashMap<String, Arc<InFlightCell>>>,
    cache: ThumbnailCache,
}

impl std::fmt::Debug for ThumbnailRegistry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ThumbnailRegistry")
            .field(
                "providers",
                &self.providers.iter().map(|p| p.name()).collect::<Vec<_>>(),
            )
            .field("cache", &self.cache)
            .finish()
    }
}

impl ThumbnailRegistry {
    /// Build a registry from a list of providers plus a prepared
    /// cache. Tests wire a vector of mock providers directly; the
    /// production constructor goes through
    /// [`ThumbnailRegistry::probe_and_build`].
    pub fn new(providers: Vec<Arc<dyn ThumbnailProvider>>, cache: ThumbnailCache) -> Self {
        Self {
            providers: Arc::new(providers),
            semaphore: Arc::new(Semaphore::new(THUMBNAIL_CONCURRENCY)),
            in_flight: Arc::new(dashmap::DashMap::new()),
            cache,
        }
    }

    /// Run the usual production probes (libvips → LibreOffice) and
    /// register whichever providers report themselves available.
    /// On a machine without any of the tools installed this returns
    /// an empty registry — the `GET /thumbnail` endpoint then
    /// serves `no-thumbnailer-for-format` uniformly.
    pub fn probe_and_build(cache: ThumbnailCache) -> Self {
        let mut providers: Vec<Arc<dyn ThumbnailProvider>> = Vec::new();
        if let Some(p) = libvips::LibvipsProvider::probe() {
            providers.push(Arc::new(p));
        }
        if let Some(p) = libreoffice::LibreofficeProvider::probe() {
            providers.push(Arc::new(p));
        }
        Self::new(providers, cache)
    }

    pub fn provider_names(&self) -> Vec<&'static str> {
        self.providers.iter().map(|p| p.name()).collect()
    }

    pub fn cache(&self) -> &ThumbnailCache {
        &self.cache
    }

    /// Locate the first installed provider that accepts `media_type`.
    pub fn provider_for(&self, media_type: &str) -> Option<Arc<dyn ThumbnailProvider>> {
        self.providers
            .iter()
            .find(|p| p.accepts(media_type))
            .cloned()
    }

    /// End-to-end thumbnail generation: cache-hit returns the
    /// cached PNG path; cache-miss acquires the concurrency
    /// semaphore, coalesces duplicate in-flight requests for the
    /// same `content_hash`, runs the provider in `spawn_blocking`,
    /// and returns the path of the written PNG.
    pub async fn get_or_generate(
        &self,
        content_hash: &str,
        media_type: &str,
        binary_path: &Path,
    ) -> Result<PathBuf, ThumbnailError> {
        if let Some(hit) = self.cache.lookup(content_hash) {
            return Ok(hit);
        }

        let provider =
            self.provider_for(media_type)
                .ok_or(ThumbnailError::NoProviderForMediaType {
                    media_type: media_type.to_owned(),
                })?;

        let cell = self
            .in_flight
            .entry(content_hash.to_owned())
            .or_insert_with(|| Arc::new(OnceCell::new()))
            .clone();

        let hash = content_hash.to_owned();
        let binary_path = binary_path.to_path_buf();
        let cache = self.cache.clone();
        let semaphore = self.semaphore.clone();
        let outcome = cell
            .get_or_init(|| async move {
                let permit = semaphore.acquire_owned().await.map_err(|e| e.to_string())?;
                let target = cache.path_for(&hash);
                cache.prepare_slot(&target).map_err(|e| e.to_string())?;
                let provider = provider.clone();
                let bin = binary_path.clone();
                let tgt = target.clone();
                let result = tokio::task::spawn_blocking(move || provider.generate(&bin, &tgt))
                    .await
                    .map_err(|e| e.to_string())?;
                drop(permit);
                result.map_err(|e| e.to_string())
            })
            .await
            .clone();

        self.in_flight.remove(content_hash);

        outcome.map_err(ThumbnailError::Generator)?;
        Ok(self.cache.path_for(content_hash))
    }
}

#[derive(Debug, Error)]
pub enum ThumbnailError {
    #[error("no thumbnail provider installed for media type {media_type}")]
    NoProviderForMediaType { media_type: String },

    #[error("thumbnail tool failed: {0}")]
    Generator(String),

    #[error("i/o error preparing thumbnail slot at {}: {source}", path.display())]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("thumbnail tool '{tool}' exited with status {status}: {stderr}")]
    ToolExit {
        tool: &'static str,
        status: i32,
        stderr: String,
    },

    #[error("thumbnail tool '{tool}' timed out after {secs}s")]
    Timeout { tool: &'static str, secs: u64 },
}

/// Run a binary with the given args and a hard timeout. Wraps the
/// common shell-out shape used by every provider so they don't
/// reinvent the timeout + stderr capture.
pub(crate) fn run_tool_with_timeout(
    tool: &'static str,
    args: &[&std::ffi::OsStr],
    timeout: std::time::Duration,
) -> Result<(), ThumbnailError> {
    use std::process::{Command, Stdio};

    let mut child = Command::new(tool)
        .args(args)
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|source| ThumbnailError::Io {
            path: PathBuf::from(tool),
            source,
        })?;

    let deadline = std::time::Instant::now() + timeout;
    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                let code = status.code().unwrap_or(-1);
                if status.success() {
                    return Ok(());
                }
                let mut stderr = String::new();
                if let Some(mut pipe) = child.stderr.take() {
                    use std::io::Read;
                    let _ = pipe.read_to_string(&mut stderr);
                }
                return Err(ThumbnailError::ToolExit {
                    tool,
                    status: code,
                    stderr,
                });
            }
            Ok(None) => {
                if std::time::Instant::now() >= deadline {
                    let _ = child.kill();
                    let _ = child.wait();
                    return Err(ThumbnailError::Timeout {
                        tool,
                        secs: timeout.as_secs(),
                    });
                }
                std::thread::sleep(std::time::Duration::from_millis(50));
            }
            Err(err) => {
                return Err(ThumbnailError::Io {
                    path: PathBuf::from(tool),
                    source: err,
                });
            }
        }
    }
}

/// Probe whether a binary is present on `$PATH`. Used at
/// registration time by each provider.
pub(crate) fn binary_on_path(name: &str) -> bool {
    use std::process::{Command, Stdio};
    Command::new(name)
        .arg("--version")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .stdin(Stdio::null())
        .status()
        .is_ok_and(|s| s.success() || s.code().is_some())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use tempfile::tempdir;

    struct StubProvider {
        name: &'static str,
        accepts: &'static [&'static str],
        calls: Arc<AtomicUsize>,
    }

    impl ThumbnailProvider for StubProvider {
        fn name(&self) -> &'static str {
            self.name
        }

        fn accepts(&self, media_type: &str) -> bool {
            self.accepts.contains(&media_type)
        }

        fn generate(&self, _input: &Path, output: &Path) -> Result<(), ThumbnailError> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            std::fs::write(output, b"\x89PNG\r\n\x1a\nstub").map_err(|source| ThumbnailError::Io {
                path: output.to_path_buf(),
                source,
            })
        }
    }

    fn cache_fixture() -> (tempfile::TempDir, ThumbnailCache) {
        let temp = tempdir().unwrap();
        let cache = ThumbnailCache::new(ThumbnailCacheConfig {
            root: temp.path().to_path_buf(),
            max_bytes: 16 * 1024,
        });
        (temp, cache)
    }

    #[tokio::test]
    async fn no_registered_providers_surfaces_structured_error() {
        let (_guard, cache) = cache_fixture();
        let registry = ThumbnailRegistry::new(Vec::new(), cache);
        let input = tempdir().unwrap();
        let binary = input.path().join("spec.pdf");
        std::fs::write(&binary, b"%PDF-1.4").unwrap();
        let err = registry
            .get_or_generate("deadbeef", "application/pdf", &binary)
            .await
            .unwrap_err();
        assert!(matches!(err, ThumbnailError::NoProviderForMediaType { .. }));
    }

    #[tokio::test]
    async fn provider_runs_once_and_caches_result() {
        let (_guard, cache) = cache_fixture();
        let calls = Arc::new(AtomicUsize::new(0));
        let provider = Arc::new(StubProvider {
            name: "stub",
            accepts: &["application/pdf"],
            calls: calls.clone(),
        });
        let registry = ThumbnailRegistry::new(vec![provider], cache);
        let input = tempdir().unwrap();
        let binary = input.path().join("spec.pdf");
        std::fs::write(&binary, b"%PDF-1.4").unwrap();

        let path_one = registry
            .get_or_generate("hashA", "application/pdf", &binary)
            .await
            .unwrap();
        let path_two = registry
            .get_or_generate("hashA", "application/pdf", &binary)
            .await
            .unwrap();
        assert_eq!(path_one, path_two);
        assert_eq!(calls.load(Ordering::SeqCst), 1);
        assert!(path_one.is_file());
    }

    #[tokio::test]
    async fn registry_picks_first_accepting_provider() {
        let (_guard, cache) = cache_fixture();
        let pdf_calls = Arc::new(AtomicUsize::new(0));
        let png_calls = Arc::new(AtomicUsize::new(0));
        let pdf = Arc::new(StubProvider {
            name: "pdf",
            accepts: &["application/pdf"],
            calls: pdf_calls.clone(),
        });
        let png = Arc::new(StubProvider {
            name: "png",
            accepts: &["image/png"],
            calls: png_calls.clone(),
        });
        let registry = ThumbnailRegistry::new(vec![pdf, png], cache);
        let input = tempdir().unwrap();
        let binary = input.path().join("spec.png");
        std::fs::write(&binary, b"\x89PNG\r\n").unwrap();
        registry
            .get_or_generate("hashB", "image/png", &binary)
            .await
            .unwrap();
        assert_eq!(pdf_calls.load(Ordering::SeqCst), 0);
        assert_eq!(png_calls.load(Ordering::SeqCst), 1);
    }
}
