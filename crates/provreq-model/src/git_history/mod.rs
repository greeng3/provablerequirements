//! gitoxide-backed history + at-commit reads (Phase 5d).
//!
//! Each project mount has a `.git` dir; we lazy-open a
//! `gix::ThreadSafeRepository` on first use and cache it on
//! [`RepoCache`]. Handlers call [`list_artifact_commits`] for the
//! diff-view dropdown and [`read_blob_at_commit`] for the
//! `at=<oid>` extensions on `/artifact` and `/artifact/blob`.
//!
//! The code soft-fails with `HistoryError::HistoryUnavailable` on
//! shallow clones / missing commits / non-git mounts so the diff
//! view can fall through to the Phase 4b approval-snapshot path
//! (per the Phase 5 locked decision on diff fallback).

use std::path::{Path, PathBuf};
use std::sync::Arc;

use chrono::{DateTime, TimeZone, Utc};
use dashmap::DashMap;
use thiserror::Error;

/// Cap on the number of commits walked per request. Protects the
/// endpoint from accidentally tailing a 100k-commit history on a
/// monorepo mount; the dropdown UI only needs a recent slice.
pub const HISTORY_COMMIT_CAP: usize = 200;

/// Shared repo-handle cache. Cheap to clone — the underlying
/// `DashMap<PathBuf, Arc<gix::ThreadSafeRepository>>` is already
/// an `Arc` internally.
#[derive(Clone, Default)]
pub struct RepoCache {
    repos: Arc<DashMap<PathBuf, Arc<gix::ThreadSafeRepository>>>,
}

impl std::fmt::Debug for RepoCache {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RepoCache")
            .field("entries", &self.repos.len())
            .finish()
    }
}

impl RepoCache {
    pub fn new() -> Self {
        Self::default()
    }

    /// Lazy-open the repo at `git_dir` (usually `<project>/.git`)
    /// and return a cloned handle. Keyed by the canonical path so
    /// repeated lookups from different handler paths resolve to
    /// the same `Arc`.
    pub fn open(&self, git_dir: &Path) -> Result<Arc<gix::ThreadSafeRepository>, HistoryError> {
        let key = git_dir
            .canonicalize()
            .unwrap_or_else(|_| git_dir.to_path_buf());
        if let Some(entry) = self.repos.get(&key) {
            return Ok(entry.clone());
        }
        let repo = gix::open(&key).map_err(|err| HistoryError::RepoOpen {
            path: key.clone(),
            reason: err.to_string(),
        })?;
        let arc = Arc::new(repo.into_sync());
        self.repos.insert(key.clone(), arc.clone());
        Ok(arc)
    }

    /// Drop cached repos whose path is no longer present. Called
    /// on `refresh()` so a removed mount doesn't leak a handle.
    pub fn retain_paths(&self, keep: &[PathBuf]) {
        let canonical: Vec<PathBuf> = keep
            .iter()
            .map(|p| p.canonicalize().unwrap_or_else(|_| p.clone()))
            .collect();
        self.repos.retain(|path, _| canonical.contains(path));
    }

    pub fn len(&self) -> usize {
        self.repos.len()
    }

    pub fn is_empty(&self) -> bool {
        self.repos.is_empty()
    }
}

/// A single commit that touched the target path. Rendered in the
/// history dropdown and used as an `oid` when the user picks a
/// historical frame for diffs.
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CommitInfo {
    pub oid: String,
    pub short_oid: String,
    pub committed_at: DateTime<Utc>,
    pub author: String,
    pub summary: String,
}

