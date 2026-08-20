//! Adopting a subject repository: derive the companion-tree name and scaffold
//! its root beside the subject's Doorstop layout (A3).
//!
//! Implements: REQ008 (propose an A3-derived companion name and scaffold the
//! mirrored companion root + manifest)

use crate::doorstop::{DoorstopDoc, DoorstopSource};
use crate::source::{Item, RequirementsSource};
use anyhow::{bail, Context, Result};
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

/// The manifest file written at the companion root, linking it back to the
/// subject's Doorstop layout.
pub const MANIFEST_FILE: &str = "provreq.yml";

/// Requirements-directory tokens recognised for name derivation, longest first
/// so `requirements` matches before the `req` it contains.
const REQ_TOKENS: [&str; 3] = ["requirements", "reqs", "req"];

/// Derive the companion-tree directory name from the subject's requirements
/// directory name: replace the requirements token with `ProvableRequirements`,
/// or prefix it when no such token is present (A3).
///
/// Implements: REQ008
pub fn companion_name(requirements_dirname: &str) -> String {
    let lower = requirements_dirname.to_ascii_lowercase();
    for token in REQ_TOKENS {
        if let Some(pos) = lower.find(token) {
            return format!(
                "{}ProvableRequirements{}",
                &requirements_dirname[..pos],
                &requirements_dirname[pos + token.len()..]
            );
        }
    }
    format!("ProvableRequirements-{requirements_dirname}")
}

/// A resolved plan for scaffolding — pure, no filesystem effects.
#[derive(Debug)]
pub struct AdoptionPlan {
    pub requirements_root: PathBuf,
    pub companion_root: PathBuf,
    pub subdirs: Vec<PathBuf>,
    pub docs: Vec<DoorstopDoc>,
}

/// Build a scaffold plan from discovered documents. `name_override` replaces the
/// derived companion name. Errors if the documents span more than one root.
pub fn plan(docs: &[DoorstopDoc], name_override: Option<&str>) -> Result<AdoptionPlan> {
    let requirements_root = docs
        .iter()
        .map(|d| d.dir.clone())
        .min_by_key(|p| p.components().count())
        .context("no Doorstop documents to plan from")?;

    // Single-root assumption: every document must nest under the shallowest one.
    // ponytail: multi-root subjects error clearly rather than guess a layout.
    for d in docs {
        if !d.dir.starts_with(&requirements_root) {
            bail!(
                "multiple independent Doorstop roots ({} and {}); \
                 init supports a single root for now",
                requirements_root.display(),
                d.dir.display()
            );
        }
    }

    let dirname = requirements_root
        .file_name()
        .and_then(|n| n.to_str())
        .context("requirements root has no directory name")?;
    let name = match name_override {
        Some(n) => n.to_string(),
        None => companion_name(dirname),
    };
    let parent = requirements_root.parent().unwrap_or(Path::new("."));
    let companion_root = parent.join(&name);

    let subdirs = docs
        .iter()
        .filter_map(|d| d.dir.strip_prefix(&requirements_root).ok())
        .filter(|rel| !rel.as_os_str().is_empty())
        .map(|rel| companion_root.join(rel))
        .collect();

    Ok(AdoptionPlan {
        requirements_root,
        companion_root,
        subdirs,
        docs: docs.to_vec(),
    })
}

#[derive(serde::Serialize)]
struct Manifest {
    schema: u32,
    /// The peer requirements directory this companion tree tracks.
    subject_requirements: String,
    documents: Vec<ManifestDoc>,
}

#[derive(serde::Serialize)]
struct ManifestDoc {
    prefix: String,
    /// Document directory relative to the requirements root (`.` for the root).
    path: String,
}

/// Create the companion tree on disk, returning its root. Errors if the root
/// already exists (never clobbers an existing tree).
///
/// Implements: REQ008
pub fn scaffold(plan: &AdoptionPlan) -> Result<PathBuf> {
    if plan.companion_root.exists() {
        bail!(
            "companion tree already exists: {}",
            plan.companion_root.display()
        );
    }
    std::fs::create_dir_all(&plan.companion_root)
        .with_context(|| format!("creating {}", plan.companion_root.display()))?;
    for sub in &plan.subdirs {
        std::fs::create_dir_all(sub).with_context(|| format!("creating {}", sub.display()))?;
    }

    let dirname = plan
        .requirements_root
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or(".");
    let documents = plan
        .docs
        .iter()
        .map(|d| {
            let rel = match d.dir.strip_prefix(&plan.requirements_root) {
                Ok(p) if !p.as_os_str().is_empty() => p.display().to_string(),
                _ => ".".to_string(),
            };
            ManifestDoc {
                prefix: d.prefix.clone(),
                path: rel,
            }
        })
        .collect();
    let manifest = Manifest {
        schema: 1,
        subject_requirements: dirname.to_string(),
        documents,
    };
    let yaml = serde_yaml::to_string(&manifest).context("serializing manifest")?;
    let manifest_path = plan.companion_root.join(MANIFEST_FILE);
    std::fs::write(&manifest_path, yaml)
        .with_context(|| format!("writing {}", manifest_path.display()))?;

    Ok(plan.companion_root.clone())
}

