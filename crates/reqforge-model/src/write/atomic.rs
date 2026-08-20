//! Atomic file writes per `STOR-atomicWrites`.
//!
//! Every managed write goes to a sibling temporary file in the
//! same directory as the target, gets fsynced, and is then
//! renamed into place. A reader (including another ReqForge
//! session or any other process) never observes a partially-
//! written file regardless of when the writing process crashes,
//! is killed, or loses power.

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use tempfile::NamedTempFile;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum AtomicWriteError {
    #[error("target path has no parent directory: {}", path.display())]
    NoParent { path: PathBuf },

    #[error("creating temp file in {}: {source}", dir.display())]
    CreateTemp {
        dir: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("writing to temp file for {}: {source}", target.display())]
    Write {
        target: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("fsync on temp file for {}: {source}", target.display())]
    Fsync {
        target: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("renaming temp file into {}: {source}", target.display())]
    Rename {
        target: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("ensuring parent directory {}: {source}", dir.display())]
    EnsureParent {
        dir: PathBuf,
        #[source]
        source: std::io::Error,
    },
}

/// Atomically write `bytes` to `path`.
///
/// Steps: create a sibling temp file in `path.parent()`, write
/// the bytes, fsync the file, then rename the temp file over the
/// target. The rename is atomic on all POSIX filesystems ReqForge
/// targets; on Windows the same-directory rename is also atomic.
///
/// The parent directory is created (recursively) if it does not
/// already exist — callers don't have to pre-create collection
/// directories before writing artifacts into them.
pub fn atomic_write(path: &Path, bytes: &[u8]) -> Result<(), AtomicWriteError> {
    atomic_write_with_mode(path, bytes, 0o644)
}

/// Atomically write `bytes` to `path` with a specific POSIX file
/// mode. On Windows the `mode` argument is ignored; the file
/// inherits whatever ACLs apply to its parent directory.
///
/// Use case: System config files containing API keys (Phase 13)
/// land at mode 0600 so a stray group / other read bit can't
/// leak the secret.
pub fn atomic_write_with_mode(
    path: &Path,
    bytes: &[u8],
    mode: u32,
) -> Result<(), AtomicWriteError> {
    let parent = path.parent().ok_or_else(|| AtomicWriteError::NoParent {
        path: path.to_path_buf(),
    })?;
    let dir: &Path = if parent.as_os_str().is_empty() {
        Path::new(".")
    } else {
        parent
    };

    if !dir.exists() {
        fs::create_dir_all(dir).map_err(|source| AtomicWriteError::EnsureParent {
            dir: dir.to_path_buf(),
            source,
        })?;
    }

    let mut tmp = NamedTempFile::new_in(dir).map_err(|source| AtomicWriteError::CreateTemp {
        dir: dir.to_path_buf(),
        source,
    })?;
    tmp.write_all(bytes)
        .map_err(|source| AtomicWriteError::Write {
            target: path.to_path_buf(),
            source,
        })?;
    tmp.as_file()
        .sync_all()
        .map_err(|source| AtomicWriteError::Fsync {
            target: path.to_path_buf(),
            source,
        })?;

    // `tempfile::NamedTempFile` creates the temp with mode 0600
    // (mkstemp(3) semantics — safe for secret-ish scratch data).
    // But the file we're about to persist is going into a git
    // checkout that other users need to read: the developer on
    // the host, CI jobs running under a different UID, and the
    // exact case Risk Check 5 hit on userns-remap'd Docker where
    // the container's subuid wrote files that host UID 1000
    // couldn't even stat. Relax to 0644 before the rename so the
    // persisted artifact + sidecar files follow the "checked-in
    // source file" convention that git repos expect, independent
    // of the process that wrote them.
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = tmp
            .as_file()
            .metadata()
            .map_err(|source| AtomicWriteError::Fsync {
                target: path.to_path_buf(),
                source,
            })?
            .permissions();
        perms.set_mode(mode);
        let _ = mode; // silence unused on non-unix builds (compiler is fine with cfg branch but keep parity)
        tmp.as_file()
            .set_permissions(perms)
            .map_err(|source| AtomicWriteError::Fsync {
                target: path.to_path_buf(),
                source,
            })?;
    }

    tmp.persist(path).map_err(|err| AtomicWriteError::Rename {
        target: path.to_path_buf(),
        source: err.error,
    })?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn writes_bytes_atomically_to_new_path() {
        let temp = tempfile::tempdir().unwrap();
        let target = temp.path().join("artifact.md");
        atomic_write(&target, b"hello").unwrap();
        assert_eq!(fs::read(&target).unwrap(), b"hello");
    }

    #[cfg(unix)]
    #[test]
    fn persists_files_with_mode_0644_for_cross_user_readability() {
        use std::os::unix::fs::PermissionsExt;
        let temp = tempfile::tempdir().unwrap();
        let target = temp.path().join("artifact.md");
        atomic_write(&target, b"hello").unwrap();
        let mode = fs::metadata(&target).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o644, "artifact file should be 0644, got {:o}", mode);
    }

    #[test]
    fn overwrites_existing_file_in_place() {
        let temp = tempfile::tempdir().unwrap();
        let target = temp.path().join("artifact.md");
        fs::write(&target, b"old contents").unwrap();
        atomic_write(&target, b"new contents").unwrap();
        assert_eq!(fs::read(&target).unwrap(), b"new contents");
    }

    #[test]
    fn creates_missing_parent_directories() {
        let temp = tempfile::tempdir().unwrap();
        let target = temp.path().join("a/b/c/artifact.md");
        atomic_write(&target, b"deep").unwrap();
        assert_eq!(fs::read(&target).unwrap(), b"deep");
    }

    #[test]
    fn temp_file_is_cleaned_up_on_success() {
        let temp = tempfile::tempdir().unwrap();
        let target = temp.path().join("artifact.md");
        atomic_write(&target, b"x").unwrap();

        // After a successful atomic_write the directory should
        // contain exactly one file — the target. tempfile's
        // persist() renames the temp file into place, so no
        // stray siblings are left behind.
        let entries: Vec<_> = fs::read_dir(temp.path())
            .unwrap()
            .map(|e| e.unwrap().file_name())
            .collect();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0], "artifact.md");
    }

    #[test]
    fn partial_writes_are_not_visible_under_concurrent_reads() {
        // Race a writer against a reader. The reader must either
        // see the old contents or the new contents — never a
        // partial write. Guaranteed by the rename-in-place, which
        // is why this test is a sanity check rather than a
        // tightly-timed race.
        use std::sync::Arc;
        use std::sync::atomic::{AtomicBool, Ordering};
        use std::thread;

        let temp = tempfile::tempdir().unwrap();
        let target = Arc::new(temp.path().join("artifact.md"));
        fs::write(target.as_path(), b"old-content-exactly-this").unwrap();

        let stop = Arc::new(AtomicBool::new(false));
        let reader_target = target.clone();
        let reader_stop = stop.clone();
        let reader = thread::spawn(move || {
            while !reader_stop.load(Ordering::Relaxed) {
                if let Ok(bytes) = fs::read(reader_target.as_path()) {
                    assert!(
                        bytes == b"old-content-exactly-this"
                            || bytes == b"new-content-exactly-this",
                        "reader observed a partial write: {bytes:?}"
                    );
                }
            }
        });

        for _ in 0..32 {
            atomic_write(target.as_path(), b"new-content-exactly-this").unwrap();
            atomic_write(target.as_path(), b"old-content-exactly-this").unwrap();
        }
        stop.store(true, Ordering::Relaxed);
        reader.join().unwrap();
    }
}
