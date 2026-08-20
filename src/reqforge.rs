//! The ReqForge [`RequirementsSource`] adapter (adapter #2) — the spike for #296.
//!
//! [`crate::source`] drew this seam single-implementation and said why: "the reqforge adapter is a
//! real, not-speculative second consumer that lands when its format stabilises." This is that
//! adapter, and its job in phase 1 of the absorb is not to be complete. It is to **falsify** the
//! claim the whole plan rests on — that provreq's verification does not know what the requirement
//! model is — by sourcing a requirement from ReqForge's on-disk shape and seeing what that drags
//! in. If an engine adapter, a refusal classification, the mirror channel, or the verdict model
//! has to move, the reading is wrong and phase 2 is mis-scoped.
//!
//! **No dependency on ReqForge's code**, deliberately. There is no `reqforge-model` crate: its
//! backend is a two-member workspace and the model lives inside `reqforge-server` beside axum and
//! tokio, so a path dependency would drag a web server in to read a file. Extracting that crate is
//! a phase-2 decision this spike should inform rather than block. The format read here was taken
//! from a real artifact in ReqForge's own tree, not from its documentation.
//!
//! Implements: REQ009 (a second adapter behind the source-agnostic seam)

use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use walkdir::WalkDir;

use crate::source::{Annotation, Classification, Item, RequirementsSource};

/// The file marking a directory as a ReqForge collection. Its presence is what tells `adopt` the
/// subject stores requirements this way rather than as Doorstop documents.
pub const COLLECTION_FILE: &str = ".collection.json";

/// The frontmatter fields this adapter reads. ReqForge's own `Artifact` struct carries far more
/// (shape, links, review log, blob and URL variants); an [`Item`] has four fields, and a spike that
/// deserialized the rest would be claiming to understand a model it has not yet brought in.
#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct Frontmatter {
    title: String,
    /// ReqForge's own triage prior. [`Item::verification_hint`] was written for this field by name.
    #[serde(default)]
    expects_code_trace: Option<bool>,
    /// `false` retires an artifact without deleting it; absent means active.
    #[serde(default)]
    active: Option<bool>,
}

/// Split a ReqForge artifact into its JSON frontmatter and Markdown body.
///
/// The convention is a line of exactly `---`, JSON, a closing `---` line, then the body — any
/// valid JSON also being valid YAML flow style, so ordinary Markdown renderers show it as
/// frontmatter. Strict on purpose, exactly as ReqForge's own parser is: a file that does not open
/// with the delimiter is a diagnostic, not something to guess at.
fn split_frontmatter(text: &str) -> Option<(&str, &str)> {
    let rest = text.strip_prefix("---\n")?;
    let end = rest.find("\n---\n")?;
    Some((&rest[..end], &rest[end + 5..]))
}

/// Every collection directory under `root` — a directory holding a [`COLLECTION_FILE`].
///
/// Walks by [`crate::subject_tree`]'s rules like every other traversal of a subject, so a
/// requirement store cannot be found somewhere the rest of provreq refuses to look.
pub fn discover(root: &Path) -> Vec<PathBuf> {
    let mut dirs: Vec<PathBuf> = WalkDir::new(root)
        .follow_links(false)
        .into_iter()
        .filter_entry(|e| {
            !(e.file_type().is_dir() && crate::subject_tree::is_pruned_dir(e.path(), e.depth()))
        })
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_dir() && e.path().join(COLLECTION_FILE).is_file())
        .map(|e| e.path().to_path_buf())
        .collect();
    dirs.sort();
    dirs
}

/// Reads requirement prose from a subject's ReqForge artifacts.
pub struct ReqforgeSource {
    root: PathBuf,
}

impl ReqforgeSource {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }
}

