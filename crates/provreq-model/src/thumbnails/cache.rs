//! On-disk thumbnail cache (Phase 5c).
//!
//! Layout per `UX-uploadPreview`:
//!
//! ```text
//! <root>/<first-two-hex-of-hash>/<full-hash>/512.png
//! ```
//!
//! The shard directory prevents any single parent from
//! accumulating every entry, which matters on filesystems whose
//! readdir cost grows with directory size. LRU eviction reads each
//! entry's mtime (we `utime()` on lookup to keep reads touching
//! "recent") and drops oldest-first until the cache is under its
//! byte budget.

use std::io;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::SystemTime;

use thiserror::Error;

/// Fixed filename under the content-hash directory. Encodes the
/// 512 px longest-edge contract: if we ever support multiple
/// sizes we'd branch here and key the cache on (hash, size).
pub const THUMBNAIL_FILE_NAME: &str = "512.png";

/// Default cache ceiling when the env var is unset (500 MB).
pub const DEFAULT_CACHE_MAX_BYTES: u64 = 500 * 1024 * 1024;

#[derive(Debug, Clone)]
pub struct ThumbnailCacheConfig {
    pub root: PathBuf,
    pub max_bytes: u64,
}

/// Shared on-disk thumbnail cache handle. Cheap to clone; the
/// eviction lock is the only shared mutable state.
#[derive(Clone)]
pub struct ThumbnailCache {
    inner: Arc<ThumbnailCacheInner>,
}

impl std::fmt::Debug for ThumbnailCache {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ThumbnailCache")
            .field("root", &self.inner.config.root)
            .field("max_bytes", &self.inner.config.max_bytes)
            .finish()
    }
}

struct ThumbnailCacheInner {
    config: ThumbnailCacheConfig,
    /// Serialises evictions. Not held across generation.
    eviction_lock: Mutex<()>,
}

impl ThumbnailCache {
    pub fn new(config: ThumbnailCacheConfig) -> Self {
        Self {
            inner: Arc::new(ThumbnailCacheInner {
                config,
                eviction_lock: Mutex::new(()),
            }),
        }
    }

    pub fn root(&self) -> &Path {
        &self.inner.config.root
    }

    pub fn max_bytes(&self) -> u64 {
        self.inner.config.max_bytes
    }

    /// Absolute path where the thumbnail for `content_hash` lives.
    /// Stable regardless of whether the file exists.
    pub fn path_for(&self, content_hash: &str) -> PathBuf {
        let shard = content_hash.get(..2).unwrap_or("00");
        self.inner
            .config
            .root
            .join(shard)
            .join(content_hash)
            .join(THUMBNAIL_FILE_NAME)
    }

    /// Return the cached thumbnail path if it already exists on
    /// disk. Bumps the entry's mtime so LRU eviction sees it as
    /// recently used; failure to bump is non-fatal.
    pub fn lookup(&self, content_hash: &str) -> Option<PathBuf> {
        let path = self.path_for(content_hash);
        if !path.is_file() {
            return None;
        }
        let now = SystemTime::now();
        let _ = set_file_mtime(&path, now);
        Some(path)
    }

    /// Prepare the target directory for a write. Creates the
    /// parents and evicts enough entries to stay under
    /// `max_bytes` after the new file arrives. Callers then write
    /// the PNG at `path_for(content_hash)` directly.
    pub fn prepare_slot(&self, target: &Path) -> Result<(), CacheError> {
        if let Some(parent) = target.parent() {
            std::fs::create_dir_all(parent).map_err(|source| CacheError::Io {
                path: parent.to_path_buf(),
                source,
            })?;
        }
        self.maybe_evict()?;
        Ok(())
    }

