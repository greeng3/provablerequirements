//! Bulk-migrate engine per `STOR-schemaBulkMigrate`.
//!
//! Walks every ReqForge-authored file inside one Project,
//! applies the registered migration chain to each, and rewrites
//! changed files through the existing atomic-write path. Files
//! already at the current schemaVersion are skipped.
//!
//! Pre-flight:
//!
//! - If the Project is a git repo AND the worktree has
//!   uncommitted changes, the run is refused unless the caller
//!   passes `force = true`. Matches the ROADMAP decision: the
//!   migration should land as its own commit, not mix with the
//!   operator's in-progress work.
//! - ReqForge never commits. The operator is expected to stage
//!   + commit the rewritten files themselves.

use std::path::{Path, PathBuf};

use serde::Serialize;
use thiserror::Error;

use crate::frontmatter::split_frontmatter;
use crate::schema_migration::{FileType, MigrationOutcome, migrate_value};
use crate::write::{OwnershipOverrides, atomic_write, reconcile_ownership};

#[derive(Debug, Error)]
pub enum BulkMigrateError {
    #[error("project root {} does not exist", path.display())]
    ProjectMissing { path: PathBuf },

    #[error("project worktree has uncommitted changes; commit them or pass force=true")]
    DirtyWorktree,

    #[error("failed to inspect git status at {}: {detail}", path.display())]
    GitInspect { path: PathBuf, detail: String },
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BulkMigrateResult {
    pub files_scanned: usize,
    pub files_rewritten: usize,
    pub files_up_to_date: usize,
    pub failures: Vec<FileFailure>,
    pub rewritten: Vec<FileRewrite>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FileRewrite {
    pub path: String,
    pub outcome: MigrationOutcome,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FileFailure {
    pub path: String,
    pub file_type: FileType,
    pub error: String,
}

/// Entry point. Walks the project and rewrites any file whose
/// schemaVersion is below the current. The `overrides` argument
/// is the same ownership override set the CRUD handlers use, so
/// rewrites honor the deployment's UID / GID contract.
pub fn migrate_project(
    project_root: &Path,
    overrides: OwnershipOverrides,
    force: bool,
) -> Result<BulkMigrateResult, BulkMigrateError> {
    if !project_root.exists() {
        return Err(BulkMigrateError::ProjectMissing {
            path: project_root.to_path_buf(),
        });
    }
    if !force {
        check_worktree_clean(project_root)?;
    }

    let mut result = BulkMigrateResult {
        files_scanned: 0,
        files_rewritten: 0,
        files_up_to_date: 0,
        failures: Vec::new(),
        rewritten: Vec::new(),
    };

    migrate_project_config(project_root, overrides, &mut result);
    walk_collections(project_root, overrides, &mut result);

    Ok(result)
}

/// Probe the worktree via `git status --porcelain`. The gix
/// feature set we link against doesn't expose a fully-baked
/// working-tree status iterator at this version, and the CLI
/// is always available wherever ReqForge runs against a real
/// repo. A non-empty `--porcelain` output means the worktree
/// is dirty; we never parse the output — presence alone is
/// enough.
fn check_worktree_clean(project_root: &Path) -> Result<(), BulkMigrateError> {
    if !project_root.join(".git").exists() {
        // No git repo → nothing to be dirty about.
        return Ok(());
    }
    let output = std::process::Command::new("git")
        .arg("status")
        .arg("--porcelain")
        .current_dir(project_root)
        .output()
        .map_err(|err| BulkMigrateError::GitInspect {
            path: project_root.to_path_buf(),
            detail: format!("invoke git: {err}"),
        })?;
    if !output.status.success() {
        return Err(BulkMigrateError::GitInspect {
            path: project_root.to_path_buf(),
            detail: format!(
                "git status exited with code {:?}: {}",
                output.status.code(),
                String::from_utf8_lossy(&output.stderr).trim()
            ),
        });
    }
    if !output.stdout.is_empty() {
        return Err(BulkMigrateError::DirtyWorktree);
    }
    Ok(())
}

fn migrate_project_config(
    project_root: &Path,
    overrides: OwnershipOverrides,
    result: &mut BulkMigrateResult,
) {
    let path = project_root.join("reqforge.json");
    if !path.exists() {
        return;
    }
    process_json_file(&path, project_root, FileType::Project, overrides, result);
}

fn walk_collections(
    project_root: &Path,
    overrides: OwnershipOverrides,
    result: &mut BulkMigrateResult,
) {
    // Read the project config again (post-migration), this time
    // to learn the artifacts root. Small extra read; keeps the
    // bulk walker self-contained.
    let config_path = project_root.join("reqforge.json");
    let artifacts_root = read_artifacts_root(project_root, &config_path);
    let Ok(dirs) = std::fs::read_dir(&artifacts_root) else {
        return;
    };
    for entry in dirs.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        migrate_collection(&path, project_root, overrides, result);
    }
}

fn read_artifacts_root(project_root: &Path, config_path: &Path) -> PathBuf {
    let Ok(text) = std::fs::read_to_string(config_path) else {
        return project_root.join("artifacts");
    };
    let Ok(value) = serde_json::from_str::<serde_json::Value>(&text) else {
        return project_root.join("artifacts");
    };
    let rel = value
        .get("artifactsPath")
        .and_then(|v| v.as_str())
        .unwrap_or("artifacts");
    project_root.join(rel)
}

fn migrate_collection(
    dir: &Path,
    project_root: &Path,
    overrides: OwnershipOverrides,
    result: &mut BulkMigrateResult,
) {
    let config_path = dir.join(".collection.json");
    if config_path.exists() {
        process_json_file(
            &config_path,
            project_root,
            FileType::Collection,
            overrides,
            result,
        );
    }
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        let Some(name) = path.file_name().and_then(|s| s.to_str()) else {
            continue;
        };
        if name.ends_with(".md") {
            process_frontmatter_file(&path, project_root, overrides, result);
        } else if name.ends_with(".reqforge.json") {
            // Blob / URL sidecars carry artifact frontmatter
            // in a standalone JSON file.
            process_json_file(&path, project_root, FileType::Artifact, overrides, result);
        }
    }
}

/// Migrate a standalone JSON file (project / collection /
/// system configs + blob/URL sidecars). Atomic-rewrite on any
/// `migrated: true` outcome.
fn process_json_file(
    path: &Path,
    project_root: &Path,
    file_type: FileType,
    overrides: OwnershipOverrides,
    result: &mut BulkMigrateResult,
) {
    result.files_scanned += 1;
    let text = match std::fs::read_to_string(path) {
        Ok(t) => t,
        Err(err) => {
            result.failures.push(FileFailure {
                path: path.display().to_string(),
                file_type,
                error: format!("i/o: {err}"),
            });
            return;
        }
    };
    let raw: serde_json::Value = match serde_json::from_str(&text) {
        Ok(v) => v,
        Err(err) => {
            result.failures.push(FileFailure {
                path: path.display().to_string(),
                file_type,
                error: format!("JSON: {err}"),
            });
            return;
        }
    };
    let (migrated, outcome) = match migrate_value(file_type, raw) {
        Ok(x) => x,
        Err(err) => {
            result.failures.push(FileFailure {
                path: path.display().to_string(),
                file_type,
                error: err.to_string(),
            });
            return;
        }
    };
    if !outcome.migrated {
        result.files_up_to_date += 1;
        return;
    }
    match rewrite_json(path, project_root, &migrated, overrides) {
        Ok(()) => {
            result.files_rewritten += 1;
            result.rewritten.push(FileRewrite {
                path: path.display().to_string(),
                outcome,
            });
        }
        Err(err) => {
            result.failures.push(FileFailure {
                path: path.display().to_string(),
                file_type,
                error: err.to_string(),
            });
        }
    }
}

/// Migrate a `.md` artifact's frontmatter in place, preserving
/// the body and the `---` delimiters.
fn process_frontmatter_file(
    path: &Path,
    project_root: &Path,
    overrides: OwnershipOverrides,
    result: &mut BulkMigrateResult,
) {
    result.files_scanned += 1;
    let text = match std::fs::read_to_string(path) {
        Ok(t) => t,
        Err(err) => {
            result.failures.push(FileFailure {
                path: path.display().to_string(),
                file_type: FileType::Artifact,
                error: format!("i/o: {err}"),
            });
            return;
        }
    };
    let (json_text, body) = match split_frontmatter(&text) {
        Ok(x) => x,
        Err(err) => {
            result.failures.push(FileFailure {
                path: path.display().to_string(),
                file_type: FileType::Artifact,
                error: format!("frontmatter: {err}"),
            });
            return;
        }
    };
    let raw: serde_json::Value = match serde_json::from_str(json_text) {
        Ok(v) => v,
        Err(err) => {
            result.failures.push(FileFailure {
                path: path.display().to_string(),
                file_type: FileType::Artifact,
                error: format!("JSON: {err}"),
            });
            return;
        }
    };
    let (migrated, outcome) = match migrate_value(FileType::Artifact, raw) {
        Ok(x) => x,
        Err(err) => {
            result.failures.push(FileFailure {
                path: path.display().to_string(),
                file_type: FileType::Artifact,
                error: err.to_string(),
            });
            return;
        }
    };
    if !outcome.migrated {
        result.files_up_to_date += 1;
        return;
    }
    match rewrite_frontmatter(path, project_root, &migrated, body, overrides) {
        Ok(()) => {
            result.files_rewritten += 1;
            result.rewritten.push(FileRewrite {
                path: path.display().to_string(),
                outcome,
            });
        }
        Err(err) => {
            result.failures.push(FileFailure {
                path: path.display().to_string(),
                file_type: FileType::Artifact,
                error: err.to_string(),
            });
        }
    }
}

fn rewrite_json(
    path: &Path,
    project_root: &Path,
    value: &serde_json::Value,
    overrides: OwnershipOverrides,
) -> Result<(), String> {
    let pretty = serde_json::to_string_pretty(value).map_err(|e| format!("serialize: {e}"))?;
    let mut bytes = pretty.into_bytes();
    bytes.push(b'\n');
    atomic_write(path, &bytes).map_err(|e| format!("atomic write: {e}"))?;
    reconcile_ownership(path, project_root, overrides).map_err(|e| format!("chown: {e}"))?;
    Ok(())
}

fn rewrite_frontmatter(
    path: &Path,
    project_root: &Path,
    value: &serde_json::Value,
    body: &str,
    overrides: OwnershipOverrides,
) -> Result<(), String> {
    let pretty = serde_json::to_string_pretty(value).map_err(|e| format!("serialize: {e}"))?;
    let mut out = String::with_capacity(pretty.len() + body.len() + 16);
    out.push_str("---\n");
    out.push_str(&pretty);
    out.push_str("\n---\n");
    out.push_str(body);
    atomic_write(path, out.as_bytes()).map_err(|e| format!("atomic write: {e}"))?;
    reconcile_ownership(path, project_root, overrides).map_err(|e| format!("chown: {e}"))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn write_project_skeleton(root: &Path) {
        fs::create_dir_all(root.join("artifacts").join("req")).unwrap();
        fs::write(
            root.join("reqforge.json"),
            serde_json::to_string_pretty(&serde_json::json!({
                "schemaVersion": 1,
                "slug": "sample",
                "name": "Sample",
                "description": "t"
            }))
            .unwrap(),
        )
        .unwrap();
        fs::write(
            root.join("artifacts/req/.collection.json"),
            serde_json::to_string_pretty(&serde_json::json!({
                "schemaVersion": 1,
                "prefix": "REQ",
                "name": "Requirements"
            }))
            .unwrap(),
        )
        .unwrap();
        let fm = serde_json::to_string_pretty(&serde_json::json!({
            "schemaVersion": 1,
            "uuid": "11111111-1111-1111-1111-111111111111",
            "title": "t",
            "shape": "content",
            "createdAt": "2026-04-24T00:00:00Z",
            "modifiedAt": "2026-04-24T00:00:00Z",
            "links": [],
            "reviewLog": []
        }))
        .unwrap();
        fs::write(
            root.join("artifacts/req/REQ-one.md"),
            format!("---\n{fm}\n---\n# Title\n\nBody.\n"),
        )
        .unwrap();
    }

    #[test]
    fn migrate_project_with_all_v1_files_rewrites_nothing() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path();
        write_project_skeleton(root);
        let overrides = OwnershipOverrides::default();
        let result = migrate_project(root, overrides, true).unwrap();
        assert_eq!(result.files_rewritten, 0);
        assert!(result.files_scanned >= 3); // project + collection + artifact
        assert_eq!(result.files_up_to_date, result.files_scanned);
        assert!(result.failures.is_empty());
    }

