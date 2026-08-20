//! Project-level loader — orchestrates reading `reqforge.json` plus
//! the Collections tree underneath it.

use std::fs;
use std::path::{Path, PathBuf};

use thiserror::Error;

use crate::schema::sidecar::{SIDECAR_SUFFIX, is_sidecar_path};
use crate::schema::{ArtifactShape, CollectionConfig, ProjectConfig};
use crate::schema_migration::{FileType, SchemaMigrationError, migrate_value};

use super::LoadDiagnostic;
use super::artifact::{LoadedArtifact, load_content_artifact};
use super::blob::load_blob_artifact;
use super::url::load_url_artifact;

/// A Project, its Collections, and any soft diagnostics accumulated
/// during the load.
#[derive(Debug)]
pub struct LoadedProject {
    pub root: PathBuf,
    pub config: ProjectConfig,
    pub collections: Vec<LoadedCollection>,
    pub diagnostics: Vec<LoadDiagnostic>,
}

impl LoadedProject {
    /// Best-effort read of the mount's own `.git/config` for a
    /// `[user] name = …` entry. Returns `None` when the file is
    /// missing, unreadable, or doesn't carry a user name — callers
    /// fall back to the workspace's default reviewer identity.
    ///
    /// Read lazily (not at load time) so the INI parse cost is only
    /// paid on the review-identity endpoint, not on every
    /// discovery.
    pub fn git_user_name(&self) -> Option<String> {
        crate::reviews::parse_git_config_user_name(&self.root.join(".git/config"))
            .ok()
            .flatten()
    }

    /// Absolute path to the mount's own `.git` directory, or `None`
    /// when the mount is a NeedsInit / NoGit candidate that hasn't
    /// been `git init`ed. Phase 5d's gitoxide history service
    /// consumes this to open the repo handle; keeping the helper
    /// here means discovery doesn't need to know about gitoxide.
    pub fn git_repo_path(&self) -> Option<PathBuf> {
        let git_dir = self.root.join(".git");
        if git_dir.exists() {
            Some(git_dir)
        } else {
            None
        }
    }
}

#[derive(Debug)]
pub struct LoadedCollection {
    /// The on-disk directory name (not the config prefix). Collection
    /// identity is in the config; the directory name is just the
    /// human label.
    pub dir_name: String,
    pub dir_path: PathBuf,
    pub config: CollectionConfig,
    pub artifacts: Vec<LoadedArtifact>,
}

#[derive(Debug, Error)]
pub enum ProjectLoadError {
    #[error("reqforge.json not found at {}", path.display())]
    ConfigMissing { path: PathBuf },

    #[error("i/o error reading reqforge.json at {}: {source}", path.display())]
    ConfigIo {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("invalid reqforge.json at {}: {source}", path.display())]
    ConfigParse {
        path: PathBuf,
        #[source]
        source: serde_json::Error,
    },

    #[error(
        "artifacts root {} does not exist (relative to project)",
        path.display()
    )]
    ArtifactsRootMissing { path: PathBuf },

    #[error("schema migration failed for {}: {source}", path.display())]
    ConfigSchema {
        path: PathBuf,
        #[source]
        source: SchemaMigrationError,
    },
}

/// Load a Project from the repository root.
///
/// `root` is the directory containing `reqforge.json`. Soft failures
/// (bad artifacts, missing `.collection.json`) become
/// [`LoadDiagnostic`]s on the returned [`LoadedProject`]. Hard
/// failures (missing Project config, missing artifacts root) return
/// an `Err`.
pub fn load_project(root: &Path) -> Result<LoadedProject, ProjectLoadError> {
    let config_path = root.join("reqforge.json");
    if !config_path.exists() {
        return Err(ProjectLoadError::ConfigMissing { path: config_path });
    }

    let config_text =
        fs::read_to_string(&config_path).map_err(|source| ProjectLoadError::ConfigIo {
            path: config_path.clone(),
            source,
        })?;
    let raw: serde_json::Value =
        serde_json::from_str(&config_text).map_err(|source| ProjectLoadError::ConfigParse {
            path: config_path.clone(),
            source,
        })?;
    let (migrated, _) =
        migrate_value(FileType::Project, raw).map_err(|source| ProjectLoadError::ConfigSchema {
            path: config_path.clone(),
            source,
        })?;
    let config: ProjectConfig =
        serde_json::from_value(migrated).map_err(|source| ProjectLoadError::ConfigParse {
            path: config_path.clone(),
            source,
        })?;

    let artifacts_root = root.join(config.effective_artifacts_path());
    if !artifacts_root.exists() {
        return Err(ProjectLoadError::ArtifactsRootMissing {
            path: artifacts_root,
        });
    }

    let mut collections = Vec::new();
    let mut diagnostics = Vec::new();

    let subdirs = immediate_subdirectories(&artifacts_root).unwrap_or_default();
    for subdir in subdirs {
        let dir_name = subdir
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or_default()
            .to_owned();

        match load_collection(&subdir, &dir_name, root, &mut diagnostics) {
            Some(collection) => collections.push(collection),
            None => { /* load_collection already pushed a diagnostic */ }
        }
    }

    // Deterministic ordering for UI stability.
    collections.sort_by(|a, b| a.config.prefix.cmp(&b.config.prefix));

    Ok(LoadedProject {
        root: root.to_path_buf(),
        config,
        collections,
        diagnostics,
    })
}