/// Walk commits reachable from `HEAD` in chronological order
/// (newest first) and return those that touched `repo_relative`.
pub fn list_artifact_commits(
    repo: &gix::ThreadSafeRepository,
    repo_relative: &Path,
) -> Result<Vec<CommitInfo>, HistoryError> {
    let repo = repo.to_thread_local();
    let head = repo
        .head_commit()
        .map_err(|e| HistoryError::HistoryUnavailable(format!("head commit: {e}")))?;

    let target_bytes = path_to_git_bytes(repo_relative);
    let mut out: Vec<CommitInfo> = Vec::new();
    let walk = head
        .ancestors()
        .all()
        .map_err(|e| HistoryError::HistoryUnavailable(format!("ancestors walk: {e}")))?;
    for info in walk.take(HISTORY_COMMIT_CAP * 4) {
        let info =
            info.map_err(|err| HistoryError::HistoryUnavailable(format!("walk error: {err}")))?;
        let Some(commit) = repo
            .find_object(info.id)
            .ok()
            .and_then(|o| o.try_into_commit().ok())
        else {
            continue;
        };

        if !commit_touched_path(&repo, &commit, &target_bytes) {
            continue;
        }

        let header = match commit.decode() {
            Ok(h) => h,
            Err(err) => {
                return Err(HistoryError::HistoryUnavailable(format!(
                    "decode commit: {err}"
                )));
            }
        };
        let time = header.time().map_err(|err| {
            HistoryError::HistoryUnavailable(format!("decode commit time: {err}"))
        })?;
        let committed_at = Utc
            .timestamp_opt(time.seconds, 0)
            .single()
            .unwrap_or_else(Utc::now);
        let signature = header.author().map_err(|err| {
            HistoryError::HistoryUnavailable(format!("decode commit author: {err}"))
        })?;
        let author = format!("{} <{}>", signature.name, signature.email);
        let raw_message = header.message.to_string();
        let summary = raw_message.lines().next().unwrap_or("").trim().to_string();
        let oid = commit.id().to_string();
        let short_oid = oid.chars().take(10).collect::<String>();
        out.push(CommitInfo {
            oid,
            short_oid,
            committed_at,
            author,
            summary,
        });
        if out.len() >= HISTORY_COMMIT_CAP {
            break;
        }
    }
    Ok(out)
}

/// Read the bytes of `repo_relative` at commit `oid`. Used by the
/// `at=<oid>` variants of `/artifact` (for content + URL sidecars)
/// and `/artifact/blob` (for binary peers at a historical
/// commit).
pub fn read_blob_at_commit(
    repo: &gix::ThreadSafeRepository,
    oid_str: &str,
    repo_relative: &Path,
) -> Result<Vec<u8>, HistoryError> {
    let repo = repo.to_thread_local();
    let oid =
        gix::ObjectId::from_hex(oid_str.as_bytes()).map_err(|_| HistoryError::InvalidOid {
            oid: oid_str.to_owned(),
        })?;
    let commit = repo
        .find_object(oid)
        .map_err(|e| HistoryError::HistoryUnavailable(format!("find commit {oid_str}: {e}")))?
        .try_into_commit()
        .map_err(|_| HistoryError::InvalidOid {
            oid: oid_str.to_owned(),
        })?;
    let tree = commit
        .tree()
        .map_err(|e| HistoryError::HistoryUnavailable(format!("tree at {oid_str}: {e}")))?;
    let entry = tree
        .lookup_entry_by_path(repo_relative)
        .map_err(|e| HistoryError::HistoryUnavailable(format!("lookup path: {e}")))?
        .ok_or_else(|| HistoryError::PathNotInCommit {
            oid: oid_str.to_owned(),
            path: repo_relative.to_path_buf(),
        })?;
    let blob = repo
        .find_object(entry.oid())
        .map_err(|e| HistoryError::HistoryUnavailable(format!("blob lookup: {e}")))?;
    Ok(blob.data.clone())
}

/// Does this commit's tree-at-path differ from its first-parent's?
/// Root commits count as "touched" when the path exists.
fn commit_touched_path(repo: &gix::Repository, commit: &gix::Commit<'_>, target: &[u8]) -> bool {
    let Ok(tree) = commit.tree() else {
        return false;
    };
    let target_path = Path::new(std::str::from_utf8(target).unwrap_or(""));
    let entry = tree.lookup_entry_by_path(target_path).ok().flatten();

    let parent_id = commit.parent_ids().next();
    let parent_tree = parent_id.and_then(|pid| {
        repo.find_object(pid)
            .ok()
            .and_then(|o| o.try_into_commit().ok())
            .and_then(|c| c.tree().ok())
    });

    match (entry, parent_tree) {
        (Some(current), Some(parent)) => {
            let parent_entry = parent.lookup_entry_by_path(target_path).ok().flatten();
            match parent_entry {
                Some(pe) => pe.oid() != current.oid(),
                None => true,
            }
        }
        (Some(_), None) => true, // root commit that introduces the file
        (None, Some(parent)) => {
            // deletion
            parent
                .lookup_entry_by_path(target_path)
                .ok()
                .flatten()
                .is_some()
        }
        (None, None) => false,
    }
}

fn path_to_git_bytes(path: &Path) -> Vec<u8> {
    let s = path.to_string_lossy().into_owned();
    s.replace('\\', "/").into_bytes()
}

#[derive(Debug, Error)]
pub enum HistoryError {
    #[error("history unavailable: {0}")]
    HistoryUnavailable(String),

    #[error("failed to open git repo at {}: {reason}", path.display())]
    RepoOpen { path: PathBuf, reason: String },