    /// Walk the cache, sum bytes, and drop oldest entries until
    /// usage is ≤ `max_bytes`. Never panics — per-entry read
    /// errors are logged and skipped.
    pub fn maybe_evict(&self) -> Result<(), CacheError> {
        let _guard = self
            .inner
            .eviction_lock
            .lock()
            .map_err(|_| CacheError::LockPoisoned)?;
        let mut entries = collect_entries(self.inner.config.root.as_path())?;
        let total: u64 = entries.iter().map(|e| e.byte_size).sum();
        if total <= self.inner.config.max_bytes {
            return Ok(());
        }
        entries.sort_by_key(|e| e.mtime);
        let mut remaining = total;
        for entry in entries {
            if remaining <= self.inner.config.max_bytes {
                break;
            }
            match std::fs::remove_file(&entry.path) {
                Ok(()) => {
                    remaining = remaining.saturating_sub(entry.byte_size);
                    let _ = prune_empty_shard(&entry.path);
                }
                Err(err) => {
                    tracing::warn!(
                        path = %entry.path.display(),
                        error = %err,
                        "thumbnail cache eviction failed to remove entry"
                    );
                }
            }
        }
        Ok(())
    }

    /// Total on-disk bytes — useful in tests and in operator
    /// diagnostics. Skips unreadable entries silently.
    pub fn total_bytes(&self) -> u64 {
        collect_entries(self.inner.config.root.as_path())
            .map(|entries| entries.iter().map(|e| e.byte_size).sum())
            .unwrap_or(0)
    }
}

#[derive(Debug)]
struct CacheEntry {
    path: PathBuf,
    byte_size: u64,
    mtime: SystemTime,
}

fn collect_entries(root: &Path) -> Result<Vec<CacheEntry>, CacheError> {
    let mut entries = Vec::new();
    if !root.is_dir() {
        return Ok(entries);
    }
    let shards = std::fs::read_dir(root).map_err(|source| CacheError::Io {
        path: root.to_path_buf(),
        source,
    })?;
    for shard in shards.flatten() {
        let shard_path = shard.path();
        if !shard_path.is_dir() {
            continue;
        }
        let hash_dirs = match std::fs::read_dir(&shard_path) {
            Ok(d) => d,
            Err(err) => {
                tracing::warn!(
                    path = %shard_path.display(),
                    error = %err,
                    "thumbnail cache shard unreadable"
                );
                continue;
            }
        };
        for hash_dir in hash_dirs.flatten() {
            let file = hash_dir.path().join(THUMBNAIL_FILE_NAME);
            let meta = match std::fs::metadata(&file) {
                Ok(m) => m,
                Err(_) => continue,
            };
            if !meta.is_file() {
                continue;
            }
            let mtime = meta.modified().unwrap_or(SystemTime::UNIX_EPOCH);
            entries.push(CacheEntry {
                path: file,
                byte_size: meta.len(),
                mtime,
            });
        }
    }
    Ok(entries)
}

/// Remove the content-hash dir and its parent shard if they're
/// empty after we drop the entry. Non-fatal — stray empty dirs
/// cost nothing except cosmetic tidiness.
fn prune_empty_shard(file: &Path) -> io::Result<()> {
    let hash_dir = match file.parent() {
        Some(p) => p,
        None => return Ok(()),
    };
    match std::fs::read_dir(hash_dir)?.next() {
        None => std::fs::remove_dir(hash_dir)?,
        Some(_) => return Ok(()),
    }
    if let Some(shard) = hash_dir.parent()
        && std::fs::read_dir(shard)?.next().is_none()
    {
        std::fs::remove_dir(shard)?;
    }
    Ok(())
}

fn set_file_mtime(path: &Path, when: SystemTime) -> io::Result<()> {
    let file = std::fs::File::options().write(true).open(path)?;
    file.set_modified(when)
}

#[derive(Debug, Error)]
pub enum CacheError {
    #[error("i/o error touching {}: {source}", path.display())]
    Io {
        path: PathBuf,
        #[source]
        source: io::Error,
    },

    #[error("thumbnail cache eviction lock was poisoned")]
    LockPoisoned,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread::sleep;
    use std::time::Duration;
    use tempfile::tempdir;

    fn write_png(cache: &ThumbnailCache, hash: &str, bytes: &[u8]) -> PathBuf {
        let path = cache.path_for(hash);
        cache.prepare_slot(&path).unwrap();
        std::fs::write(&path, bytes).unwrap();
        path
    }

