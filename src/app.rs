//! Application state shared across HTTP handlers.
//!
//! `AppState` is the single `Arc<AppState>` cloned into every axum
//! handler. It holds:
//!
//! - a `ready` flag that flips from `false` to `true` once mount
//!   discovery finishes (see `readyz`);
//! - a slot holding the current `Arc<World>` (mounts, UUID index,
//!   duplicates, loaded System config, missing-mount reconciliation);
//! - the [`DiscoveryConfig`] used to rediscover on CRUD writes and
//!   on polling-watcher rescans;
//! - the [`OwnershipOverrides`] used to chown written files.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use serde::Serialize;
use tokio::sync::{RwLock, broadcast};

use reqforge_model::git_history::RepoCache;
use reqforge_model::llm::LlmRuntime;
use reqforge_model::thumbnails::{ThumbnailCache, ThumbnailCacheConfig, ThumbnailRegistry};
use reqforge_model::urls::UrlCheckClient;
use std::path::PathBuf;

use reqforge_model::world::{DiscoveryConfig, DiscoveryError, discover_single, run_discovery};
use reqforge_model::write::OwnershipOverrides;

// `World` is the model's view, absorbed into reqforge-model in arc-1 (#348). The application layer
// (AppState, handlers) refers to it as `crate::app::World`, so re-export it here rather than
// redefining it.
pub use reqforge_model::world::World;

/// Change notifications broadcast to SSE subscribers. Phase 2 has
/// a single variant — the world was replaced, clients should
/// re-fetch whatever they care about — since the frontend drives
/// rendering off react-query caches which it can invalidate en
/// masse. Later phases may subdivide.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase", tag = "kind")]
pub enum ChangeEvent {
    WorldChanged,
}

/// Shared server state wrapped in an `Arc<AppState>` by callers.
#[derive(Debug)]
pub struct AppState {
    ready: AtomicBool,
    world: RwLock<Option<Arc<World>>>,
    config: DiscoveryConfig,
    overrides: OwnershipOverrides,
    events: broadcast::Sender<ChangeEvent>,
    /// Reviewer identities submitted during the current container
    /// lifetime. Kept separate from the persisted `reviewers.json`
    /// so a restart still shows authoritative persisted entries
    /// even if the file on disk is out of sync.
    session_identities: RwLock<Vec<String>>,
    /// HTTP client used by the URL-check handler. One connection
    /// pool per process, wrapped here so handlers reuse it.
    url_check_client: UrlCheckClient,
    /// Process-wide gitoxide repo-handle cache. Lazy-opened per
    /// project mount on first history request (Phase 5d); evicted
    /// when a mount disappears on `refresh()`.
    repo_cache: RepoCache,
    /// Thumbnail registry. `None` when no workspace dir is
    /// configured — the `GET /thumbnail` endpoint then returns a
    /// structured 404 pointing at the missing workspace. `Some`
    /// with an empty provider list is the normal dev-machine case
    /// (libvips/libreoffice absent) and also surfaces the same 404
    /// but for "no-thumbnailer-for-format" reasons.
    thumbnail_registry: Option<ThumbnailRegistry>,
    /// Phase 8: latest doorstop import report per project
    /// slug, held in memory only (persisted-to-disk reports
    /// are out of scope per the Phase 8 locked decision). A
    /// server restart loses the cached report.
    doorstop_reports:
        RwLock<std::collections::HashMap<String, Arc<reqforge_model::doorstop::ImportReport>>>,
    /// Phase 10a: LLM adapter runtime — built once at first
    /// `publish` from `SystemConfig.llm`, then stable for
    /// the process lifetime. `None` when the System carries
    /// no `llm` block (LLM-optional) or when the server has
    /// not yet completed initial discovery. Rebuilding on
    /// config-change would lose health/backoff state, so we
    /// require restart for LLM reconfig.
    llm_runtime: RwLock<Option<Arc<LlmRuntime>>>,
    /// provreq single-subject mode (#370): the one repository this process serves. When set,
    /// discovery yields a one-mount World (`discover_single`) instead of scanning
    /// `config.mount_prefix` for sibling projects, and proof handlers read the subject from here.
    /// `None` restores ReqForge's multi-project scan.
    subject_root: Option<PathBuf>,
}

