//! Ownership reconciliation per `DEPLOY-chownFromDotGit`.
//!
//! After writing a file, match its UID/GID to the owner of the
//! repository's `.git` entry so ReqForge-created files don't end
//! up owned by root on hosts where the container runs as root.
//!
//! Resolution order for target ownership:
//!
//! 1. Explicit `OwnershipOverrides` passed to [`reconcile_ownership`]
//!    — the server reads `REQFORGE_UID` / `REQFORGE_GID` once at
//!    startup (per `DEPLOY-envVars`) and passes them through.
//! 2. The owner of `<repo_root>/.git`, with git-worktree
//!    indirection: if `.git` is a regular file, its first line
//!    is `gitdir: <real-git-dir>` and that real dir's owner is
//!    used.
//! 3. No reconciliation — the file keeps whatever UID/GID the
//!    write produced.
//!
//! Reconciliation is best-effort: if the process lacks CAP_CHOWN
//! (typical when running as a non-root user inside the container),
//! the chown attempt fails and ReqForge logs the failure at debug
//! level without propagating an error. This mirrors the spec's
//! "prevent files from being created as root" framing rather than
//! turning ownership into a blocking precondition.

use std::path::{Path, PathBuf};

use thiserror::Error;

/// Per-process UID/GID overrides derived from `REQFORGE_UID` /
/// `REQFORGE_GID`. The server reads these once at startup and
/// threads them through every write path.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct OwnershipOverrides {
    pub uid: Option<u32>,
    pub gid: Option<u32>,
}

impl OwnershipOverrides {
    /// Read `REQFORGE_UID` and `REQFORGE_GID` from the process
    /// environment. Missing / empty values resolve to `None`;
    /// non-numeric values become typed errors.
    pub fn from_env() -> Result<Self, OwnershipError> {
        Ok(Self {
            uid: read_u32_env("REQFORGE_UID")
                .map_err(|raw| OwnershipError::InvalidUidEnv { raw })?,
            gid: read_u32_env("REQFORGE_GID")
                .map_err(|raw| OwnershipError::InvalidGidEnv { raw })?,
        })
    }
}

/// Attempt to chown `target` so its UID/GID match the repository's
/// `.git` owner (with the supplied overrides taking precedence).
///
/// Unix only. Windows is a no-op that returns `Ok(())` — POSIX
/// ownership semantics don't apply there, and the bind-mount story
/// on Windows hosts is the subject of risk check 2.
pub fn reconcile_ownership(
    target: &Path,
    repo_root: &Path,
    overrides: OwnershipOverrides,
) -> Result<(), OwnershipError> {
    reconcile_ownership_impl(target, repo_root, overrides)
}

#[derive(Debug, Error)]
pub enum OwnershipError {
    #[error("stat on {}: {source}", path.display())]
    Stat {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("reading git worktree pointer at {}: {source}", path.display())]
    ReadWorktreePointer {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("{} does not exist — cannot determine owner", path.display())]
    DotGitMissing { path: PathBuf },

    #[error("invalid REQFORGE_UID value '{raw}': not a non-negative integer")]
    InvalidUidEnv { raw: String },

    #[error("invalid REQFORGE_GID value '{raw}': not a non-negative integer")]
    InvalidGidEnv { raw: String },
}

#[cfg(unix)]
fn reconcile_ownership_impl(
    target: &Path,
    repo_root: &Path,
    overrides: OwnershipOverrides,
) -> Result<(), OwnershipError> {
    use std::os::unix::fs::{MetadataExt, chown};

    let (uid, gid) = resolve_uid_gid(repo_root, overrides)?;

    // If the file is already owned correctly, skip the chown
    // syscall — it's the common case on dev machines where the
    // ReqForge process and .git already share an owner.
    let meta = std::fs::metadata(target).map_err(|source| OwnershipError::Stat {
        path: target.to_path_buf(),
        source,
    })?;
    if meta.uid() == uid && meta.gid() == gid {
        return Ok(());
    }

    match chown(target, Some(uid), Some(gid)) {
        Ok(()) => Ok(()),
        Err(err) => {
            tracing::debug!(
                target = %target.display(),
                uid, gid,
                error = %err,
                "chown failed — continuing without ownership reconciliation",
            );
            Ok(())
        }
    }
}

#[cfg(not(unix))]
fn reconcile_ownership_impl(
    _target: &Path,
    _repo_root: &Path,
    _overrides: OwnershipOverrides,
) -> Result<(), OwnershipError> {
    Ok(())
}

#[cfg(unix)]
fn resolve_uid_gid(
    repo_root: &Path,
    overrides: OwnershipOverrides,
) -> Result<(u32, u32), OwnershipError> {
    if let (Some(uid), Some(gid)) = (overrides.uid, overrides.gid) {
        return Ok((uid, gid));
    }

    // Fall back to .git-derived ownership. If overrides were
    // partially provided, use them where set and the .git value
    // elsewhere.
    let (git_uid, git_gid) = dot_git_owner(repo_root)?;
    Ok((
        overrides.uid.unwrap_or(git_uid),
        overrides.gid.unwrap_or(git_gid),
    ))
}

