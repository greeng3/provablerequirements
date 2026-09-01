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

use anyhow::{Result, bail};
use chrono::Utc;
use uuid::Uuid;

use reqforge_model::schema::{Artifact, ArtifactShape};
use reqforge_model::write::{atomic_write, render_artifact_file};

/// Author requirement `id` with `title` and `prose` into the subject's ReqForge collection. Returns
/// the path written. With one collection it authors there; with several it selects the one whose
/// prefix matches the id's (`ART001` → the `ART` collection, #410). Refuses when the id already
/// exists, when there is no collection, or when the id's prefix matches none.
pub fn create(subject: &Path, id: &str, title: &str, prose: &str) -> Result<PathBuf> {
    let req_root = crate::adopt::requirements_root(subject);
    let mut collections = crate::reqforge::discover(&req_root);
    let dir = match collections.len() {
        1 => collections.pop().expect("length checked"),
        0 => bail!(
            "no ReqForge collection under {} to author into",
            req_root.display()
        ),
        // More than one collection: route by the id's alphabetic prefix (`ART001` → the collection
        // whose `.collection.json` prefix is `ART`), so a multi-collection project authors into the
        // right place instead of refusing (#410).
        _ => {
            let id_prefix: String = id.chars().take_while(|c| c.is_ascii_alphabetic()).collect();
            if id_prefix.is_empty() {
                bail!("id `{id}` has no alphabetic prefix to select a collection by");
            }
            let mut matched: Vec<PathBuf> = collections
                .into_iter()
                .filter(|dir| {
                    collection_prefix(dir).is_some_and(|p| p.eq_ignore_ascii_case(&id_prefix))
                })
                .collect();
            match matched.len() {
                1 => matched.pop().expect("length checked"),
                0 => bail!(
                    "no collection with prefix `{id_prefix}` under {} to author `{id}` into",
                    req_root.display()
                ),
                n => bail!("{n} collections share the prefix `{id_prefix}` — ambiguous"),
            }
        }
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

/// The declared prefix of the collection rooted at `dir`, read from its `.collection.json`. Returns
/// `None` when the config is missing or unreadable — such a directory simply does not match any id
/// prefix, so a bad config narrows the routing rather than aborting the author.
fn collection_prefix(dir: &Path) -> Option<String> {
    let text = std::fs::read_to_string(dir.join(crate::reqforge::COLLECTION_FILE)).ok()?;
    let config: reqforge_model::schema::CollectionConfig = serde_json::from_str(&text).ok()?;
    Some(config.prefix)
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

    /// The single-collection subject with a second, native collection (prefix `ART`) added beside
    /// the migrated `req` one — the multi-collection shape `create` must now route within.
    fn two_collection_subject() -> tempfile::TempDir {
        let tmp = subject();
        let art = crate::adopt::requirements_root(tmp.path()).join("artifacts/art");
        std::fs::create_dir_all(&art).unwrap();
        std::fs::write(
            art.join(crate::reqforge::COLLECTION_FILE),
            r#"{"schemaVersion":1,"prefix":"ART","name":"Artifact model"}"#,
        )
        .unwrap();
        tmp
    }

    // Verifies: REQ074 / #410 — with more than one collection, an id routes to the collection whose
    // prefix it carries; a `REQ` id still lands in `req`, an `ART` id in `art`.
    #[test]
    fn routes_by_id_prefix_when_multiple_collections() {
        let tmp = two_collection_subject();

        let art = create(tmp.path(), "ART001", "Shapes", "Three shapes, one graph.").unwrap();
        assert!(
            art.ends_with("artifacts/art/ART001.md"),
            "ART id authored into the art collection: {}",
            art.display()
        );

        let req = create(tmp.path(), "REQ002", "Another", "x").unwrap();
        assert!(
            req.ends_with("artifacts/req/REQ002.md"),
            "REQ id still authored into the req collection: {}",
            req.display()
        );
    }

    // Verifies: REQ074 / #410 — an id whose prefix matches no collection is refused, not misfiled.
    #[test]
    fn errors_when_no_collection_matches_the_prefix() {
        let tmp = two_collection_subject();
        let err = create(tmp.path(), "XYZ001", "no home", "x")
            .unwrap_err()
            .to_string();
        assert!(err.contains("prefix"), "got: {err}");
    }
}