impl AppState {
    pub fn new(config: DiscoveryConfig, overrides: OwnershipOverrides) -> Self {
        Self::build(config, overrides, None)
    }

    /// provreq single-subject constructor: discovery and proof both key off `subject_root`.
    pub fn new_single_subject(
        subject_root: PathBuf,
        config: DiscoveryConfig,
        overrides: OwnershipOverrides,
    ) -> Self {
        Self::build(config, overrides, Some(subject_root))
    }

    fn build(
        config: DiscoveryConfig,
        overrides: OwnershipOverrides,
        subject_root: Option<PathBuf>,
    ) -> Self {
        let (tx, _rx) = broadcast::channel(32);
        let thumbnail_registry = config.workspace_dir.as_ref().map(|workspace| {
            let cache = ThumbnailCache::new(ThumbnailCacheConfig {
                root: workspace.join("thumbnail-cache"),
                max_bytes: config.thumbnail_cache_max_bytes,
            });
            let registry = ThumbnailRegistry::probe_and_build(cache);
            tracing::info!(
                providers = ?registry.provider_names(),
                "thumbnail registry ready"
            );
            registry
        });
        Self {
            ready: AtomicBool::new(false),
            world: RwLock::new(None),
            config,
            overrides,
            events: tx,
            session_identities: RwLock::new(Vec::new()),
            url_check_client: UrlCheckClient::new(10),
            repo_cache: RepoCache::new(),
            thumbnail_registry,
            doorstop_reports: RwLock::new(std::collections::HashMap::new()),
            llm_runtime: RwLock::new(None),
            subject_root,
        }
    }

    /// The single subject this process serves, if in provreq single-subject mode. Proof handlers
    /// (verify/engines/triage) read the subject filesystem directly through this.
    pub fn subject(&self) -> Option<&std::path::Path> {
        self.subject_root.as_deref()
    }

    /// Shared gitoxide repo-handle cache. Cloned on every access
    /// — the underlying `DashMap` already handles the sharing.
    pub fn repo_cache(&self) -> &RepoCache {
        &self.repo_cache
    }

    /// Cache the latest doorstop import report for a project
    /// slug. Overwrites any previous report so the frontend
    /// always sees the most recent run.
    pub async fn set_doorstop_report(
        &self,
        slug: String,
        report: reqforge_model::doorstop::ImportReport,
    ) {
        self.doorstop_reports
            .write()
            .await
            .insert(slug, Arc::new(report));
    }

    /// Fetch the cached doorstop import report for a project
    /// slug, or `None` if none has been run this process.
    pub async fn get_doorstop_report(
        &self,
        slug: &str,
    ) -> Option<Arc<reqforge_model::doorstop::ImportReport>> {
        self.doorstop_reports.read().await.get(slug).cloned()
    }

    /// Borrow the thumbnail registry if the AppState was
    /// constructed with a workspace directory. The `None` arm is
    /// the signal for the HTTP handler to return a structured 404
    /// pointing at the missing workspace.
    pub fn thumbnail_registry(&self) -> Option<&ThumbnailRegistry> {
        self.thumbnail_registry.as_ref()
    }

    /// Borrow the shared URL-check client. Returned by reference
    /// so callers don't clone the underlying `reqwest::Client`
    /// (which already wraps an `Arc` internally — the reference is
    /// purely an ergonomic nudge).
    pub fn url_check_client(&self) -> &UrlCheckClient {
        &self.url_check_client
    }