#[cfg(unix)]
fn dot_git_owner(repo_root: &Path) -> Result<(u32, u32), OwnershipError> {
    use std::os::unix::fs::MetadataExt;

    let dot_git = repo_root.join(".git");
    let meta = match std::fs::metadata(&dot_git) {
        Ok(m) => m,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
            return Err(OwnershipError::DotGitMissing { path: dot_git });
        }
        Err(source) => {
            return Err(OwnershipError::Stat {
                path: dot_git,
                source,
            });
        }
    };

    if meta.is_file() {
        // Worktree: .git is a `gitdir: <real-git-dir>` pointer
        // file. Follow it.
        let text = std::fs::read_to_string(&dot_git).map_err(|source| {
            OwnershipError::ReadWorktreePointer {
                path: dot_git.clone(),
                source,
            }
        })?;
        let real = text
            .lines()
            .find_map(|line| line.strip_prefix("gitdir:"))
            .map(|s| s.trim())
            .unwrap_or("");
        let real_path = if real.is_empty() {
            dot_git.clone()
        } else {
            // Relative paths resolve against the repo root (git's
            // own convention for worktree pointer files).
            let p = Path::new(real);
            if p.is_absolute() {
                p.to_path_buf()
            } else {
                repo_root.join(p)
            }
        };
        let real_meta = std::fs::metadata(&real_path).map_err(|source| OwnershipError::Stat {
            path: real_path.clone(),
            source,
        })?;
        return Ok((real_meta.uid(), real_meta.gid()));
    }

    Ok((meta.uid(), meta.gid()))
}

/// Returns `Ok(Some(n))` when the env var is set and parses as a
/// non-negative u32; `Ok(None)` when unset; `Err(raw)` when set to
/// a non-numeric value (so the caller can surface a helpful error).
fn read_u32_env(key: &str) -> Result<Option<u32>, String> {
    match std::env::var(key) {
        Ok(raw) if raw.is_empty() => Ok(None),
        Ok(raw) => match raw.parse::<u32>() {
            Ok(n) => Ok(Some(n)),
            Err(_) => Err(raw),
        },
        Err(_) => Ok(None),
    }
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use std::os::unix::fs::MetadataExt;
    use tempfile::tempdir;

    #[test]
    fn reconciles_to_dot_git_owner_when_already_matching() {
        let temp = tempdir().unwrap();
        let repo = temp.path();
        std::fs::create_dir_all(repo.join(".git")).unwrap();
        let target = repo.join("artifact.md");
        std::fs::write(&target, b"x").unwrap();

        reconcile_ownership(&target, repo, OwnershipOverrides::default()).unwrap();
        let meta = std::fs::metadata(&target).unwrap();
        let git_meta = std::fs::metadata(repo.join(".git")).unwrap();
        assert_eq!(meta.uid(), git_meta.uid());
        assert_eq!(meta.gid(), git_meta.gid());
    }

    #[test]
    fn missing_dot_git_returns_an_error() {
        let temp = tempdir().unwrap();
        let repo = temp.path();
        std::fs::write(repo.join("artifact.md"), b"x").unwrap();
        let err = reconcile_ownership(
            &repo.join("artifact.md"),
            repo,
            OwnershipOverrides::default(),
        )
        .unwrap_err();
        assert!(matches!(err, OwnershipError::DotGitMissing { .. }));
    }

    #[test]
    fn follows_a_worktree_gitdir_pointer_file() {
        let temp = tempdir().unwrap();
        let repo = temp.path();
        let real_git = temp.path().join("real-git");
        std::fs::create_dir_all(&real_git).unwrap();
        std::fs::write(
            repo.join(".git"),
            format!("gitdir: {}\n", real_git.display()),
        )
        .unwrap();
        std::fs::write(repo.join("artifact.md"), b"x").unwrap();

        reconcile_ownership(
            &repo.join("artifact.md"),
            repo,
            OwnershipOverrides::default(),
        )
        .unwrap();
        let meta = std::fs::metadata(repo.join("artifact.md")).unwrap();
        let real_meta = std::fs::metadata(&real_git).unwrap();
        assert_eq!(meta.uid(), real_meta.uid());
        assert_eq!(meta.gid(), real_meta.gid());
    }

    #[test]
    fn explicit_overrides_bypass_dot_git_resolution() {
        let temp = tempdir().unwrap();
        let repo = temp.path();
        std::fs::write(repo.join("artifact.md"), b"x").unwrap();
        // No .git present — but we set both overrides, so the
        // .git check must be skipped entirely.
        let me = unsafe { (geteuid(), getegid()) };
        let result = reconcile_ownership(
            &repo.join("artifact.md"),
            repo,
            OwnershipOverrides {
                uid: Some(me.0),
                gid: Some(me.1),
            },
        );
        assert!(
            result.is_ok(),
            "expected overrides to skip .git check, got {result:?}",
        );
    }

    unsafe extern "C" {
        fn geteuid() -> u32;
        fn getegid() -> u32;
    }
}
