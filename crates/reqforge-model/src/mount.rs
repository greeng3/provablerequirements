//! Mount discovery — classifies every directory under
//! `REQFORGE_MOUNT_PREFIX` against the validity states defined in
//! `DEPLOY-mountValidityStates`.
//!
//! Read-only detection is deliberately not implemented at Phase 1a:
//! the back-end is read-only everywhere in this phase, so the
//! distinction is invisible to users. It will land alongside write
//! operations in Phase 2.

use std::fs;
use std::path::{Path, PathBuf};

use thiserror::Error;

use crate::load::{LoadedProject, ProjectLoadError, load_project};

/// The result of classifying one mounted directory.
#[derive(Debug)]
pub struct MountInfo {
    /// Absolute path to the mounted directory.
    pub path: PathBuf,
    /// The discovered state, per `DEPLOY-mountValidityStates`.
    pub state: MountState,
}

/// One of the four mount-validity categories, plus a `LoadFailed`
/// fallback for directories that pass the `.git` + `reqforge.json`
/// existence check but fail to load (for example, a corrupt
/// `reqforge.json`). The spec doesn't enumerate this explicitly,
/// but surfacing it is strictly better than silently dropping the
/// mount.
#[derive(Debug)]
pub enum MountState {
    /// Both `.git/` and `reqforge.json` present; load succeeded.
    Project(LoadedProject),
    /// `.git/` present, `reqforge.json` missing — "not yet a
    /// ReqForge project" with an init option in the UI.
    NeedsInit,
    /// `.git/` missing — show a warning banner, otherwise ignore.
    NoGit,
    /// Both gate files present but load returned a hard error.
    LoadFailed(ProjectLoadError),
}

#[derive(Debug, Error)]
pub enum MountDiscoveryError {
    #[error("mount prefix {} does not exist", path.display())]
    PrefixMissing { path: PathBuf },

    #[error("i/o error reading mount prefix {}: {source}", path.display())]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
}

/// Scan `mount_prefix` and classify each immediate subdirectory.
pub fn discover_mounts(mount_prefix: &Path) -> Result<Vec<MountInfo>, MountDiscoveryError> {
    if !mount_prefix.exists() {
        return Err(MountDiscoveryError::PrefixMissing {
            path: mount_prefix.to_path_buf(),
        });
    }

    let entries = fs::read_dir(mount_prefix).map_err(|source| MountDiscoveryError::Io {
        path: mount_prefix.to_path_buf(),
        source,
    })?;

    let mut out = Vec::new();
    for entry in entries {
        let entry = entry.map_err(|source| MountDiscoveryError::Io {
            path: mount_prefix.to_path_buf(),
            source,
        })?;
        let file_type = entry
            .file_type()
            .map_err(|source| MountDiscoveryError::Io {
                path: entry.path(),
                source,
            })?;
        if !file_type.is_dir() {
            continue;
        }
        // Skip hidden directories (e.g. `.reqforge-workspace`,
        // `.git`, `.idea`). Operators commonly co-locate ReqForge's
        // own workspace dir alongside the project repos, and those
        // dot-prefixed dirs are never themselves projects.
        if entry.file_name().to_string_lossy().starts_with('.') {
            continue;
        }
        out.push(classify_mount(entry.path()));
    }

    // Deterministic ordering: directory name ascending.
    out.sort_by(|a, b| a.path.cmp(&b.path));
    Ok(out)
}

pub(crate) fn classify_mount(path: PathBuf) -> MountInfo {
    classify_at(path.clone(), &path)
}

/// Single-subject classify where the git repo and the ReqForge project can live at
/// different paths. ReqForge's multi-project model assumes `.git` and `reqforge.json`
/// sit in the same mount directory; provreq self-hosts with `.git` at the repo root and
/// `reqforge.json` in a `requirements/` subdir, so it resolves the two roots separately
/// (`git_root` from the subject, `project_root` from the companion's `subject_requirements`)
/// and classifies against both. The returned mount's `path` is `project_root`, so
/// collections and artifacts resolve where they actually live.
pub(crate) fn classify_single(project_root: PathBuf, git_root: &Path) -> MountInfo {
    classify_at(project_root, git_root)
}

/// Shared core: `.git` is looked for under `git_root`, `reqforge.json` under `project_root`.
/// When the two are the same path this is exactly ReqForge's original per-mount rule.
fn classify_at(project_root: PathBuf, git_root: &Path) -> MountInfo {
    let has_git = git_root.join(".git").exists();
    let has_config = project_root.join("reqforge.json").exists();

    let state = match (has_git, has_config) {
        (false, _) => MountState::NoGit,
        (true, false) => MountState::NeedsInit,
        (true, true) => match load_project(&project_root) {
            Ok(project) => MountState::Project(project),
            Err(err) => MountState::LoadFailed(err),
        },
    };

    MountInfo {
        path: project_root,
        state,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn discover_mounts_skips_hidden_directories() {
        let tmp = tempfile::tempdir().unwrap();
        fs::create_dir(tmp.path().join("alpha")).unwrap();
        fs::create_dir(tmp.path().join(".reqforge-workspace")).unwrap();
        fs::create_dir(tmp.path().join(".git")).unwrap();
        fs::create_dir(tmp.path().join("beta")).unwrap();

        let mounts = discover_mounts(tmp.path()).unwrap();
        let names: Vec<String> = mounts
            .iter()
            .map(|m| m.path.file_name().unwrap().to_string_lossy().into_owned())
            .collect();

        assert_eq!(names, vec!["alpha", "beta"]);
    }

    #[test]
    fn classify_single_loads_a_project_with_git_and_config_at_different_roots() {
        // provreq's layout: `.git` at the repo root, `reqforge.json` + artifacts in a
        // `requirements/` subdir. Neither `classify_mount(root)` nor
        // `classify_mount(requirements)` would classify this as a Project.
        let tmp = tempfile::tempdir().unwrap();
        let git_root = tmp.path();
        let project_root = git_root.join("requirements");
        fs::create_dir(git_root.join(".git")).unwrap();
        fs::create_dir_all(project_root.join("artifacts")).unwrap();
        fs::write(
            project_root.join("reqforge.json"),
            r#"{"schemaVersion":1,"slug":"provreq","name":"Provable Requirements"}"#,
        )
        .unwrap();

        let mount = classify_single(project_root.clone(), git_root);
        assert_eq!(mount.path, project_root);
        match mount.state {
            MountState::Project(p) => assert_eq!(p.config.slug, "provreq"),
            other => panic!("expected Project, got {other:?}"),
        }
    }

    #[test]
    fn classify_single_is_needs_init_when_config_missing_under_project_root() {
        let tmp = tempfile::tempdir().unwrap();
        let git_root = tmp.path();
        let project_root = git_root.join("requirements");
        fs::create_dir(git_root.join(".git")).unwrap();
        fs::create_dir(&project_root).unwrap();

        assert!(matches!(
            classify_single(project_root, git_root).state,
            MountState::NeedsInit
        ));
    }

    #[test]
    fn classify_single_is_no_git_when_repo_root_has_no_git() {
        let tmp = tempfile::tempdir().unwrap();
        let project_root = tmp.path().join("requirements");
        fs::create_dir_all(project_root.join("artifacts")).unwrap();
        fs::write(
            project_root.join("reqforge.json"),
            r#"{"schemaVersion":1,"slug":"provreq","name":"Provable Requirements"}"#,
        )
        .unwrap();

        assert!(matches!(
            classify_single(project_root, tmp.path()).state,
            MountState::NoGit
        ));
    }
}
