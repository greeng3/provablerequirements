//! Convert a Doorstop requirements tree into a ReqForge project, via the absorbed importer.
//!
//! This wires ReqForge's own importer — `parse::discover` → `build_plan` → `execute` — into a
//! provreq action. It writes a ReqForge project (`reqforge.json` marker plus an `artifacts/`
//! collection of `{id}.md` files) that `crate::reqforge::ReqforgeSource` reads back as the same
//! requirement items the Doorstop tree yielded. Since the importer now preserves the source uid
//! verbatim (#315), the ids don't move, so a subject migrated this way keeps every id its verdicts,
//! drafts, and code references are keyed on.
//!
//! The source tree is only read; the whole write lands under `target`.
//!
//! Implements: REQ073 (convert a Doorstop tree into a ReqForge project; #317)

use std::path::Path;

use anyhow::{Context, Result};

use reqforge_model::doorstop::{ExecuteTarget, ImportReport, build_plan, execute, parse};
use reqforge_model::load::project::LoadedProject;
use reqforge_model::schema::ProjectConfig;
use reqforge_model::write::OwnershipOverrides;

/// Convert the Doorstop tree at `source` into a ReqForge project rooted at `target`.
///
/// `slug`/`name` identify the resulting ReqForge project. Returns the importer's report (collections
/// and artifacts written, refs classified) so a caller can show what happened.
pub fn migrate_doorstop(
    source: &Path,
    target: &Path,
    slug: &str,
    name: &str,
) -> Result<ImportReport> {
    let documents = parse::discover(source)
        .with_context(|| format!("discovering the doorstop tree at {}", source.display()))?;

    let config = ProjectConfig {
        schema_version: 1,
        slug: slug.to_string(),
        name: name.to_string(),
        description: None,
        // Default `artifacts/` collection root — the location `ReqforgeSource` discovers by its
        // `.collection.json` marker, so nothing else has to be told where the requirements went.
        artifacts_path: None,
        scan_paths: None,
        overflow: Default::default(),
    };
    let project = LoadedProject {
        root: target.to_path_buf(),
        // Fresh target: no existing collections for the importer's prefix-collision check to hit.
        collections: Vec::new(),
        diagnostics: Vec::new(),
        config,
    };

    let artifacts_root = project.root.join(project.config.effective_artifacts_path());
    std::fs::create_dir_all(&artifacts_root)
        .with_context(|| format!("creating {}", artifacts_root.display()))?;
    write_project_marker(&project)?;

    let plan =
        build_plan(&project, documents, chrono::Utc::now()).context("building the import plan")?;
    let exec_target = ExecuteTarget::from_project(&project);
    let report = execute(
        &exec_target,
        &format!("doorstop:{}", source.display()),
        plan,
        // Own the written files like the source tree, which skips the `.git`-owner lookup the
        // reconcile would otherwise do against the fresh (git-less) target root.
        ownership_overrides(source),
    )
    .context("writing the ReqForge project")?;
    Ok(report)
}

/// Write the `reqforge.json` project marker at the project root, so the target is a recognisable
/// ReqForge project and not just a loose collection.
fn write_project_marker(project: &LoadedProject) -> Result<()> {
    let path = project.root.join("reqforge.json");
    let mut bytes =
        serde_json::to_vec_pretty(&project.config).context("serializing reqforge.json")?;
    bytes.push(b'\n');
    std::fs::write(&path, &bytes).with_context(|| format!("writing {}", path.display()))
}

#[cfg(unix)]
fn ownership_overrides(reference: &Path) -> OwnershipOverrides {
    use std::os::unix::fs::MetadataExt;
    match std::fs::metadata(reference) {
        Ok(m) => OwnershipOverrides {
            uid: Some(m.uid()),
            gid: Some(m.gid()),
        },
        Err(_) => OwnershipOverrides::default(),
    }
}

#[cfg(not(unix))]
fn ownership_overrides(_reference: &Path) -> OwnershipOverrides {
    // Ownership reconciliation is a no-op off Unix, so the overrides are never consulted.
    OwnershipOverrides::default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::doorstop::DoorstopSource;
    use crate::reqforge::ReqforgeSource;
    use crate::source::RequirementsSource;

    /// A minimal doorstop tree matching provreq's own settings (prefix `REQ`, empty sep).
    fn doorstop_tree() -> tempfile::TempDir {
        let tmp = tempfile::tempdir().unwrap();
        let doc = tmp.path().join("reqs");
        std::fs::create_dir(&doc).unwrap();
        std::fs::write(
            doc.join(".doorstop.yml"),
            "settings:\n  prefix: REQ\n  sep: ''\n  digits: 3\n",
        )
        .unwrap();
        std::fs::write(doc.join("REQ001.yml"), "text: |\n  the first item\n").unwrap();
        std::fs::write(doc.join("REQ002.yml"), "text: |\n  the second item\n").unwrap();
        tmp
    }

    // Verifies: #317 — a migrated doorstop tree is read back by the ReqForge adapter as the same
    // requirement items (ids + prose) the Doorstop adapter yielded. This is the round-trip the whole
    // self-migration rests on: if an id or a line of prose moved, every verdict and code reference
    // keyed on it would silently stale.
    #[test]
    fn migration_round_trips_ids_and_prose() {
        let src = doorstop_tree();
        let target = tempfile::tempdir().unwrap();

        migrate_doorstop(
            src.path(),
            target.path(),
            "provreq",
            "Provable Requirements",
        )
        .unwrap();

        // The project marker and a discoverable collection exist.
        assert!(target.path().join("reqforge.json").is_file());

        let before = DoorstopSource::new(src.path()).items().unwrap();
        let after = ReqforgeSource::new(target.path()).items().unwrap();

        let ids_before: Vec<&str> = before.iter().map(|i| i.id.as_str()).collect();
        let ids_after: Vec<&str> = after.iter().map(|i| i.id.as_str()).collect();
        assert_eq!(ids_after, ids_before, "ids must survive verbatim");
        assert_eq!(ids_after, ["REQ001", "REQ002"]);

        for (b, a) in before.iter().zip(after.iter()) {
            assert_eq!(a.text, b.text, "prose for {} must round-trip", b.id);
        }
    }
}