fn load_collection(
    dir: &Path,
    dir_name: &str,
    project_root: &Path,
    diagnostics: &mut Vec<LoadDiagnostic>,
) -> Option<LoadedCollection> {
    let config_path = dir.join(".collection.json");
    if !config_path.exists() {
        diagnostics.push(LoadDiagnostic::CollectionConfigMissing {
            dir_name: dir_name.to_owned(),
        });
        return None;
    }

    let config: CollectionConfig = match fs::read_to_string(&config_path) {
        Ok(text) => match load_collection_config_text(&text) {
            Ok(cfg) => cfg,
            Err(CollectionConfigError::Schema(SchemaMigrationError::NewerThanCurrent {
                file_type: _,
                found,
                current,
            })) => {
                diagnostics.push(LoadDiagnostic::SchemaTooNew {
                    path: config_path.clone(),
                    file_type: FileType::Collection,
                    found_version: found,
                    current_version: current,
                });
                diagnostics.push(LoadDiagnostic::CollectionConfigInvalid {
                    dir_name: dir_name.to_owned(),
                    path: config_path.clone(),
                    reason: format!(
                        "schema: file is at schemaVersion {found} but this build supports up to {current}"
                    ),
                });
                return None;
            }
            Err(err) => {
                diagnostics.push(LoadDiagnostic::CollectionConfigInvalid {
                    dir_name: dir_name.to_owned(),
                    path: config_path.clone(),
                    reason: err.to_string(),
                });
                return None;
            }
        },
        Err(source) => {
            diagnostics.push(LoadDiagnostic::CollectionConfigInvalid {
                dir_name: dir_name.to_owned(),
                path: config_path.clone(),
                reason: format!("i/o: {source}"),
            });
            return None;
        }
    };

    let mut artifacts = Vec::new();
    let mut md_files = immediate_markdown_files(dir).unwrap_or_default();
    md_files.sort();
    for path in md_files {
        match load_content_artifact(&path) {
            Ok(loaded) => artifacts.push(loaded),
            Err(err) => {
                if let Some((found, current)) = err.schema_too_new() {
                    diagnostics.push(LoadDiagnostic::SchemaTooNew {
                        path: path.clone(),
                        file_type: FileType::Artifact,
                        found_version: found,
                        current_version: current,
                    });
                }
                let name = path
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or("")
                    .to_owned();
                diagnostics.push(LoadDiagnostic::ArtifactFailed {
                    name,
                    path: path.clone(),
                    reason: err.short_reason(),
                });
            }
        }
    }

    // Phase 5a: walk `.reqforge.json` sidecars for blob + URL
    // artifacts. Dispatch on the `shape` field inside the JSON so
    // a single walker covers both shapes; unreadable or mis-shaped
    // sidecars become diagnostics rather than aborting the load.
    let mut sidecars = immediate_sidecar_files(dir).unwrap_or_default();
    sidecars.sort();
    for path in sidecars {
        match peek_sidecar_shape(&path) {
            Ok(ArtifactShape::Blob) => match load_blob_artifact(&path, project_root) {
                Ok(loaded) => artifacts.push(loaded),
                Err(err) => {
                    if let Some((found, current)) = err.schema_too_new() {
                        diagnostics.push(LoadDiagnostic::SchemaTooNew {
                            path: path.clone(),
                            file_type: FileType::Artifact,
                            found_version: found,
                            current_version: current,
                        });
                    }
                    diagnostics.push(LoadDiagnostic::ArtifactFailed {
                        name: sidecar_artifact_name(&path),
                        path: path.clone(),
                        reason: err.short_reason(),
                    });
                }
            },
            Ok(ArtifactShape::Url) => match load_url_artifact(&path) {
                Ok(loaded) => artifacts.push(loaded),
                Err(err) => {
                    if let Some((found, current)) = err.schema_too_new() {
                        diagnostics.push(LoadDiagnostic::SchemaTooNew {
                            path: path.clone(),
                            file_type: FileType::Artifact,
                            found_version: found,
                            current_version: current,
                        });
                    }
                    diagnostics.push(LoadDiagnostic::ArtifactFailed {
                        name: sidecar_artifact_name(&path),
                        path: path.clone(),
                        reason: err.short_reason(),
                    });
                }
            },
            Ok(ArtifactShape::Content) => {
                diagnostics.push(LoadDiagnostic::ArtifactFailed {
                    name: sidecar_artifact_name(&path),
                    path: path.clone(),
                    reason: "content-hosted artifacts live in .md files, not sidecars".to_owned(),
                });
            }
            Err(reason) => {
                diagnostics.push(LoadDiagnostic::ArtifactFailed {
                    name: sidecar_artifact_name(&path),
                    path: path.clone(),
                    reason,
                });
            }
        }
    }

    // Orphan-binary detection: a file in the collection directory
    // that matches a blob sidecar name pattern but has no sidecar.
    // Phase 5a flags these as a diagnostic so a partial upload
    // surfaces in the UI.
    let orphan_binaries = collect_orphan_binaries(dir).unwrap_or_default();
    for orphan in orphan_binaries {
        diagnostics.push(LoadDiagnostic::OrphanBinary { path: orphan });
    }

    Some(LoadedCollection {
        dir_name: dir_name.to_owned(),
        dir_path: dir.to_path_buf(),
        config,
        artifacts,
    })
}

