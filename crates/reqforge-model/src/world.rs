//! The discovery snapshot — `World` — and the pass that builds it.
//!
//! Absorbed from ReqForge (#348): the `World` struct came from its `app.rs` and `run_discovery`
//! from its `discovery.rs`. `AppState` (which wrapped `World` in axum/tokio state — `RwLock`,
//! `broadcast::Sender`, the LLM runtime) is deliberately left behind in ReqForge's server; only the
//! plain discovery snapshot and its synchronous constructor cross into the model, because the
//! reports/search cluster (#331) consumes a `World`, not a running server.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use thiserror::Error;

use crate::index::{DuplicateUuid, UuidIndex, build_uuid_index};
use crate::links::{LinkType, effective_catalog};
use crate::mount::{MountDiscoveryError, MountInfo, MountState, discover_mounts};
use crate::search::{SearchIndex, SearchIndexError};
use crate::system::{LoadedSystem, SystemLoadError, load_system_config, missing_project_slugs};

/// The in-memory view ReqForge builds from discovery. Replaced
/// atomically on every CRUD write and on every polling-watcher
/// rescan; never mutated in place.
#[derive(Debug)]
pub struct World {
    pub mounts: Vec<MountInfo>,
    pub index: UuidIndex,
    pub duplicates: Vec<DuplicateUuid>,
    pub system: LoadedSystem,
    pub missing_project_slugs: Vec<String>,
    /// Effective link-type catalog — built-ins plus System-declared
    /// extras — recomputed on every discovery / refresh so handlers
    /// read one authoritative view. Empty only in tests that don't
    /// exercise catalog-dependent code paths.
    pub link_catalog: Vec<LinkType>,
    /// Phase 7c Tantivy full-text index, rebuilt alongside the
    /// UUID index on every `run_discovery` so the two views
    /// converge on the same snapshot. `Arc`-shared so handlers
    /// can clone cheaply without holding the world's read lock.
    pub search_index: Arc<SearchIndex>,
}

/// Configuration subset relevant to discovery. Read once from the
/// environment and passed through to `AppState` + live writes.
#[derive(Debug, Clone)]
pub struct DiscoveryConfig {
    pub mount_prefix: PathBuf,
    pub system_config_path: Option<PathBuf>,
    /// Operator-local workspace directory per
    /// `DEPLOY-devWorkspace` / `DEPLOY-operatorWorkspace`. Home to
    /// `reviewers.json`, `review-snapshots/`, and similar
    /// runtime-writable state. Unset in environments that don't
    /// need a workspace (read-only demos, some tests).
    pub workspace_dir: Option<PathBuf>,
    /// Cap for a single blob-upload body, per
    /// `STOR-largeBlobByReference`. Operators raise this via
    /// `REQFORGE_MAX_BLOB_BYTES` when their content needs more
    /// headroom; over the cap the upload endpoint returns 413 and
    /// points at URL-reference artifacts. Default is the low
    /// figure in the ROADMAP's locked decisions.
    pub max_blob_bytes: u64,
    /// Cap for the on-disk thumbnail cache, from
    /// `REQFORGE_THUMBNAIL_CACHE_MAX_BYTES`. LRU-evicted when the
    /// sum of `<workspace>/thumbnail-cache/**/*512.png` crosses
    /// this number. 500 MB default.
    pub thumbnail_cache_max_bytes: u64,
    /// Externally-visible base URL used by the Phase 6b HTML
    /// report exports to build absolute `<a href>` links. Empty
    /// when unset — exported HTML then carries same-origin
    /// relative paths (resolve when re-served through ReqForge,
    /// break when opened offline). Operators with a real
    /// deployment URL set `REQFORGE_EXTERNAL_URL=https://
    /// reqforge.example.com`.
    pub external_url: Option<String>,
}

/// Build a World from a single subject repository, bypassing the parent-directory scan
/// `run_discovery` performs. provreq is single-subject (#370): one instance serves exactly one
/// repository, so its World holds exactly one mount — the subject itself — rather than every
/// sibling directory under a `mount_prefix`. Everything downstream (uuid index, system config,
/// link catalog, search index) is identical to `run_discovery`; only the mount set differs.
/// Build a one-mount World for provreq's single subject. `git_root` is the subject repo (where
/// `.git` lives); `project_root` is where the ReqForge project (`reqforge.json` + artifacts) lives,
/// which provreq resolves through the companion's `subject_requirements` — for its own repo these
/// differ (`.git` at the root, `reqforge.json` in `requirements/`). When they're the same path this
/// is exactly ReqForge's original single-directory classification.
pub fn discover_single(
    project_root: PathBuf,
    git_root: PathBuf,
    config: &DiscoveryConfig,
) -> Result<World, DiscoveryError> {
    let mounts = vec![crate::mount::classify_single(project_root, &git_root)];
    build_world(mounts, config)
}

/// Run the full discovery pipeline against `config`. Synchronous / blocking — callers wrap it in
/// `spawn_blocking` when they need to run it from an async context. Scans `config.mount_prefix`
/// for every sibling project (ReqForge's multi-project model); provreq uses [`discover_single`].
pub fn run_discovery(config: &DiscoveryConfig) -> Result<World, DiscoveryError> {
    let mounts = discover_mounts(&config.mount_prefix)?;
    build_world(mounts, config)
}

fn build_world(mounts: Vec<MountInfo>, config: &DiscoveryConfig) -> Result<World, DiscoveryError> {
    let loaded_projects: Vec<&crate::load::LoadedProject> = mounts
        .iter()
        .filter_map(|m| match &m.state {
            MountState::Project(p) => Some(p),
            _ => None,
        })
        .collect();
    let (index, duplicates) = build_uuid_index(&loaded_projects);
    let system = load_system_config(config.system_config_path.as_deref())?;
    let missing = missing_project_slugs(&system, &mounts);
    let link_catalog = effective_catalog(&system);
    let search_index =
        std::sync::Arc::new(SearchIndex::build(&mounts).map_err(DiscoveryError::Search)?);
    Ok(World {
        mounts,
        index,
        duplicates,
        system,
        missing_project_slugs: missing,
        link_catalog,
        search_index,
    })
}

#[derive(Debug, Error)]
pub enum DiscoveryError {
    #[error(transparent)]
    Mount(#[from] MountDiscoveryError),
    #[error(transparent)]
    System(#[from] SystemLoadError),
    #[error("search index build failed: {0}")]
    Search(SearchIndexError),
}

/// Helper: locate a mounted project by slug, returning the project
/// root path on disk. Used by CRUD handlers that need to resolve a
/// mount path from a URL param.
pub fn project_root_by_slug<'a>(world: &'a World, slug: &str) -> Option<&'a Path> {
    world.mounts.iter().find_map(|m| match &m.state {
        MountState::Project(p) if p.config.slug == slug => Some(p.root.as_path()),
        _ => None,
    })
}
