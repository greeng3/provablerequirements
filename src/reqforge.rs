//! The ReqForge [`RequirementsSource`] adapter (adapter #2) — began as the spike for #296.
//!
//! [`crate::source`] drew this seam single-implementation and said why: "the reqforge adapter is a
//! real, not-speculative second consumer that lands when its format stabilises." This is that
//! adapter. It began as the phase-1 spike whose job was to **falsify** the claim the whole plan
//! rests on — that provreq's verification does not know what the requirement model is — by sourcing
//! a requirement from ReqForge's on-disk shape and seeing what that dragged in. The thesis held:
//! reading a requirement moved no engine adapter, refusal classification, mirror channel, or verdict
//! model.
//!
//! It now reads and writes through the extracted `reqforge-model` crate (#305): `items()` loads with
//! ReqForge's own loader, and `annotate()` appends provenance to an artifact's review log with its
//! own writer (#313). The spike's hand-rolled frontmatter reader is gone.
//!
//! Implements: REQ009 (a second adapter behind the source-agnostic seam),
//! REQ020 (the formalization back-write, in this source's native review-log form)

use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use walkdir::WalkDir;

use crate::source::{Annotation, Classification, Item, RequirementsSource};

/// The file marking a directory as a ReqForge collection. Its presence is what tells `adopt` the
/// subject stores requirements this way rather than as Doorstop documents.
pub const COLLECTION_FILE: &str = ".collection.json";

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
                // The shared rule first, because the extension test cannot stand in for it: an
                // AppleDouble sidecar carries the extension of the file it shadows, so
                // `._REQ001.md` passes as Markdown and parses perfectly. Reading one produced a
                // second requirement with its own id, draft, and verdict — a duplicate of one the
                // author wrote once (#307). This walk uses `read_dir` rather than `walkdir`, so it
                // has to ask; the other adapters get the same answer via `filter_entry`.
                if crate::subject_tree::is_pruned_file(&path)
                    || path.extension().is_none_or(|x| x != "md")
                {
                    continue;
                }
                // Read through ReqForge's own loader (#305). The spike hand-rolled a frontmatter
                // splitter and a cut-down struct because there was nothing to call; both were
                // guesses at a format we can now read with the code that writes it — including the
                // schema migration a file older than the current `schemaVersion` needs.
                let loaded = reqforge_model::load::artifact::load_content_artifact(&path)
                    .with_context(|| format!("loading {}", path.display()))?;
                let meta = loaded.metadata;

                // An inactive artifact is retired, not deleted. Reporting it as a live requirement
                // would resurrect it as something the operator is asked to formalize.
                if meta.active == Some(false) {
                    continue;
                }

                // The filename stem is the identity ReqForge itself uses (`LoadedArtifact::name`),
                // and is what its links point at through their hint. `legacy.doorstopUid` records
                // where an imported item came from and is deliberately not read as the id: once
                // imported, provreq does not care where a requirement came from.
                let id = loaded.name;
                // `body` is `None` for the blob and URL shapes, which carry no prose to formalize.
                let text = loaded.body.unwrap_or_default().trim().to_string();
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

    /// Stamp the annotation onto the artifact as a review-log entry (R-src-6, REQ020).
    ///
    /// ReqForge's review log is where an admission belongs — the phase-1 reason this refused until
    /// now (#296) — so the back-write appends there rather than inventing a foreign frontmatter key
    /// that phase 4 would have to migrate. The entry is tagged `provreq-formalized` (the schema
    /// keeps `outcome` an open string for exactly this), carries the reviewer and admission time,
    /// and holds the structured provenance under a single namespaced `provreq` key. The review log
    /// is an event stream, so a re-write appends a further entry rather than replacing — where the
    /// Doorstop adapter replaces its single `provreq:` block. Mutates the working tree; the operator
    /// commits it.
    fn annotate(&self, id: &str, annotation: &Annotation) -> Result<()> {
        let path = self.artifact_path(id)?;
        let loaded = reqforge_model::load::artifact::load_content_artifact(&path)
            .with_context(|| format!("loading {}", path.display()))?;
        let mut metadata = loaded.metadata;
        let body = loaded.body.unwrap_or_default();

        let timestamp = chrono::DateTime::from_timestamp(annotation.reviewed_at_unix, 0)
            .with_context(|| {
                format!(
                    "annotation timestamp {} is out of range",
                    annotation.reviewed_at_unix
                )
            })?;

        // One namespaced key, not the annotation's fields loose in overflow: it stays unambiguous
        // to a later migration which provenance came from provreq, and cannot collide with a
        // ReqForge outcome tag's own fields.
        let provreq = serde_json::json!({
            "status": annotation.status,
            "prl": annotation.prl,
            "review": annotation.review,
            "sourceRevision": annotation.source_revision,
        });
        let mut overflow = reqforge_model::schema::Overflow::new();
        overflow.insert("provreq".into(), provreq);

        metadata
            .review_log
            .push(reqforge_model::schema::ReviewLogEntry {
                timestamp,
                reviewer: annotation.reviewer.clone(),
                outcome: "provreq-formalized".into(),
                explanation: Some(format!(
                    "provreq admitted a formalization ({}) against source revision {}",
                    annotation.status, annotation.source_revision
                )),
                added_todos: Vec::new(),
                resolved_todos: Vec::new(),
                overflow,
            });
        metadata.modified_at = timestamp;

        let bytes = reqforge_model::write::render_artifact_file(&metadata, &body)
            .with_context(|| format!("rendering {}", path.display()))?;
        reqforge_model::write::atomic_write(&path, &bytes)
            .with_context(|| format!("writing {}", path.display()))
    }
}

