//! Persistence side effects of a successful review write.
//!
//! Two things can fall out of `POST /api/artifacts/:uuid/reviews`
//! beyond appending to the review log:
//!
//! - Appending a new reviewer identity to
//!   `<workspace>/reviewers.json` so it shows up in future
//!   dropdowns (per `REVIEW-reviewerIdentity`).
//! - Writing an approval snapshot to
//!   `<workspace>/review-snapshots/<uuid>/<ts>/artifact.md` so the
//!   Phase 4c "since last approval" diff has a before-image to
//!   compare against.
//!
//! Both are best-effort: a failure here logs a warning and returns
//! an error to the caller, but the review itself has already
//! landed on disk and been broadcast.

use std::fs;
use std::path::{Path, PathBuf};

use chrono::{DateTime, Utc};
use uuid::Uuid;

use crate::reviews::identity::{ReviewersFile, ReviewersFileError, load_reviewers_json};
use crate::write::{AtomicWriteError, atomic_write};

/// The Phase-4a identity loader returns `ReviewersFile`; keep the
/// append path typed against the same shape so round-trips preserve
/// any `overflow` fields.
pub fn append_reviewer_if_missing(
    workspace_dir: &Path,
    reviewer: &str,
) -> Result<bool, ReviewerPersistenceError> {
    let trimmed = reviewer.trim();
    if trimmed.is_empty() {
        return Ok(false);
    }
    ensure_dir_exists(workspace_dir)?;

    let path = workspace_dir.join("reviewers.json");
    let mut current: ReviewersFile =
        load_reviewers_json(&path).map_err(ReviewerPersistenceError::Load)?;
    if current.reviewers.iter().any(|r| r == trimmed) {
        return Ok(false);
    }
    current.reviewers.push(trimmed.to_owned());

    let mut bytes =
        serde_json::to_vec_pretty(&current).map_err(ReviewerPersistenceError::Serialize)?;
    bytes.push(b'\n');
    atomic_write(&path, &bytes)?;
    Ok(true)
}

/// Write an approval snapshot under
/// `<workspace>/review-snapshots/<uuid>/<timestamp>/artifact.md`.
///
/// The timestamp is formatted without colons (`20260421T120000Z`)
/// so the path is portable across filesystems that rejected the
/// `:` character.
pub fn write_approval_snapshot(
    workspace_dir: &Path,
    artifact_uuid: Uuid,
    approved_at: DateTime<Utc>,
    frontmatter_json: &str,
    body: &str,
) -> Result<PathBuf, ReviewerPersistenceError> {
    let dir = workspace_dir
        .join("review-snapshots")
        .join(artifact_uuid.to_string())
        .join(format_snapshot_timestamp(approved_at));
    fs::create_dir_all(&dir).map_err(|source| ReviewerPersistenceError::Io {
        path: dir.clone(),
        source,
    })?;
    let file = dir.join("artifact.md");
    let contents = format!("---\n{frontmatter_json}\n---\n{body}");
    atomic_write(&file, contents.as_bytes())?;
    Ok(file)
}

/// Filesystem-portable timestamp encoding for snapshot directory
/// names: `YYYYMMDDTHHMMSSZ`.
pub fn format_snapshot_timestamp(ts: DateTime<Utc>) -> String {
    ts.format("%Y%m%dT%H%M%SZ").to_string()
}

/// Load the most recent approval snapshot for `artifact_uuid`, if
/// one exists. Returns `Ok(None)` when the artifact has never been
/// approved (no directory) or has an empty snapshot directory.
///
/// The content is returned verbatim — callers split the
/// frontmatter / body boundary themselves (or pass the raw string
/// through to the client for the Phase 4c diff UI).
pub fn load_latest_approval_snapshot(
    workspace_dir: &Path,
    artifact_uuid: Uuid,
) -> Result<Option<LoadedSnapshot>, ReviewerPersistenceError> {
    let dir = workspace_dir
        .join("review-snapshots")
        .join(artifact_uuid.to_string());
    if !dir.exists() {
        return Ok(None);
    }
    let mut timestamps: Vec<String> = Vec::new();
    for entry in fs::read_dir(&dir).map_err(|source| ReviewerPersistenceError::Io {
        path: dir.clone(),
        source,
    })? {
        let entry = entry.map_err(|source| ReviewerPersistenceError::Io {
            path: dir.clone(),
            source,
        })?;
        if entry.file_type().map(|t| t.is_dir()).unwrap_or(false)
            && let Some(name) = entry.file_name().to_str()
        {
            timestamps.push(name.to_owned());
        }
    }
    // The timestamp encoding sorts lexicographically in time
    // order, so the last entry is the most recent snapshot.
    timestamps.sort();
    let Some(latest) = timestamps.pop() else {
        return Ok(None);
    };
    let file = dir.join(&latest).join("artifact.md");
    let contents = fs::read_to_string(&file).map_err(|source| ReviewerPersistenceError::Io {
        path: file.clone(),
        source,
    })?;
    let (frontmatter, body) = split_frontmatter_body(&contents);
    let approved_at = parse_snapshot_timestamp(&latest);
    Ok(Some(LoadedSnapshot {
        approved_at,
        frontmatter_json: frontmatter,
        body,
    }))
}