    /// Snapshot the list of reviewer identities submitted this
    /// container lifetime. Returned in insertion order (most-recently
    /// added last); callers reverse it when the UX wants
    /// most-recent-first.
    pub async fn session_identities_snapshot(&self) -> Vec<String> {
        self.session_identities.read().await.clone()
    }

    /// Record a reviewer identity as having submitted at least one
    /// review this container lifetime. Deduplicates case-sensitively
    /// (Alice and alice are distinct identities — the identity
    /// scheme is non-authenticating per `REVIEW-reviewerIdentity`,
    /// so we don't second-guess the operator's spelling).
    pub async fn record_session_identity(&self, identity: &str) {
        let trimmed = identity.trim();
        if trimmed.is_empty() {
            return;
        }
        let mut guard = self.session_identities.write().await;
        if guard.iter().any(|existing| existing == trimmed) {
            return;
        }
        guard.push(trimmed.to_owned());
    }

    /// Install a freshly-discovered World, mark the server ready,
    /// and broadcast a `ChangeEvent::WorldChanged`. Safe to call
    /// repeatedly — the old Arc drops when the last outstanding
    /// reader releases it.
    pub async fn publish(&self, world: World) {
        // Evict stale repo handles for mounts that disappeared
        // between the previous World and this one (per the Phase 5d
        // locked decision on gitoxide repo caching).
        let git_dirs: Vec<std::path::PathBuf> = world
            .mounts
            .iter()
            .filter_map(|m| match &m.state {
                reqforge_model::mount::MountState::Project(p) => Some(p.root.join(".git")),
                _ => None,
            })
            .collect();
        self.repo_cache.retain_paths(&git_dirs);
        // Phase 10a: build the LLM runtime once, on the
        // first publish that carries a System config. Later
        // publishes keep the existing runtime so health and
        // ack state persist across refreshes.
        self.maybe_init_llm_runtime(&world).await;
        *self.world.write().await = Some(Arc::new(world));
        self.ready.store(true, Ordering::Release);
        // Send errors are harmless: they only fire when there are
        // no subscribers, which is the common case.
        let _ = self.events.send(ChangeEvent::WorldChanged);
    }

    async fn maybe_init_llm_runtime(&self, world: &World) {
        {
            let guard = self.llm_runtime.read().await;
            if guard.is_some() {
                return;
            }
        }
        let llm_value = world.system.config().and_then(|c| c.llm.as_ref());
        let parsed = match reqforge_model::llm::parse_llm(llm_value) {
            Ok(v) => v,
            Err(err) => {
                tracing::warn!(%err, "SystemConfig.llm is malformed; LLM endpoints will be unavailable");
                return;
            }
        };
        match LlmRuntime::build(parsed) {
            Ok(runtime) => {
                let count = runtime.provider_count();
                *self.llm_runtime.write().await = Some(runtime);
                tracing::info!(providers = count, "LLM runtime initialised");
            }
            Err(err) => {
                tracing::warn!(%err, "failed to build LLM runtime; LLM endpoints will be unavailable");
            }
        }
    }

    /// Cloned `Arc<LlmRuntime>` snapshot, or `None` if the
    /// server has no LLM config or the runtime hasn't been
    /// initialised yet.
    pub async fn llm_runtime(&self) -> Option<Arc<LlmRuntime>> {
        self.llm_runtime.read().await.clone()
    }

    /// Drop the cached LLM runtime so the next discovery cycle
    /// rebuilds it from the (presumably-just-edited) System config.
    /// Phase 13's CRUD endpoints call this immediately before
    /// `refresh()` so a freshly-added or edited provider takes
    /// effect on the next prompt without a server restart.
    /// Health and privacy-ack state for unchanged providers are
    /// reset to defaults — operators editing config implicitly
    /// opt into re-verification.
    pub async fn invalidate_llm_runtime(&self) {
        *self.llm_runtime.write().await = None;
    }

