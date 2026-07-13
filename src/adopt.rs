//! Adopting a subject repository: derive the companion-tree name and scaffold
//! its root beside the subject's Doorstop layout (A3).
//!
//! Implements: REQ008 (propose an A3-derived companion name and scaffold the
//! mirrored companion root + manifest)

use crate::doorstop::DoorstopDoc;
use anyhow::{bail, Context, Result};
use std::path::{Path, PathBuf};

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

#[cfg(test)]
mod tests {
    use super::*;

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
}