/// A loaded approval snapshot: the parsed approval-time
/// timestamp, the verbatim JSON frontmatter string, and the body.
#[derive(Debug, Clone)]
pub struct LoadedSnapshot {
    pub approved_at: DateTime<Utc>,
    pub frontmatter_json: String,
    pub body: String,
}

fn split_frontmatter_body(contents: &str) -> (String, String) {
    // Expect `---\n<json>\n---\n<body>` — the exact layout
    // `write_approval_snapshot` emits. If the file is malformed
    // (e.g. hand-edited), fall through to returning the raw text
    // as the body; the caller still gets *something* to render.
    let Some(rest) = contents.strip_prefix("---\n") else {
        return (String::new(), contents.to_owned());
    };
    let Some(end) = rest.find("\n---\n") else {
        return (String::new(), contents.to_owned());
    };
    let frontmatter = &rest[..end];
    let body = &rest[end + "\n---\n".len()..];
    (frontmatter.to_owned(), body.to_owned())
}

fn parse_snapshot_timestamp(dir_name: &str) -> DateTime<Utc> {
    // The snapshot directory name is `YYYYMMDDTHHMMSSZ`; convert
    // to RFC-3339 for chrono. If parsing fails (directory
    // hand-created?) fall back to `Utc::now()` so the handler
    // still returns something sensible.
    let rfc = format!(
        "{}-{}-{}T{}:{}:{}Z",
        dir_name.get(0..4).unwrap_or("1970"),
        dir_name.get(4..6).unwrap_or("01"),
        dir_name.get(6..8).unwrap_or("01"),
        dir_name.get(9..11).unwrap_or("00"),
        dir_name.get(11..13).unwrap_or("00"),
        dir_name.get(13..15).unwrap_or("00"),
    );
    DateTime::parse_from_rfc3339(&rfc)
        .map(|dt| dt.with_timezone(&Utc))
        .unwrap_or_else(|_| Utc::now())
}

fn ensure_dir_exists(dir: &Path) -> Result<(), ReviewerPersistenceError> {
    fs::create_dir_all(dir).map_err(|source| ReviewerPersistenceError::Io {
        path: dir.to_path_buf(),
        source,
    })
}

#[derive(Debug, thiserror::Error)]
pub enum ReviewerPersistenceError {
    #[error("i/o error at {}: {source}", path.display())]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("loading reviewers.json failed: {0}")]
    Load(#[source] ReviewersFileError),
    #[error(transparent)]
    AtomicWrite(#[from] AtomicWriteError),
    #[error("serializing reviewers.json failed: {0}")]
    Serialize(#[source] serde_json::Error),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn append_creates_file_when_absent() {
        let temp = tempfile::tempdir().unwrap();
        let appended = append_reviewer_if_missing(temp.path(), "alice").unwrap();
        assert!(appended);
        let loaded = load_reviewers_json(&temp.path().join("reviewers.json")).unwrap();
        assert_eq!(loaded.reviewers, vec!["alice".to_owned()]);
    }

    #[test]
    fn append_is_noop_when_reviewer_already_persisted() {
        let temp = tempfile::tempdir().unwrap();
        append_reviewer_if_missing(temp.path(), "alice").unwrap();
        let appended = append_reviewer_if_missing(temp.path(), "alice").unwrap();
        assert!(!appended);
    }

    #[test]
    fn append_preserves_prior_reviewers_and_unknown_fields() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("reviewers.json");
        std::fs::write(
            &path,
            serde_json::json!({
                "reviewers": ["alice"],
                "futureField": true
            })
            .to_string(),
        )
        .unwrap();
        append_reviewer_if_missing(temp.path(), "bob").unwrap();
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(text.contains("\"alice\""));
        assert!(text.contains("\"bob\""));
        assert!(text.contains("\"futureField\""));
    }

    #[test]
    fn append_ignores_whitespace_only_identities() {
        let temp = tempfile::tempdir().unwrap();
        let appended = append_reviewer_if_missing(temp.path(), "   ").unwrap();
        assert!(!appended);
        assert!(!temp.path().join("reviewers.json").exists());
    }

    #[test]
    fn snapshot_writes_under_review_snapshots_tree() {
        let temp = tempfile::tempdir().unwrap();
        let uuid = Uuid::now_v7();
        let ts = DateTime::parse_from_rfc3339("2026-04-21T12:00:00Z")
            .unwrap()
            .with_timezone(&Utc);
        let file =
            write_approval_snapshot(temp.path(), uuid, ts, "{\"schemaVersion\":1}", "# Body\n")
                .unwrap();
        assert!(file.exists());
        assert!(
            file.to_string_lossy()
                .contains(&format!("review-snapshots/{uuid}/20260421T120000Z/"))
        );
        let contents = std::fs::read_to_string(&file).unwrap();
        assert!(contents.starts_with("---\n{\"schemaVersion\":1}\n---\n# Body"));
    }

    #[test]
    fn snapshot_timestamp_format_is_filesystem_portable() {
        let ts = DateTime::parse_from_rfc3339("2026-04-21T12:00:00Z")
            .unwrap()
            .with_timezone(&Utc);
        assert_eq!(format_snapshot_timestamp(ts), "20260421T120000Z");
    }
}