/// Failure modes for `load_collection_config_text`. Distinct
/// from a plain `String` so the caller can pick out the
/// schema-too-new case and emit a dedicated diagnostic.
#[derive(Debug)]
enum CollectionConfigError {
    Json(serde_json::Error),
    Schema(SchemaMigrationError),
    Deserialize(serde_json::Error),
}

impl std::fmt::Display for CollectionConfigError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Json(e) => write!(f, "JSON: {e}"),
            Self::Schema(e) => write!(f, "schema: {e}"),
            Self::Deserialize(e) => write!(f, "JSON: {e}"),
        }
    }
}

/// Parse the contents of a `.collection.json` file via the
/// Phase 11a migration pipeline. Returns a typed error so the
/// caller can distinguish schema-too-new (dedicated diagnostic)
/// from ordinary parse / deserialize failures.
fn load_collection_config_text(text: &str) -> Result<CollectionConfig, CollectionConfigError> {
    let raw: serde_json::Value = serde_json::from_str(text).map_err(CollectionConfigError::Json)?;
    let (migrated, _) =
        migrate_value(FileType::Collection, raw).map_err(CollectionConfigError::Schema)?;
    serde_json::from_value::<CollectionConfig>(migrated).map_err(CollectionConfigError::Deserialize)
}

fn immediate_sidecar_files(dir: &Path) -> std::io::Result<Vec<PathBuf>> {
    let mut out = Vec::new();
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        if entry.file_type()?.is_file() && is_sidecar_path(&entry.path()) {
            out.push(entry.path());
        }
    }
    Ok(out)
}

fn collect_orphan_binaries(dir: &Path) -> std::io::Result<Vec<PathBuf>> {
    // Gather sidecar paths for quick peer-check.
    let sidecars: std::collections::HashSet<PathBuf> =
        immediate_sidecar_files(dir)?.into_iter().collect();
    let mut out = Vec::new();
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if !entry.file_type()?.is_file() {
            continue;
        }
        let name = match path.file_name().and_then(|s| s.to_str()) {
            Some(n) => n,
            None => continue,
        };
        // Skip the three file categories we already recognised:
        // markdown content artifacts, the collection config, and
        // sidecars themselves.
        if name == ".collection.json" || name.ends_with(".md") || name.ends_with(SIDECAR_SUFFIX) {
            continue;
        }
        // An orphan is a binary whose companion sidecar is absent.
        let expected_sidecar = crate::schema::sidecar::sidecar_path_for_blob(&path);
        if !sidecars.contains(&expected_sidecar) {
            out.push(path);
        }
    }
    out.sort();
    Ok(out)
}

fn peek_sidecar_shape(path: &Path) -> Result<ArtifactShape, String> {
    let text = fs::read_to_string(path).map_err(|e| format!("i/o: {e}"))?;
    // Parse only the `shape` field to decide which loader owns the
    // sidecar. Full parse runs inside each loader so error
    // reporting stays per-shape.
    #[derive(serde::Deserialize)]
    struct ShapePeek {
        shape: ArtifactShape,
    }
    let peek: ShapePeek = serde_json::from_str(&text).map_err(|e| format!("JSON: {e}"))?;
    Ok(peek.shape)
}

fn sidecar_artifact_name(path: &Path) -> String {
    crate::schema::sidecar::artifact_name_from_sidecar(path).unwrap_or_default()
}

fn immediate_subdirectories(dir: &Path) -> std::io::Result<Vec<PathBuf>> {
    let mut out = Vec::new();
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        if !entry.file_type()?.is_dir() {
            continue;
        }
        // Dot-prefixed directories are reserved for tooling
        // (ReqForge sidecars under `.suggestions/`, IDE dirs,
        // `.git`, etc.) and are not collection candidates. Skip
        // silently so they don't pollute the project's
        // diagnostics list.
        let name = entry.file_name();
        if name.to_str().is_some_and(|s| s.starts_with('.')) {
            continue;
        }
        out.push(entry.path());
    }
    out.sort();
    Ok(out)
}

fn immediate_markdown_files(dir: &Path) -> std::io::Result<Vec<PathBuf>> {
    let mut out = Vec::new();
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if entry.file_type()?.is_file() && path.extension().and_then(|s| s.to_str()) == Some("md") {
            out.push(path);
        }
    }
    Ok(out)
}