impl RequirementsSource for ReqforgeSource {
    fn items(&self) -> Result<Vec<Item>> {
        let mut items = Vec::new();
        for dir in discover(&self.root) {
            let entries =
                std::fs::read_dir(&dir).with_context(|| format!("reading {}", dir.display()))?;
            for entry in entries {
                let path = entry?.path();
                if path.extension().is_none_or(|x| x != "md") {
                    continue;
                }
                let raw = std::fs::read_to_string(&path)
                    .with_context(|| format!("reading {}", path.display()))?;
                let (json, body) = split_frontmatter(&raw)
                    .with_context(|| format!("{} has no JSON frontmatter", path.display()))?;
                let meta: Frontmatter = serde_json::from_str(json)
                    .with_context(|| format!("parsing frontmatter of {}", path.display()))?;

                // An inactive artifact is retired, not deleted. Reporting it as a live requirement
                // would resurrect it as something the operator is asked to formalize.
                if meta.active == Some(false) {
                    continue;
                }

                // The filename stem is the identity ReqForge itself uses (`LoadedArtifact::name`),
                // and is what its links point at through their hint. `legacy.doorstopUid` records
                // where an imported item came from and is deliberately not read as the id: once
                // imported, provreq does not care where a requirement came from.
                let id = path
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .with_context(|| format!("unreadable artifact name {}", path.display()))?
                    .to_string();
                let text = body.trim().to_string();
                items.push(Item {
                    // `modifiedAt` is ReqForge's native token, and is *not* used: it moves when
                    // metadata alone changes, and stands still if prose is edited without it. The
                    // drift baseline has to be the prose, so this hashes the prose (R-src-3), the
                    // same choice the Doorstop adapter makes for the same reason.
                    revision: crate::source::content_hash(&text),
                    id,
                    text,
                    title: Some(meta.title),
                    // Advisory seed only (R-src-5), and only an *explicit* artifact-level `true`
                    // seeds anything. ReqForge defines this flag (`TRACE-codeCoverageExpectation`)
                    // as whether an artifact is expected to have implementation and verification
                    // references in code, defaulting to `true` at the collection level with a
                    // per-artifact override — so absent means "inherit", not "unknown", and an
                    // inherited default is a choice nobody made. Seeding every imported requirement
                    // as formalizable on the strength of it would bias triage while carrying no
                    // operator intent.
                    //
                    // An explicit `false` is real information — ReqForge's generalisation of
                    // doorstop's non-functional-requirement exemption — and it still seeds nothing,
                    // because it rules `FormalizableNow` *out* without choosing between the
                    // remaining two: "responds within 200ms" is falsifiable-only, "shall be
                    // maintainable" stays prose. [`Classification`] cannot say "not that one", so
                    // any pick here would be a coin flip wearing a hint's clothes. Widening the
                    // type to carry it is filed for phase 2/4 (#297).
                    verification_hint: match meta.expects_code_trace {
                        Some(true) => Some(Classification::FormalizableNow),
                        _ => None,
                    },
                });
            }
        }
        items.sort_by(|a, b| a.id.cmp(&b.id));
        Ok(items)
    }