    #[test]
    fn migrate_project_fails_cleanly_when_project_missing() {
        let err = migrate_project(
            Path::new("/nonexistent-xyz-123"),
            OwnershipOverrides::default(),
            true,
        )
        .unwrap_err();
        assert!(matches!(err, BulkMigrateError::ProjectMissing { .. }));
    }

    #[test]
    fn migrate_project_tolerates_absent_git_dir_when_force_is_false() {
        // No .git — force=false should still succeed because
        // there's nothing to be dirty about.
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path();
        write_project_skeleton(root);
        let result = migrate_project(root, OwnershipOverrides::default(), false).unwrap();
        assert!(result.failures.is_empty());
    }

    #[test]
    fn migrate_project_emits_failure_entries_for_too_new_files() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path();
        write_project_skeleton(root);
        // Hand-craft a too-new artifact alongside the v1 one.
        let too_new = serde_json::to_string_pretty(&serde_json::json!({
            "schemaVersion": 99,
            "uuid": "22222222-2222-2222-2222-222222222222",
            "title": "future",
            "shape": "content",
            "createdAt": "2026-04-24T00:00:00Z",
            "modifiedAt": "2026-04-24T00:00:00Z",
            "links": [],
            "reviewLog": []
        }))
        .unwrap();
        fs::write(
            root.join("artifacts/req/REQ-two.md"),
            format!("---\n{too_new}\n---\n# Title\n\nBody.\n"),
        )
        .unwrap();
        let result = migrate_project(root, OwnershipOverrides::default(), true).unwrap();
        assert_eq!(result.failures.len(), 1);
        assert!(result.failures[0].error.contains("newer than"));
        assert_eq!(result.failures[0].file_type, FileType::Artifact);
    }
}
