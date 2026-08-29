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
    let has_git = path.join(".git").exists();
    let has_config = path.join("reqforge.json").exists();

    let state = match (has_git, has_config) {
        (false, _) => MountState::NoGit,
        (true, false) => MountState::NeedsInit,
        (true, true) => match load_project(&path) {
            Ok(project) => MountState::Project(project),
            Err(err) => MountState::LoadFailed(err),
        },
    };

    MountInfo { path, state }
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
}