/// Locate the companion tree under `subject_root` by finding its `provreq.yml`
/// manifest (written by `init`). Returns `None` if the subject has not been
/// adopted yet. Prunes the same heavy directories discovery does and does not
/// follow symlinks. Single companion assumption, consistent with `init`.
pub fn find_companion(subject_root: &Path) -> Result<Option<PathBuf>> {
    let walker = WalkDir::new(subject_root)
        .follow_links(false)
        .into_iter()
        .filter_entry(|e| {
            !(e.file_type().is_dir() && crate::subject_tree::is_pruned_dir(e.path(), e.depth()))
        });
    for entry in walker {
        let entry = entry.with_context(|| format!("walking {}", subject_root.display()))?;
        if entry.file_type().is_file() && entry.file_name() == MANIFEST_FILE {
            let root = entry.path().parent().unwrap_or(subject_root).to_path_buf();
            return Ok(Some(root));
        }
    }
    Ok(None)
}

/// Resolve the adopted companion root + source items for a subject, or explain that `init` must
/// run first. Shared by the CLI commands and the `serve` backend so both reach requirements the
/// same way (through the `RequirementsSource` seam, R-src-1).
pub fn resolve(subject: &Path) -> Result<(PathBuf, Vec<Item>)> {
    let companion = find_companion(subject)?.with_context(|| {
        format!(
            "no companion tree found under {} — run `provreq init` first",
            subject.display()
        )
    })?;
    let items = source_for(subject).items()?;
    Ok((companion, items))
}

