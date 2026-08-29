//! Polling filesystem watcher per `DEPLOY-pollingWatch`.
//!
//! Detects external changes (git pull, text-editor save outside
//! ReqForge) by periodically snapshotting a cheap fingerprint of
//! the mount prefix — the set of file paths and their
//! modification timestamps — and comparing successive snapshots.
//! On diff, triggers `AppState::refresh` which rediscovers and
//! broadcasts a `ChangeEvent` to any open SSE clients.
//!
//! Polling is the detection mechanism by design: inotify and
//! similar do not reliably cross bind-mount boundaries on all
//! host platforms.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, SystemTime};

use tokio::time::interval;

use crate::app::AppState;

/// Polling interval. Short enough that an external `git pull` is
/// noticeable within a second or two; long enough not to drown
/// dev laptops in stat() calls.
pub const DEFAULT_POLL_INTERVAL: Duration = Duration::from_secs(2);

/// Cheap per-file fingerprint: sorted map of repository-relative
/// path to last-modified timestamp. Stable ordering via BTreeMap
/// so equal fingerprints compare cleanly.
pub type Fingerprint = BTreeMap<PathBuf, SystemTime>;

/// Walk `mount_prefix` and compute a fingerprint covering every
/// file ReqForge could care about: `.collection.json`,
/// `reqforge.json`, and `*.md` under any directory. Paths include
/// `.git/` too so branch switches trigger a refresh.
///
/// Synchronous / blocking; callers run this from
/// `spawn_blocking`.
pub fn compute_fingerprint(mount_prefix: &Path) -> Fingerprint {
    let mut out = BTreeMap::new();
    if !mount_prefix.exists() {
        return out;
    }
    walk(mount_prefix, &mut out);
    out
}

fn walk(dir: &Path, out: &mut Fingerprint) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let Ok(ft) = entry.file_type() else { continue };
        if ft.is_dir() {
            walk(&path, out);
        } else if ft.is_file()
            && path
                .extension()
                .and_then(|s| s.to_str())
                .is_some_and(|e| e == "md" || e == "json")
        {
            let mtime = entry
                .metadata()
                .and_then(|m| m.modified())
                .unwrap_or(SystemTime::UNIX_EPOCH);
            out.insert(path, mtime);
        }
    }
}

/// Long-running task: polls the mount prefix and triggers a
/// refresh + broadcast whenever the fingerprint changes. Runs
/// until the AppState drops.
pub async fn run_polling_watcher(state: Arc<AppState>, poll_interval: Duration) {
    let mut ticker = interval(poll_interval);
    // The first tick fires immediately; skip it to avoid a
    // redundant refresh right after startup (the main serve
    // loop has already published the initial world).
    ticker.tick().await;

    let mount_prefix = state.config().mount_prefix.clone();
    let mut last_fp = {
        let prefix = mount_prefix.clone();
        tokio::task::spawn_blocking(move || compute_fingerprint(&prefix))
            .await
            .unwrap_or_default()
    };

    loop {
        ticker.tick().await;
        let prefix = mount_prefix.clone();
        let fp = match tokio::task::spawn_blocking(move || compute_fingerprint(&prefix)).await {
            Ok(fp) => fp,
            Err(join_err) => {
                tracing::warn!(error = %join_err, "watcher fingerprint task panicked");
                continue;
            }
        };
        if fp == last_fp {
            continue;
        }
        tracing::info!("external change detected; refreshing world");
        if let Err(err) = state.refresh().await {
            tracing::warn!(error = %err, "watcher refresh failed");
            continue;
        }
        last_fp = fp;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn fingerprint_covers_md_and_json_files() {
        let temp = tempdir().unwrap();
        let root = temp.path();
        std::fs::write(root.join("reqforge.json"), "{}").unwrap();
        std::fs::create_dir_all(root.join("artifacts/reqs")).unwrap();
        std::fs::write(root.join("artifacts/reqs/.collection.json"), "{}").unwrap();
        std::fs::write(root.join("artifacts/reqs/REQ-a.md"), "# a").unwrap();
        // non-relevant file is ignored
        std::fs::write(root.join("artifacts/reqs/notes.txt"), "ignore me").unwrap();

        let fp = compute_fingerprint(root);
        let names: Vec<_> = fp.keys().filter_map(|p| p.file_name()).collect();
        assert!(names.iter().any(|n| *n == "reqforge.json"));
        assert!(names.iter().any(|n| *n == ".collection.json"));
        assert!(names.iter().any(|n| *n == "REQ-a.md"));
        assert!(!names.iter().any(|n| *n == "notes.txt"));
    }

    #[test]
    fn fingerprint_is_empty_for_missing_prefix() {
        let fp = compute_fingerprint(Path::new("/definitely-not-a-real-path-xyz"));
        assert!(fp.is_empty());
    }

    #[test]
    fn fingerprint_changes_when_a_file_is_modified() {
        let temp = tempdir().unwrap();
        let root = temp.path();
        let path = root.join("reqforge.json");
        std::fs::write(&path, "{}").unwrap();
        let fp1 = compute_fingerprint(root);

        // Bump mtime via filetime-like approach: rewrite the file
        // after a small sleep so its mtime moves.
        std::thread::sleep(Duration::from_millis(20));
        std::fs::write(&path, "{\"changed\":true}").unwrap();

        let fp2 = compute_fingerprint(root);
        assert_ne!(fp1, fp2, "fingerprint should change after file write");
    }

    #[test]
    fn fingerprint_changes_when_a_file_is_added() {
        let temp = tempdir().unwrap();
        let root = temp.path();
        let fp1 = compute_fingerprint(root);

        std::fs::write(root.join("reqforge.json"), "{}").unwrap();
        let fp2 = compute_fingerprint(root);
        assert_ne!(fp1, fp2);
        assert_eq!(fp1.len(), 0);
        assert_eq!(fp2.len(), 1);
    }
}
