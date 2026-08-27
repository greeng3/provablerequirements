//! Code traceability scanner (Phase 9a).
//!
//! Implements the scanner subsystem that Phase 9b's
//! `REPORT-codeTraceability` will read from:
//!
//! - [`languages`] — the built-in + System-declared source-
//!   language registry (TRACE-codeLanguageRegistry).
//! - [`tags`] — the comment-scoped tag parser with verb
//!   aliasing + multi-ID + trailing-comma continuation
//!   (TRACE-codeTagFormat).
//!
//! 9a.2 adds `config` + `walker` + `run_scan`; 9a.3 exposes a
//! debug HTTP endpoint so the subsystem is end-to-end
//! testable before 9b wraps it into the report.
//!
//! Per TRACE-codeScanNotArtifacts the scanner never creates
//! ReqForge artifacts — it emits overlay data only.

pub mod config;
pub mod languages;
pub mod tags;
pub mod walker;

pub use config::{DEFAULT_SCAN_PATHS, ResolvedScanPaths, ignore_dirs, resolve_scan_paths};
pub use languages::{BUILTIN_LANGUAGES, Language, LanguageRegistryError, effective_languages};
pub use tags::{CANONICAL_VERBS, RawTag, canonicalise_verb, parse_tags};
pub use walker::{FileScanError, FileTag, WalkOutput, extract_comment_runs, walk_scan_roots};

use std::collections::BTreeMap;
use std::path::PathBuf;

use serde::Serialize;

use crate::load::LoadedProject;
use crate::mount::MountState;
use crate::world::World;

/// Stable key for a resolved artifact reference. Phase 9b's
/// report uses this to group tags under their target.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ArtifactKey {
    pub project_slug: String,
    pub collection_prefix: String,
    pub artifact_name: String,
}

/// One resolved tag pointing at an existing artifact.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScanTag {
    pub file: PathBuf,
    pub line: usize,
    pub verb: String,
    pub raw_id: String,
}

/// A tag whose `(prefix, name)` pair didn't resolve to any
/// mounted artifact — typically because of a rename or typo.
/// The Phase 9b report surfaces these separately.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OrphanTag {
    pub file: PathBuf,
    pub line: usize,
    pub verb: String,
    pub raw_id: String,
}

/// The scanner's per-project output. Tags grouped by target
/// artifact so 9b's report builder can zip against the
/// mounted project tree without walking the file system
/// again.
#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScanOutput {
    pub tags_by_artifact: BTreeMap<String, Vec<ScanTag>>,
    pub orphan_tags: Vec<OrphanTag>,
    pub scanned_file_count: usize,
    pub file_errors: Vec<ScanFileError>,
    pub missing_declared_scan_paths: Vec<String>,
}

/// Wire form of `walker::FileScanError`. Kept separate so the
/// Serialize impl doesn't bleed into the walker's internal
/// types.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScanFileError {
    pub file: PathBuf,
    pub message: String,
}

/// Run the scanner for one project against the current world.
/// Cross-project resolution emits one entry per match, so a
/// tag that resolves to artifacts in two mounted projects
/// appears under both keys.
pub fn run_scan(project: &LoadedProject, world: &World) -> ScanOutput {
    let languages = match effective_languages(system_languages(world)) {
        Ok(langs) => langs,
        Err(_) => {
            // System-declared language validation ran at
            // discovery time already; if we somehow got here
            // with a bad list the safest behaviour is to fall
            // back to built-ins.
            BUILTIN_LANGUAGES
                .iter()
                .map(languages::SystemLanguage::from_builtin)
                .collect()
        }
    };
    let resolved = resolve_scan_paths(project);
    let walk = walk_scan_roots(&resolved.roots, &languages);

    let index = build_artifact_index(world);
    let mut tags_by_artifact: BTreeMap<String, Vec<ScanTag>> = BTreeMap::new();
    let mut orphan_tags: Vec<OrphanTag> = Vec::new();
    for FileTag {
        file,
        line,
        verb,
        raw_id,
    } in walk.tags
    {
        let matches = index.resolve(&raw_id);
        if matches.is_empty() {
            orphan_tags.push(OrphanTag {
                file,
                line,
                verb,
                raw_id,
            });
            continue;
        }
        let tag = ScanTag {
            file,
            line,
            verb,
            raw_id,
        };
        for key in matches {
            tags_by_artifact
                .entry(artifact_key_string(&key))
                .or_default()
                .push(tag.clone());
        }
    }

    ScanOutput {
        tags_by_artifact,
        orphan_tags,
        scanned_file_count: walk.scanned_file_count,
        file_errors: walk
            .file_errors
            .into_iter()
            .map(|FileScanError { file, message }| ScanFileError { file, message })
            .collect(),
        missing_declared_scan_paths: resolved.missing_declared,
    }
}

fn system_languages(world: &World) -> Option<&serde_json::Value> {
    world.system.config().and_then(|c| c.languages.as_ref())
}

/// Composite index: the raw id (`<prefix>-<name>`) → list of
/// `ArtifactKey` targets. Built once per `run_scan` call so
/// tag resolution is O(1) per tag.
struct ArtifactIndex {
    by_raw_id: BTreeMap<String, Vec<ArtifactKey>>,
}

impl ArtifactIndex {
    fn resolve(&self, raw_id: &str) -> Vec<ArtifactKey> {
        self.by_raw_id.get(raw_id).cloned().unwrap_or_default()
    }
}

fn build_artifact_index(world: &World) -> ArtifactIndex {
    let mut by_raw_id: BTreeMap<String, Vec<ArtifactKey>> = BTreeMap::new();
    for mount in &world.mounts {
        let MountState::Project(project) = &mount.state else {
            continue;
        };
        for collection in &project.collections {
            for artifact in &collection.artifacts {
                // Artifact names in ReqForge already carry
                // the prefix (`REQ-apple`), so the raw id
                // from a tag (e.g. `REQ-apple`) matches the
                // artifact's `name` directly. Using the
                // artifact name as the key therefore resolves
                // correctly against tags that spell out the
                // full ReqForge UID.
                by_raw_id
                    .entry(artifact.name.clone())
                    .or_default()
                    .push(ArtifactKey {
                        project_slug: project.config.slug.clone(),
                        collection_prefix: collection.config.prefix.clone(),
                        artifact_name: artifact.name.clone(),
                    });
            }
        }
    }
    ArtifactIndex { by_raw_id }
}

fn artifact_key_string(key: &ArtifactKey) -> String {
    format!(
        "{}/{}/{}",
        key.project_slug, key.collection_prefix, key.artifact_name
    )
}