impl ReqforgeSource {
    /// The on-disk path of the artifact whose id is `id`, or an error naming the id when no live
    /// artifact carries it. The filename stem is the identity (`items()` reads `LoadedArtifact::name`
    /// from it), so an artifact `id` lives at `{id}.md` in one of the subject's collections. Asks
    /// [`crate::subject_tree`] before trusting a match, the same rule `items()` applies, so a
    /// resource-file sidecar can never be the target of a write.
    fn artifact_path(&self, id: &str) -> Result<PathBuf> {
        for dir in discover(&self.root) {
            let candidate = dir.join(format!("{id}.md"));
            if candidate.is_file() && !crate::subject_tree::is_pruned_file(&candidate) {
                return Ok(candidate);
            }
        }
        bail!(
            "no ReqForge artifact with id {id} in {}",
            self.root.display()
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

    // Verifies: REQ060 / #307 — an operating system's resource file never becomes a requirement.
    // The sidecar holds a byte-for-byte copy of a real artifact on purpose: it parses perfectly, so
    // only the shared rule can keep it out. Before this it arrived as a second requirement with its
    // own id, draft, and verdict — a duplicate of one the author wrote once, sourced from a file
    // nobody wrote. That is the #294 failure, reintroduced by a walk that consulted `read_dir`
    // instead of `subject_tree`.
    #[test]
    fn a_mac_sidecar_never_becomes_a_requirement() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path().join("artifacts/req");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::copy(
            fixture().join("artifacts/req/.collection.json"),
            dir.join(COLLECTION_FILE),
        )
        .unwrap();
        let real =
            std::fs::read_to_string(fixture().join("artifacts/req/REQ-queueIsDrained.md")).unwrap();
        std::fs::write(dir.join("REQ-queueIsDrained.md"), &real).unwrap();
        std::fs::write(dir.join("._REQ-queueIsDrained.md"), &real).unwrap();
        std::fs::write(dir.join(".DS_Store"), &real).unwrap();

        let items = ReqforgeSource::new(tmp.path()).items().unwrap();
        let ids: Vec<&str> = items.iter().map(|i| i.id.as_str()).collect();
        assert_eq!(ids, ["REQ-queueIsDrained"], "only the authored artifact");
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

    fn sample_annotation() -> Annotation {
        Annotation {
            status: "admitted-but-ungrounded".into(),
            prl: "requirement r { }".into(),
            review: "mandatory".into(),
            reviewer: "someone".into(),
            reviewed_at_unix: 1_700_000_000,
            source_revision: "abc".into(),
        }
    }

    /// A collection dir holding the fixture artifact, under a temp root.
    fn subject_with_fixture_artifact() -> (tempfile::TempDir, PathBuf) {
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
        let artifact = dir.join("REQ-queueIsDrained.md");
        std::fs::write(&artifact, src).unwrap();
        let root = tmp.path().to_path_buf();
        (tmp, root)
    }

    // Verifies: REQ020 / #313 — the ReqForge adapter writes provenance back as a review-log
    // entry (its native form), not a foreign frontmatter key. The structured fields ride under a
    // namespaced `provreq` overflow key so they round-trip and are unambiguous to migrate.
    #[test]
    fn annotate_appends_a_provreq_review_log_entry() {
        let (_tmp, root) = subject_with_fixture_artifact();
        let ann = sample_annotation();
        ReqforgeSource::new(&root)
            .annotate("REQ-queueIsDrained", &ann)
            .unwrap();

        let loaded = reqforge_model::load::artifact::load_content_artifact(
            &root.join("artifacts/req/REQ-queueIsDrained.md"),
        )
        .unwrap();
        let entry = loaded
            .metadata
            .review_log
            .last()
            .expect("a review-log entry was appended");
        assert_eq!(entry.outcome, "provreq-formalized");
        assert_eq!(entry.reviewer, "someone");
        assert_eq!(entry.timestamp.timestamp(), 1_700_000_000);
        let provreq = entry.overflow.get("provreq").expect("namespaced block");
        assert_eq!(provreq["status"], "admitted-but-ungrounded");
        assert_eq!(provreq["prl"], "requirement r { }");
        assert_eq!(provreq["review"], "mandatory");
        assert_eq!(provreq["sourceRevision"], "abc");
    }

    // Verifies: REQ020 / #313 — the review log is an event stream, so re-writing appends rather
    // than replacing; the reader takes the latest by timestamp.
    #[test]
    fn annotate_appends_and_does_not_replace() {
        let (_tmp, root) = subject_with_fixture_artifact();
        let src = ReqforgeSource::new(&root);
        let before = reqforge_model::load::artifact::load_content_artifact(
            &root.join("artifacts/req/REQ-queueIsDrained.md"),
        )
        .unwrap()
        .metadata
        .review_log
        .len();

        src.annotate("REQ-queueIsDrained", &sample_annotation())
            .unwrap();
        src.annotate("REQ-queueIsDrained", &sample_annotation())
            .unwrap();

        let after = reqforge_model::load::artifact::load_content_artifact(
            &root.join("artifacts/req/REQ-queueIsDrained.md"),
        )
        .unwrap()
        .metadata
        .review_log
        .len();
        assert_eq!(after, before + 2, "each write-back appends one entry");
    }

    // Verifies: REQ020 / #313 — writing back to an id no source artifact carries is an error the
    // operator can act on, not a silent no-op.
    #[test]
    fn annotate_rejects_unknown_item() {
        let (_tmp, root) = subject_with_fixture_artifact();
        let err = ReqforgeSource::new(&root)
            .annotate("REQ-doesNotExist", &sample_annotation())
            .unwrap_err()
            .to_string();
        assert!(err.contains("REQ-doesNotExist"), "names the id, got: {err}");
    }
}
