//! Where the subject's TLA+ lives: the subject tree, plus any roots the operator configured.
//!
//! Category-2a grounding finds `.tla` files by walking the subject ([`crate::tla_adapter`]). That
//! makes a layout decision for the operator: a team whose specs live in a sibling repo, or in a
//! shared models directory, could not ground a category-2a requirement at all — every name
//! resolved to nothing and every binding parked (#120). Parking was honest, but the layout was a
//! hard limit rather than a choice.
//!
//! So `provreq.yml` may name extra roots:
//!
//! ```yaml
//! tla:
//!   spec_paths:
//!     - ../models
//! ```
//!
//! Relative roots resolve against the **subject** root, not the companion tree — the operator is
//! describing where their model lives relative to the thing being verified, which is the subject.
//!
//! Two consequences of a spec living outside the subject are handled elsewhere, and both are the
//! reason this is more than a search path:
//!
//! - **Provenance.** The subject's commit no longer covers the model, so a verdict proved against
//!   an external spec would read `fresh` forever while that spec moved underneath it. The specs
//!   themselves are fingerprinted instead ([`crate::tla_adapter::SubjectSpecs::external_fingerprint`]),
//!   and that becomes a drift axis of its own.
//! - **Where generated files go.** provreq no longer writes its module beside the spec — an
//!   external root may be a repository provreq has no business writing into. It generates into its
//!   own scratch directory and points TLC's module search path at the spec instead
//!   ([`crate::tlc::run`]).
//!
//! Implements: REQ028 (a cat-2a binding resolves against the subject's TLA+, wherever the operator
//! keeps it).

use std::path::{Path, PathBuf};

/// The roots a category-2a lookup searches **in addition to** the subject tree. Empty for a
/// subject that keeps its specs in-tree, which is every subject that worked before this existed —
/// so an unconfigured subject behaves exactly as it did.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SpecPaths {
    roots: Vec<PathBuf>,
}

impl SpecPaths {
    /// Read `tla.spec_paths` from the companion `provreq.yml`. A missing file, a missing block, or
    /// a manifest that will not parse all mean "no extra roots" — the same forgiving read as
    /// [`crate::kani::Bounds::load`], because a subject that never configured this must not be
    /// broken by the field existing.
    pub fn load(subject_root: &Path, companion_root: &Path) -> Self {
        #[derive(serde::Deserialize)]
        struct ManifestTla {
            #[serde(default)]
            tla: Option<TlaBlock>,
        }
        #[derive(serde::Deserialize)]
        struct TlaBlock {
            #[serde(default)]
            spec_paths: Vec<String>,
        }
        let Ok(text) = std::fs::read_to_string(companion_root.join(crate::adopt::MANIFEST_FILE))
        else {
            return Self::default();
        };
        let Ok(manifest) = serde_yaml::from_str::<ManifestTla>(&text) else {
            return Self::default();
        };
        let roots = manifest
            .tla
            .map(|t| t.spec_paths)
            .unwrap_or_default()
            .iter()
            .filter(|p| !p.trim().is_empty())
            .map(|p| resolve(subject_root, p.trim()))
            .collect();
        Self { roots }
    }

    /// Build from already-resolved roots — for callers that know the paths (tests, and any future
    /// caller that is not reading a manifest).
    pub fn from_roots(roots: Vec<PathBuf>) -> Self {
        Self { roots }
    }

    pub fn roots(&self) -> &[PathBuf] {
        &self.roots
    }

    pub fn is_empty(&self) -> bool {
        self.roots.is_empty()
    }
}

