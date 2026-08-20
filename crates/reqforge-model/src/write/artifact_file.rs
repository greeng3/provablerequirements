//! Serialize a content-hosted artifact back to its on-disk form —
//! JSON frontmatter between `---` delimiters, then the Markdown body.
//!
//! Pretty-printed JSON with two-space indentation per
//! `FORMAT-fileEncoding`. Trailing newline on the body to match the
//! hand-authored convention. The `overflow` field survives the
//! round-trip (`FORMAT-fieldTolerance`).

use std::path::Path;

use serde_json::ser::{PrettyFormatter, Serializer};
use thiserror::Error;

use crate::schema::Artifact;
use crate::write::atomic::{AtomicWriteError, atomic_write};
use crate::write::ownership::{OwnershipError, OwnershipOverrides, reconcile_ownership};

/// Serialize `(metadata, body)` into the ReqForge on-disk format.
///
/// Shape of the bytes returned:
/// ```text
/// ---
/// {
///   "schemaVersion": 1,
///   ...
/// }
/// ---
/// # Body heading
///
/// Body paragraphs.
/// ```
pub fn render_artifact_file(metadata: &Artifact, body: &str) -> Result<Vec<u8>, RenderError> {
    let mut json = Vec::with_capacity(512);
    let formatter = PrettyFormatter::with_indent(b"  ");
    let mut ser = Serializer::with_formatter(&mut json, formatter);
    metadata
        .serialize(&mut ser)
        .map_err(RenderError::SerializeJson)?;

    let mut out = Vec::with_capacity(json.len() + body.len() + 16);
    out.extend_from_slice(b"---\n");
    out.extend_from_slice(&json);
    out.push(b'\n');
    out.extend_from_slice(b"---\n");
    out.extend_from_slice(body.as_bytes());
    if !body.ends_with('\n') {
        out.push(b'\n');
    }
    Ok(out)
}

/// Write an artifact to disk atomically and reconcile its
/// ownership to the repository's `.git` owner. This is the single
/// public entry point every CRUD handler uses.
pub fn write_artifact_file(
    target: &Path,
    repo_root: &Path,
    metadata: &Artifact,
    body: &str,
    overrides: OwnershipOverrides,
) -> Result<(), WriteArtifactError> {
    let bytes = render_artifact_file(metadata, body).map_err(WriteArtifactError::Render)?;
    atomic_write(target, &bytes).map_err(WriteArtifactError::AtomicWrite)?;
    reconcile_ownership(target, repo_root, overrides).map_err(WriteArtifactError::Ownership)?;
    Ok(())
}

#[derive(Debug, Error)]
pub enum RenderError {
    #[error("serialising artifact JSON: {0}")]
    SerializeJson(#[source] serde_json::Error),
}

#[derive(Debug, Error)]
pub enum WriteArtifactError {
    #[error(transparent)]
    Render(#[from] RenderError),

    #[error(transparent)]
    AtomicWrite(#[from] AtomicWriteError),

    #[error(transparent)]
    Ownership(#[from] OwnershipError),
}

// Pull `Serialize` into scope for the impl above without leaking
// it into the public surface.
use serde::Serialize;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::frontmatter::split_frontmatter;
    use crate::schema::{Artifact, ArtifactShape};
    use chrono::{DateTime, Utc};
    use std::collections::BTreeMap;
    use uuid::Uuid;

    fn sample_artifact() -> Artifact {
        Artifact {
            schema_version: 1,
            uuid: Uuid::parse_str("0194f6d0-0001-7000-8000-000000000001").unwrap(),
            title: "Hello World requirement".to_owned(),
            shape: ArtifactShape::Content,
            created_at: "2026-04-18T00:00:00Z".parse::<DateTime<Utc>>().unwrap(),
            modified_at: "2026-04-18T00:00:00Z".parse::<DateTime<Utc>>().unwrap(),
            links: Vec::new(),
            review_log: Vec::new(),
            description: Some("A trivial starter requirement.".to_owned()),
            expects_code_trace: None,
            active: None,
            derived: None,
            tags: None,
            outline_level: None,
            legacy: None,
            blob_path: None,
            url: None,
            checked_at: None,
            check_status: None,
            overflow: BTreeMap::new(),
        }
    }

    #[test]
    fn round_trips_through_split_frontmatter() {
        let body = "# Hello\n\nBody.\n";
        let bytes = render_artifact_file(&sample_artifact(), body).unwrap();
        let text = std::str::from_utf8(&bytes).unwrap();
        let (json, parsed_body) = split_frontmatter(text).unwrap();
        let reparsed: Artifact = serde_json::from_str(json).unwrap();
        assert_eq!(reparsed.title, "Hello World requirement");
        assert_eq!(parsed_body, body);
    }

    #[test]
    fn uses_two_space_json_indentation() {
        let bytes = render_artifact_file(&sample_artifact(), "").unwrap();
        let text = std::str::from_utf8(&bytes).unwrap();
        // Line starting with two spaces and a JSON field confirms
        // the pretty formatter's indent.
        assert!(
            text.contains("\n  \"schemaVersion\":"),
            "expected two-space-indented JSON, got:\n{text}",
        );
    }

    #[test]
    fn preserves_unknown_overflow_fields_in_the_file() {
        let mut artifact = sample_artifact();
        artifact
            .overflow
            .insert("experimentalFlag".to_owned(), serde_json::Value::Bool(true));
        let bytes = render_artifact_file(&artifact, "body\n").unwrap();
        let text = std::str::from_utf8(&bytes).unwrap();
        assert!(text.contains("\"experimentalFlag\": true"));
    }

    #[test]
    fn adds_trailing_newline_to_bodies_that_lack_one() {
        let bytes = render_artifact_file(&sample_artifact(), "no trailing newline").unwrap();
        let text = std::str::from_utf8(&bytes).unwrap();
        assert!(text.ends_with('\n'));
    }

    #[test]
    fn write_artifact_file_roundtrips_from_disk() {
        let temp = tempfile::tempdir().unwrap();
        let repo = temp.path();
        std::fs::create_dir_all(repo.join(".git")).unwrap();
        let target = repo.join("artifacts/requirements/REQ-hello.md");
        write_artifact_file(
            &target,
            repo,
            &sample_artifact(),
            "# Hello\n\nBody.\n",
            OwnershipOverrides::default(),
        )
        .unwrap();

        let text = std::fs::read_to_string(&target).unwrap();
        let (json, body) = split_frontmatter(&text).unwrap();
        let reparsed: Artifact = serde_json::from_str(json).unwrap();
        assert_eq!(reparsed.uuid, sample_artifact().uuid);
        assert!(body.starts_with("# Hello"));
    }
}