    #[error("invalid git oid '{oid}'")]
    InvalidOid { oid: String },

    #[error("path {} not found in commit {oid}", path.display())]
    PathNotInCommit { oid: String, path: PathBuf },
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::process::Command;

    /// Build a throw-away repo with three commits touching the
    /// test file DES-spec.md (root → v1 → v2), plus an unrelated
    /// file that must not show up in the walk.
    fn init_repo_with_history(dir: &Path) {
        run(dir, &["git", "init", "-q", "-b", "main"]);
        run(dir, &["git", "config", "user.email", "a@b"]);
        run(dir, &["git", "config", "user.name", "Test"]);
        std::fs::write(dir.join("unrelated.txt"), "hello").unwrap();
        run(dir, &["git", "add", "-A"]);
        run(dir, &["git", "commit", "-q", "-m", "root"]);
        std::fs::write(dir.join("DES-spec.md"), "v1\n").unwrap();
        run(dir, &["git", "add", "-A"]);
        run(dir, &["git", "commit", "-q", "-m", "add DES-spec v1"]);
        std::fs::write(dir.join("DES-spec.md"), "v2\n").unwrap();
        run(dir, &["git", "add", "-A"]);
        run(dir, &["git", "commit", "-q", "-m", "update DES-spec v2"]);
    }

    fn run(dir: &Path, argv: &[&str]) {
        let status = Command::new(argv[0])
            .args(&argv[1..])
            .current_dir(dir)
            .status()
            .unwrap_or_else(|e| panic!("spawn {argv:?}: {e}"));
        assert!(status.success(), "command {argv:?} failed with {status}");
    }

    #[test]
    fn repo_cache_lazy_opens_and_reuses_handle() {
        let temp = tempfile::tempdir().unwrap();
        init_repo_with_history(temp.path());
        let cache = RepoCache::new();
        let a = cache.open(&temp.path().join(".git")).unwrap();
        let b = cache.open(&temp.path().join(".git")).unwrap();
        assert!(Arc::ptr_eq(&a, &b));
        assert_eq!(cache.len(), 1);
    }

    #[test]
    fn list_artifact_commits_filters_to_commits_touching_the_file() {
        let temp = tempfile::tempdir().unwrap();
        init_repo_with_history(temp.path());
        let repo = gix::open(temp.path().join(".git")).unwrap().into_sync();
        let commits = list_artifact_commits(&repo, Path::new("DES-spec.md")).unwrap();
        assert_eq!(commits.len(), 2, "two commits touched DES-spec.md");
        assert!(commits[0].summary.contains("v2"));
        assert!(commits[1].summary.contains("v1"));
    }

    #[test]
    fn read_blob_at_commit_returns_bytes_at_a_specific_oid() {
        let temp = tempfile::tempdir().unwrap();
        init_repo_with_history(temp.path());
        let repo = gix::open(temp.path().join(".git")).unwrap().into_sync();
        let commits = list_artifact_commits(&repo, Path::new("DES-spec.md")).unwrap();
        let older = commits.last().unwrap();
        let bytes = read_blob_at_commit(&repo, &older.oid, Path::new("DES-spec.md")).unwrap();
        assert_eq!(&bytes, b"v1\n");
    }

    #[test]
    fn read_blob_at_commit_surfaces_invalid_oid_cleanly() {
        let temp = tempfile::tempdir().unwrap();
        init_repo_with_history(temp.path());
        let repo = gix::open(temp.path().join(".git")).unwrap().into_sync();
        let err = read_blob_at_commit(&repo, "nothex", Path::new("DES-spec.md")).unwrap_err();
        assert!(matches!(err, HistoryError::InvalidOid { .. }));
    }

    #[test]
    fn read_blob_at_commit_returns_not_in_commit_for_unknown_path() {
        let temp = tempfile::tempdir().unwrap();
        init_repo_with_history(temp.path());
        let repo = gix::open(temp.path().join(".git")).unwrap().into_sync();
        let commits = list_artifact_commits(&repo, Path::new("DES-spec.md")).unwrap();
        let err = read_blob_at_commit(&repo, &commits[0].oid, Path::new("nope.md")).unwrap_err();
        assert!(matches!(err, HistoryError::PathNotInCommit { .. }));
    }

    #[test]
    fn retain_paths_drops_uncached_entries() {
        let temp = tempfile::tempdir().unwrap();
        init_repo_with_history(temp.path());
        let cache = RepoCache::new();
        cache.open(&temp.path().join(".git")).unwrap();
        assert_eq!(cache.len(), 1);
        cache.retain_paths(&[]);
        assert_eq!(cache.len(), 0);
    }
}