/// One configured root as an absolute path. Relative roots resolve against the subject, and the
/// result is canonicalized so a read-back names `/repos/models` rather than
/// `/repos/subject/../models`. A root that does not exist cannot be canonicalized; the joined path
/// is kept as-is, so the operator sees the path they configured rather than nothing at all.
fn resolve(subject_root: &Path, configured: &str) -> PathBuf {
    let joined = subject_root.join(configured);
    std::fs::canonicalize(&joined).unwrap_or(joined)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn subject_with_manifest(manifest: &str) -> tempfile::TempDir {
        let tmp = tempfile::tempdir().expect("tempdir");
        let companion = tmp.path().join("ProvableRequirements");
        std::fs::create_dir_all(&companion).expect("companion");
        std::fs::write(companion.join(crate::adopt::MANIFEST_FILE), manifest).expect("manifest");
        tmp
    }

    fn load(tmp: &tempfile::TempDir) -> SpecPaths {
        SpecPaths::load(tmp.path(), &tmp.path().join("ProvableRequirements"))
    }

    // Verifies: REQ028 (#120) — a configured root is read and resolved against the SUBJECT, so the
    // operator describes where the model lives relative to the thing being verified.
    #[test]
    fn a_configured_root_resolves_against_the_subject() {
        let tmp = subject_with_manifest("tla:\n  spec_paths:\n    - models\n");
        let models = tmp.path().join("models");
        std::fs::create_dir_all(&models).expect("models");
        let paths = load(&tmp);
        assert_eq!(paths.roots().len(), 1);
        assert_eq!(
            std::fs::canonicalize(&models).expect("canonical"),
            paths.roots()[0]
        );
    }

    // Verifies: REQ028 (#120) — a root outside the subject tree is exactly the case this exists
    // for, and it resolves without the `..` still in it.
    #[test]
    fn a_sibling_root_resolves_to_a_clean_absolute_path() {
        let parent = tempfile::tempdir().expect("tempdir");
        let subject = parent.path().join("subject");
        let companion = subject.join("ProvableRequirements");
        std::fs::create_dir_all(&companion).expect("companion");
        std::fs::create_dir_all(parent.path().join("models")).expect("models");
        std::fs::write(
            companion.join(crate::adopt::MANIFEST_FILE),
            "tla:\n  spec_paths:\n    - ../models\n",
        )
        .expect("manifest");

        let paths = SpecPaths::load(&subject, &companion);
        let root = &paths.roots()[0];
        assert!(!root.to_string_lossy().contains(".."), "{root:?}");
        assert!(root.ends_with("models"), "{root:?}");
    }

    // Verifies: REQ028 (#120) — a subject that configured nothing has no extra roots, so every
    // in-tree subject behaves exactly as it did before this existed.
    #[test]
    fn an_unconfigured_subject_has_no_extra_roots() {
        assert!(load(&subject_with_manifest("kani:\n  default_unwind: 3\n")).is_empty());
        assert!(load(&subject_with_manifest("tla: {}\n")).is_empty());
        assert!(load(&subject_with_manifest("")).is_empty());
    }

    // Verifies: REQ028 (#120) — a manifest that will not parse means no extra roots, never a
    // panic: a subject whose provreq.yml has an unrelated problem must still ground in-tree.
    #[test]
    fn an_unparseable_manifest_means_no_extra_roots() {
        assert!(load(&subject_with_manifest("tla: [this is not a map\n")).is_empty());
    }

    // Verifies: REQ028 (#120) — a configured root that does not exist keeps the path the operator
    // wrote, rather than vanishing. It contributes no specs, so bindings park honestly, and the
    // path is still there to be named.
    #[test]
    fn a_missing_root_keeps_the_configured_path() {
        let tmp = subject_with_manifest("tla:\n  spec_paths:\n    - nowhere\n");
        let paths = load(&tmp);
        assert_eq!(paths.roots().len(), 1);
        assert!(paths.roots()[0].ends_with("nowhere"));
    }

    // Verifies: REQ028 (#120) — a blank entry is not a root. An empty string would otherwise
    // resolve to the subject itself and silently re-walk the whole tree.
    #[test]
    fn a_blank_entry_is_not_a_root() {
        assert!(load(&subject_with_manifest(
            "tla:\n  spec_paths:\n    - ''\n    - '  '\n"
        ))
        .is_empty());
    }
}
