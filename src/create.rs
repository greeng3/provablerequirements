//! Author a new requirement as a ReqForge artifact (#325).
//!
//! The self-migration (#321) left provreq able to import and read requirements but with no way to
//! *create* one — creation was UI / LLM / doorstop-import only. This is the CLI path: an id, a
//! title, and prose become a valid ReqForge artifact with a freshly minted uuid.
//!
//! The artifact arrives **unreviewed** (an empty review log). That is the absorb's trust rule: a
//! doorstop import carries a human baseline and is auto-approved, but a requirement authored here is
//! new prose nobody has reviewed, so it must pass through the review workflow like any other.
//!
//! Implements: REQ074 (author a requirement into the subject's ReqForge collection)

use std::path::{Path, PathBuf};

use anyhow::{bail, Result};
use chrono::Utc;
use uuid::Uuid;

use reqforge_model::schema::{Artifact, ArtifactShape};
use reqforge_model::write::{atomic_write, render_artifact_file};

/// Author requirement `id` with `title` and `prose` into the subject's ReqForge collection. Returns
/// the path written. Refuses when the id already exists, or when the subject does not have exactly
/// one collection to author into.
pub fn create(subject: &Path, id: &str, title: &str, prose: &str) -> Result<PathBuf> {
    let req_root = crate::adopt::requirements_root(subject);
    let mut collections = crate::reqforge::discover(&req_root);
    let dir = match collections.len() {
        1 => collections.pop().expect("length checked"),
        0 => bail!(
            "no ReqForge collection under {} to author into",
            req_root.display()
        ),
        n => bail!("{n} collections under {}; which one is ambiguous — author by editing the artifact directly for now", req_root.display()),
    };

    let path = dir.join(format!("{id}.md"));
    if path.exists() {
        bail!("{id} already exists at {}", path.display());
    }

    let now = Utc::now();
    let artifact = Artifact {
        schema_version: 1,
        uuid: Uuid::now_v7(),
        title: title.to_string(),
        shape: ArtifactShape::Content,
        created_at: now,
        modified_at: now,
        links: Vec::new(),
        // Unreviewed: authored prose nobody has confirmed. The absorb's trust rule (#296).
        review_log: Vec::new(),
        description: None,
        expects_code_trace: None,
        active: Some(true),
        derived: Some(false),
        tags: None,
        outline_level: None,
        legacy: None,
        blob_path: None,
        url: None,
        checked_at: None,
        check_status: None,
        overflow: Default::default(),
    };

    let bytes = render_artifact_file(&artifact, prose)?;
    atomic_write(&path, &bytes)?;
    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::reqforge::ReqforgeSource;
    use crate::source::RequirementsSource;

    /// A subject whose requirements live in a freshly-migrated ReqForge project, with a companion
    /// declaring it — the shape `create` writes into.
    fn subject() -> tempfile::TempDir {
        let tmp = tempfile::tempdir().unwrap();
        let src = tmp.path().join("src/reqs");
        std::fs::create_dir_all(&src).unwrap();
        std::fs::write(
            src.join(".doorstop.yml"),
            "settings:\n  prefix: REQ\n  sep: ''\n",
        )
        .unwrap();
        std::fs::write(src.join("REQ001.yml"), "text: one\n").unwrap();
        crate::migrate::migrate_doorstop(
            &tmp.path().join("src"),
            &tmp.path().join("proj"),
            "p",
            "P",
        )
        .unwrap();
        let companion = tmp.path().join("Companion");
        std::fs::create_dir_all(&companion).unwrap();
        std::fs::write(
            companion.join(crate::adopt::MANIFEST_FILE),
            "subject_requirements: proj\n",
        )
        .unwrap();
        tmp
    }

    // Verifies: REQ074 / #325 — an authored requirement is read back through the seam with its id,
    // prose, and title, and arrives unreviewed.
    #[test]
    fn authors_a_readable_unreviewed_requirement() {
        let tmp = subject();
        create(
            tmp.path(),
            "REQ002",
            "A new thing",
            "The system shall do the new thing.",
        )
        .unwrap();

        let items = ReqforgeSource::new(crate::adopt::requirements_root(tmp.path()))
            .items()
            .unwrap();
        let new = items
            .iter()
            .find(|i| i.id == "REQ002")
            .expect("REQ002 present");
        assert_eq!(new.text, "The system shall do the new thing.");
        assert_eq!(new.title.as_deref(), Some("A new thing"));

        // Unreviewed: the written artifact carries an empty review log.
        let loaded = reqforge_model::load::artifact::load_content_artifact(
            &crate::adopt::requirements_root(tmp.path()).join("artifacts/req/REQ002.md"),
        )
        .unwrap();
        assert!(
            loaded.metadata.review_log.is_empty(),
            "authored requirements are unreviewed"
        );
    }

    // Verifies: REQ074 / #325 — authoring over an existing id refuses rather than clobbering it.
    #[test]
    fn refuses_to_overwrite_an_existing_id() {
        let tmp = subject();
        let err = create(tmp.path(), "REQ001", "dup", "x")
            .unwrap_err()
            .to_string();
        assert!(err.contains("already exists"), "got: {err}");
    }
}