    /// Subscribe to the change-event broadcast. Each SSE client
    /// gets its own receiver.
    pub fn subscribe(&self) -> broadcast::Receiver<ChangeEvent> {
        self.events.subscribe()
    }

    /// Whether `publish` has ever been called. Checked by `readyz`.
    pub fn is_ready(&self) -> bool {
        self.ready.load(Ordering::Acquire)
    }

    /// Return a cheap-to-clone `Arc<World>` snapshot (or `None`
    /// before the first `publish`). The lock is released as soon
    /// as the snapshot is cloned, so handlers can hold the
    /// snapshot across await points without blocking writers.
    pub async fn snapshot(&self) -> Option<Arc<World>> {
        self.world.read().await.clone()
    }

    /// Re-run discovery and swap in the new World. Invoked by
    /// CRUD handlers after a write and by the polling watcher on
    /// every tick.
    pub async fn refresh(&self) -> Result<(), DiscoveryError> {
        let config = self.config.clone();
        let subject = self.subject_root.clone();
        let world = tokio::task::spawn_blocking(move || match subject {
            Some(root) => discover_single(root, &config),
            None => run_discovery(&config),
        })
        .await
        .map_err(|join_err| {
            DiscoveryError::Mount(reqforge_model::mount::MountDiscoveryError::Io {
                path: std::path::PathBuf::new(),
                source: std::io::Error::other(join_err.to_string()),
            })
        })??;
        self.publish(world).await;
        Ok(())
    }

    pub fn config(&self) -> &DiscoveryConfig {
        &self.config
    }

    pub fn overrides(&self) -> OwnershipOverrides {
        self.overrides
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use reqforge_model::index::UuidIndex;
    use reqforge_model::system::LoadedSystem;

    fn fake_config() -> DiscoveryConfig {
        DiscoveryConfig {
            mount_prefix: std::path::PathBuf::from("/nonexistent"),
            system_config_path: None,
            workspace_dir: None,
            max_blob_bytes: 50 * 1024 * 1024,
            thumbnail_cache_max_bytes: 500 * 1024 * 1024,
            external_url: None,
        }
    }

    fn fake_world() -> World {
        World {
            mounts: Vec::new(),
            index: UuidIndex::new(),
            duplicates: Vec::new(),
            system: LoadedSystem::Unnamed,
            missing_project_slugs: Vec::new(),
            link_catalog: Vec::new(),
            search_index: reqforge_model::search::empty_index(),
        }
    }

    #[tokio::test]
    async fn starts_not_ready_with_no_world() {
        let state = AppState::new(fake_config(), OwnershipOverrides::default());
        assert!(!state.is_ready());
        assert!(state.snapshot().await.is_none());
    }

    #[tokio::test]
    async fn publish_flips_ready_and_installs_world() {
        let state = AppState::new(fake_config(), OwnershipOverrides::default());
        state.publish(fake_world()).await;
        assert!(state.is_ready());
        assert!(state.snapshot().await.is_some());
    }

    #[tokio::test]
    async fn session_identities_start_empty_and_dedupe_case_sensitively() {
        let state = AppState::new(fake_config(), OwnershipOverrides::default());
        assert!(state.session_identities_snapshot().await.is_empty());
        state.record_session_identity("alice").await;
        state.record_session_identity("alice").await; // duplicate
        state.record_session_identity("Alice").await; // distinct case
        state.record_session_identity("   ").await; // whitespace-only dropped
        assert_eq!(
            state.session_identities_snapshot().await,
            vec!["alice".to_owned(), "Alice".to_owned()],
        );
    }

    #[tokio::test]
    async fn republishing_replaces_the_current_world() {
        let state = AppState::new(fake_config(), OwnershipOverrides::default());
        state.publish(fake_world()).await;
        let first = state.snapshot().await.unwrap();
        state.publish(fake_world()).await;
        let second = state.snapshot().await.unwrap();
        assert!(!Arc::ptr_eq(&first, &second));
    }
}
