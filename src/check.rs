//! Validate a subject's ReqForge requirements project — the analogue of `doorstop -e` now that
//! provreq's own requirements live in a ReqForge collection (#321).
//!
//! Loads the project through `reqforge-model` and promotes every soft diagnostic to an error, the
//! way `doorstop -e` promoted doorstop's warnings: a requirement tree that loads with warnings is
//! not validated. Catches schema errors, missing or invalid collection configs, unloadable
//! artifacts, files whose `schemaVersion` is newer than this build, and two artifacts sharing a
//! uuid.

use std::path::Path;

use anyhow::{bail, Context, Result};

use reqforge_model::index::build_uuid_index;
use reqforge_model::load::load_project;

/// Validate the subject's ReqForge requirements project. Returns the number of artifacts checked on
/// success; errors (after reporting each problem) when any diagnostic or duplicate uuid is present.
pub fn check(subject: &Path) -> Result<usize> {
    let req_root = crate::adopt::requirements_root(subject);
    let project = load_project(&req_root).with_context(|| {
        format!(
            "loading the ReqForge requirements project at {}",
            req_root.display()
        )
    })?;
    let (_index, duplicates) = build_uuid_index(&[&project]);
    let artifact_count: usize = project.collections.iter().map(|c| c.artifacts.len()).sum();

    if project.diagnostics.is_empty() && duplicates.is_empty() {
        return Ok(artifact_count);
    }

    for diagnostic in &project.diagnostics {
        eprintln!("  {diagnostic:?}");
    }
    for duplicate in &duplicates {
        eprintln!("  {duplicate:?}");
    }
    bail!(
        "requirements project failed validation: {} diagnostic(s), {} duplicate uuid(s)",
        project.diagnostics.len(),
        duplicates.len()
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A subject whose requirements live in a freshly-migrated ReqForge project at `proj/`, with a
    /// companion declaring it. Built through `migrate_doorstop` so the fixture is a real project the
    /// importer wrote, not a hand-guess at the format.
    fn subject_with_reqforge_project() -> tempfile::TempDir {
        let tmp = tempfile::tempdir().unwrap();
        let src = tmp.path().join("src/reqs");
        std::fs::create_dir_all(&src).unwrap();
        std::fs::write(
            src.join(".doorstop.yml"),
            "settings:\n  prefix: REQ\n  sep: ''\n",
        )
        .unwrap();
        std::fs::write(src.join("REQ001.yml"), "text: one\n").unwrap();
        std::fs::write(src.join("REQ002.yml"), "text: two\n").unwrap();
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

    // Verifies: #323 — a clean ReqForge project validates and reports its artifact count.
    #[test]
    fn check_passes_a_clean_project() {
        let tmp = subject_with_reqforge_project();
        assert_eq!(check(tmp.path()).unwrap(), 2);
    }

    // Verifies: #323 — an unloadable artifact is a validation failure, not a silently-skipped file.
    // The loader accumulates it as a soft diagnostic; the gate promotes that to an error.
    #[test]
    fn check_fails_on_an_unloadable_artifact() {
        let tmp = subject_with_reqforge_project();
        let artifact = tmp.path().join("proj/artifacts/req/REQ001.md");
        std::fs::write(&artifact, "---\nnot valid json frontmatter\n---\nbody\n").unwrap();
        assert!(check(tmp.path()).is_err());
    }
}