    #[test]
    fn path_for_shards_by_first_two_hex() {
        let temp = tempdir().unwrap();
        let cache = ThumbnailCache::new(ThumbnailCacheConfig {
            root: temp.path().to_path_buf(),
            max_bytes: 1024,
        });
        let path = cache.path_for("abcdef0123");
        assert!(path.starts_with(temp.path().join("ab").join("abcdef0123")));
        assert!(path.ends_with("512.png"));
    }

    #[test]
    fn short_hashes_fall_back_to_shard_00() {
        let temp = tempdir().unwrap();
        let cache = ThumbnailCache::new(ThumbnailCacheConfig {
            root: temp.path().to_path_buf(),
            max_bytes: 1024,
        });
        // A pathological caller with a 1-char hash still gets a
        // valid path instead of a panic on the `get(..2)` slice.
        let path = cache.path_for("a");
        assert!(path.starts_with(temp.path().join("00")));
    }

    #[test]
    fn lookup_returns_none_when_file_absent() {
        let temp = tempdir().unwrap();
        let cache = ThumbnailCache::new(ThumbnailCacheConfig {
            root: temp.path().to_path_buf(),
            max_bytes: 1024,
        });
        assert!(cache.lookup("missing").is_none());
    }

    #[test]
    fn lookup_returns_path_after_write() {
        let temp = tempdir().unwrap();
        let cache = ThumbnailCache::new(ThumbnailCacheConfig {
            root: temp.path().to_path_buf(),
            max_bytes: 1024,
        });
        let path = write_png(&cache, "deadbeef0", b"\x89PNG");
        assert_eq!(cache.lookup("deadbeef0"), Some(path));
    }

    #[test]
    fn eviction_drops_oldest_entries_until_under_cap() {
        let temp = tempdir().unwrap();
        // Cap: 8 bytes — the fourth write must evict the first.
        let cache = ThumbnailCache::new(ThumbnailCacheConfig {
            root: temp.path().to_path_buf(),
            max_bytes: 8,
        });
        for (idx, hash) in ["aaaaaa", "bbbbbb", "cccccc", "dddddd"]
            .into_iter()
            .enumerate()
        {
            write_png(&cache, hash, &[0u8; 4]);
            if idx < 3 {
                // Force distinct mtimes — file-system granularity
                // is usually 1 s which is too coarse for a fast
                // test; we nudge each entry a hair.
                sleep(Duration::from_millis(20));
            }
        }
        cache.maybe_evict().unwrap();
        assert!(cache.total_bytes() <= 8);
        assert!(cache.lookup("aaaaaa").is_none(), "oldest should be gone");
        assert!(cache.lookup("dddddd").is_some(), "newest should remain");
    }

    #[test]
    fn lookup_bumps_mtime_so_recently_used_survives_eviction() {
        let temp = tempdir().unwrap();
        let cache = ThumbnailCache::new(ThumbnailCacheConfig {
            root: temp.path().to_path_buf(),
            max_bytes: 8,
        });
        write_png(&cache, "aaaaaa", &[0u8; 4]);
        sleep(Duration::from_millis(50));
        write_png(&cache, "bbbbbb", &[0u8; 4]);
        sleep(Duration::from_millis(50));
        // Touch the older entry so it's the *newest* LRU timestamp.
        assert!(cache.lookup("aaaaaa").is_some());
        sleep(Duration::from_millis(50));
        write_png(&cache, "cccccc", &[0u8; 4]);
        cache.maybe_evict().unwrap();
        assert!(
            cache.lookup("aaaaaa").is_some(),
            "touched entry must survive eviction"
        );
        assert!(
            cache.lookup("bbbbbb").is_none(),
            "untouched older entry must be evicted"
        );
    }

    #[test]
    fn prepare_slot_is_idempotent() {
        let temp = tempdir().unwrap();
        let cache = ThumbnailCache::new(ThumbnailCacheConfig {
            root: temp.path().to_path_buf(),
            max_bytes: 1024,
        });
        let path = cache.path_for("deadbeef");
        cache.prepare_slot(&path).unwrap();
        cache.prepare_slot(&path).unwrap();
    }
}