    /// Not implemented by the spike, and loudly so.
    ///
    /// Writing provenance back needs a decision this phase has not earned: ReqForge's review log is
    /// where an admission belongs (that is most of why its model is worth having), and stamping a
    /// `provreq:` key beside the schema's own fields would be inventing a convention phase 4 is
    /// meant to settle. A spike that guessed here would leave a wrong shape on disk for later
    /// phases to migrate, so it refuses instead — see #296.
    fn annotate(&self, _id: &str, _annotation: &Annotation) -> Result<()> {
        bail!(
            "provreq cannot yet write a formalization back to a ReqForge artifact — the review log \
             is where an admission belongs, and that convergence is phase 4 of the absorb (#296). \
             Formalize against a Doorstop source until then"
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The committed fixture, copied verbatim from a real artifact in ReqForge's own tree.
    fn fixture() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/reqforge-subject")
    }

    // Verifies: REQ009 / #296 — a ReqForge artifact reaches the seam as an ordinary `Item`. The
    // fixture is the real on-disk shape, JSON frontmatter and all, so this fails if that format is
    // read from documentation rather than from what ReqForge actually writes.
    #[test]
    fn reads_a_reqforge_artifact_as_a_source_item() {
        let items = ReqforgeSource::new(fixture()).items().unwrap();
        assert_eq!(items.len(), 1, "one artifact in the fixture, got {items:?}");
        let item = &items[0];
        assert_eq!(item.id, "REQ-queueIsDrained");
        assert_eq!(
            item.text,
            "Every message accepted onto the queue is eventually removed from it."
        );
        assert_eq!(
            item.title.as_deref(),
            Some("A queued message is eventually drained")
        );
        assert_eq!(
            item.verification_hint,
            Some(Classification::FormalizableNow),
            "`expectsCodeTrace: true` is the prior Item::verification_hint was written for"
        );
        assert!(!item.revision.is_empty());
    }

    // Verifies: REQ009 / #296 — the revision tracks the prose, not the metadata timestamp. A
    // baseline that moved when only `modifiedAt` moved would report drift that did not happen, and
    // one that stood still through a prose edit would miss the drift that did.
    #[test]
    fn the_revision_follows_the_prose_and_not_the_timestamp() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path().join("artifacts/req");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::copy(
            fixture().join("artifacts/req/.collection.json"),
            dir.join(COLLECTION_FILE),
        )
        .unwrap();
        let src =
            std::fs::read_to_string(fixture().join("artifacts/req/REQ-queueIsDrained.md")).unwrap();

        std::fs::write(dir.join("REQ-queueIsDrained.md"), &src).unwrap();
        let before = ReqforgeSource::new(tmp.path()).items().unwrap()[0]
            .revision
            .clone();

        // Metadata moves, prose does not.
        let touched = src.replace("2026-05-05T20:31:36", "2027-01-01T00:00:00");
        assert_ne!(touched, src, "the timestamp must actually have changed");
        std::fs::write(dir.join("REQ-queueIsDrained.md"), &touched).unwrap();
        assert_eq!(
            ReqforgeSource::new(tmp.path()).items().unwrap()[0].revision,
            before,
            "a metadata-only edit is not requirement drift"
        );

        // Prose moves.
        let edited = src.replace("eventually removed", "immediately removed");
        assert_ne!(edited, src, "the prose must actually have changed");
        std::fs::write(dir.join("REQ-queueIsDrained.md"), &edited).unwrap();
        assert_ne!(
            ReqforgeSource::new(tmp.path()).items().unwrap()[0].revision,
            before,
            "edited prose is drift, and the baseline must say so"
        );
    }

    // Verifies: REQ009 / #296 — a retired artifact is not offered as a live requirement.
    #[test]
    fn an_inactive_artifact_is_not_a_requirement() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path().join("artifacts/req");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::copy(
            fixture().join("artifacts/req/.collection.json"),
            dir.join(COLLECTION_FILE),
        )
        .unwrap();
        let retired =
            std::fs::read_to_string(fixture().join("artifacts/req/REQ-queueIsDrained.md"))
                .unwrap()
                .replace("\"active\": true", "\"active\": false");
        std::fs::write(dir.join("REQ-queueIsDrained.md"), retired).unwrap();

        assert!(ReqforgeSource::new(tmp.path()).items().unwrap().is_empty());
    }

    // Verifies: REQ009 / #296 — writing provenance back refuses in a way the operator can act on,
    // rather than inventing a convention phase 4 is meant to settle.
    #[test]
    fn writing_back_refuses_with_a_reason() {
        let annotation = Annotation {
            status: "admitted-but-ungrounded".into(),
            prl: "requirement r { }".into(),
            review: "mandatory".into(),
            reviewer: "someone".into(),
            reviewed_at_unix: 0,
            source_revision: "abc".into(),
        };
        let err = ReqforgeSource::new(fixture())
            .annotate("REQ-queueIsDrained", &annotation)
            .unwrap_err()
            .to_string();
        assert!(err.contains("phase 4"), "must say why, got: {err}");
    }
}