/// Which [`RequirementsSource`] a subject keeps its requirements in — the one place provreq decides
/// that, so every caller reaches requirements the same way (R-src-1).
///
/// Detected from the tree rather than configured. A ReqForge collection announces itself with a
/// [`crate::reqforge::COLLECTION_FILE`], and a subject holding one is read that way; everything
/// else is Doorstop, which stays the default because it is what foreign subjects like qrusty have
/// and will keep having — the importer is a permanent boundary, not scaffolding.
///
/// This function is the whole of what phase 1 of the ReqForge absorb had to change outside the new
/// adapter (#296). That is the claim the spike was built to test: the substrate is reachable
/// through one decision, and the engines, refusal classifications, mirror channel, and verdict
/// model never learn which side of it a requirement came from.
pub fn source_for(subject: &Path) -> Box<dyn RequirementsSource> {
    if crate::reqforge::discover(subject).is_empty() {
        Box::new(DoorstopSource::new(subject))
    } else {
        Box::new(crate::reqforge::ReqforgeSource::new(subject))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A subject holding ReqForge artifacts and an adopted companion — the shape `resolve` meets
    /// once a subject has moved off Doorstop.
    fn reqforge_subject() -> tempfile::TempDir {
        let tmp = tempfile::tempdir().unwrap();
        let fixture = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/reqforge-subject/artifacts/req");
        let dir = tmp.path().join("artifacts/req");
        std::fs::create_dir_all(&dir).unwrap();
        for name in [crate::reqforge::COLLECTION_FILE, "REQ-queueIsDrained.md"] {
            std::fs::copy(fixture.join(name), dir.join(name)).unwrap();
        }
        let companion = tmp.path().join("ProvableRequirements");
        std::fs::create_dir_all(&companion).unwrap();
        std::fs::write(companion.join(MANIFEST_FILE), "subject: .\n").unwrap();
        tmp
    }

    // Verifies: REQ009 / #296 — THE SPIKE. A requirement stored as a ReqForge artifact reaches the
    // rest of provreq through `resolve`, the one call both the CLI and the `serve` backend use, and
    // arrives as the same `Item` a Doorstop subject yields. What this had to change outside the new
    // adapter is `source_for` and nothing else: no engine adapter, no refusal classification, no
    // mirror-channel code, no verdict type. That is the claim the absorb rests on, and the spike
    // existed to falsify it rather than to confirm it.
    #[test]
    fn a_reqforge_subject_resolves_through_the_same_seam_as_a_doorstop_one() {
        let tmp = reqforge_subject();
        let (companion, items) = resolve(tmp.path()).unwrap();

        assert_eq!(companion, tmp.path().join("ProvableRequirements"));
        assert_eq!(items.len(), 1, "got {items:?}");
        assert_eq!(items[0].id, "REQ-queueIsDrained");
        assert_eq!(
            items[0].text,
            "Every message accepted onto the queue is eventually removed from it."
        );
    }

    // Verifies: REQ009 / #296 — Doorstop stays the default, because a foreign subject that never
    // heard of ReqForge must keep working. Detection is by what the tree holds, not configuration.
    #[test]
    fn a_subject_without_a_collection_is_still_read_as_doorstop() {
        let tmp = tempfile::tempdir().unwrap();
        let docs = tmp.path().join("reqs");
        std::fs::create_dir_all(&docs).unwrap();
        std::fs::write(
            docs.join(".doorstop.yml"),
            "settings:\n  prefix: REQ\n  digits: 3\n",
        )
        .unwrap();
        std::fs::write(docs.join("REQ001.yml"), "text: the only item\n").unwrap();
        let companion = tmp.path().join("ProvableRequirements");
        std::fs::create_dir_all(&companion).unwrap();
        std::fs::write(companion.join(MANIFEST_FILE), "subject: .\n").unwrap();

        let (_, items) = resolve(tmp.path()).unwrap();
        assert_eq!(items.len(), 1, "got {items:?}");
        assert_eq!(items[0].id, "REQ001");
        assert_eq!(items[0].text, "the only item");
    }

    #[test]
    fn companion_name_follows_a3_rule() {
        assert_eq!(companion_name("reqs"), "ProvableRequirements");
        assert_eq!(companion_name("requirements"), "ProvableRequirements");
        assert_eq!(companion_name("my_reqs"), "my_ProvableRequirements");
        assert_eq!(
            companion_name("requirements-doorstop"),
            "ProvableRequirements-doorstop"
        );
        // No token present → prefix fallback.
        assert_eq!(companion_name("specs"), "ProvableRequirements-specs");
    }

    fn doc(dir: &str) -> DoorstopDoc {
        DoorstopDoc {
            dir: PathBuf::from(dir),
            prefix: "REQ".into(),
            item_ids: vec!["REQ001".into()],
        }
    }

    #[test]
    fn plan_places_companion_as_peer_of_requirements_root() {
        let p = plan(&[doc("/subj/requirements-doorstop")], None).unwrap();
        assert_eq!(
            p.companion_root,
            PathBuf::from("/subj/ProvableRequirements-doorstop")
        );
        assert!(p.subdirs.is_empty());
    }

    #[test]
    fn plan_mirrors_nested_documents() {
        let docs = [doc("/subj/reqs"), doc("/subj/reqs/net")];
        let p = plan(&docs, None).unwrap();
        assert_eq!(
            p.companion_root,
            PathBuf::from("/subj/ProvableRequirements")
        );
        assert_eq!(
            p.subdirs,
            vec![PathBuf::from("/subj/ProvableRequirements/net")]
        );
    }

    #[test]
    fn plan_rejects_multiple_roots() {
        let docs = [doc("/subj/reqs"), doc("/subj/other")];
        assert!(plan(&docs, None).is_err());
    }

    // Verifies: REQ008 — scaffold creates the peer root + manifest, mirrors nesting.
    #[test]
    fn scaffold_creates_root_and_manifest() {
        let tmp = tempfile::tempdir().unwrap();
        let req_root = tmp.path().join("reqs");
        std::fs::create_dir(&req_root).unwrap();
        let docs = [DoorstopDoc {
            dir: req_root,
            prefix: "REQ".into(),
            item_ids: vec!["REQ001".into()],
        }];

        let p = plan(&docs, None).unwrap();
        let created = scaffold(&p).unwrap();
        assert_eq!(created, tmp.path().join("ProvableRequirements"));
        let manifest = std::fs::read_to_string(created.join(MANIFEST_FILE)).unwrap();
        assert!(
            manifest.contains("subject_requirements: reqs"),
            "{manifest}"
        );
        assert!(manifest.contains("prefix: REQ"), "{manifest}");

        // Re-running refuses to clobber.
        assert!(scaffold(&p).is_err());
    }

    #[test]
    fn find_companion_locates_scaffolded_manifest() {
        let tmp = tempfile::tempdir().unwrap();
        let req_root = tmp.path().join("reqs");
        std::fs::create_dir(&req_root).unwrap();
        let docs = [DoorstopDoc {
            dir: req_root,
            prefix: "REQ".into(),
            item_ids: vec![],
        }];
        let created = scaffold(&plan(&docs, None).unwrap()).unwrap();

        assert_eq!(find_companion(tmp.path()).unwrap(), Some(created));
        // A subject with no companion yet.
        let empty = tempfile::tempdir().unwrap();
        assert_eq!(find_companion(empty.path()).unwrap(), None);
    }
}
