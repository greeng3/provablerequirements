//! Step 3 draft lifecycle: a formalization-in-progress persists as a resumable
//! **draft** — a third artifact category beside A3's committed source-of-truth and
//! regenerated-derived, because it holds human keystrokes that are neither
//! admitted nor regenerable (R-draft-1). Resuming a draft re-checks the source
//! revision token so an item that moved underneath it is flagged **stale** before
//! work continues (R-draft-2).
//!
//! No LLM forward-translate, mechanical gate, or read-back yet — those are later
//! Step 3 slices. The candidate PRL is hand-authored for now; the D11 translate
//! slice will fill it automatically.
//!
//! Implements: REQ013 (persist resumable draft state), REQ014 (resume drift-check)

use crate::source::Item;
use anyhow::{Context, Result};
use std::collections::BTreeMap;
use std::path::Path;

/// Mutable companion state file at the companion root (A6 write-freely channel,
/// keyed by source id) — the draft peer of `triage.yml`.
pub const DRAFT_FILE: &str = "drafts.yml";

/// The mechanical-gate outcome recorded on a draft (R-draft-1). Rendered to strings
/// because a draft is a snapshot for the human, not something re-processed — the
/// structured [`crate::prl::GateError`]/`GateWarning` don't need to round-trip YAML.
#[derive(Debug, Clone, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum GateStatus {
    /// A candidate exists but the gate has not been run over it.
    #[default]
    Ungated,
    /// The candidate cleared the gate; `warnings` are vacuity/triviality flags for
    /// the human (empty = clean).
    Passed {
        #[serde(default)]
        warnings: Vec<String>,
    },
    /// The gate rejected the candidate; `errors` are the rendered reasons.
    Failed { errors: Vec<String> },
}

/// The D12 risk tier of a human confirmation. Vacuity-flagged (and later
/// grounding-heavy / high-stakes) candidates are `Mandatory`; a clean candidate is
/// `Optional`. Recorded so "review not required" is never confused with "reviewed".
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReviewTier {
    Mandatory,
    Optional,
}

impl ReviewTier {
    pub fn as_str(&self) -> &'static str {
        match self {
            ReviewTier::Mandatory => "mandatory",
            ReviewTier::Optional => "optional",
        }
    }
}

/// The formalization-admission state of a draft (D12). `Pending` is the in-progress
/// draft; `Admitted` is the `admitted-but-ungrounded` lifecycle state — formalization
/// is done, only the grounding anchor is missing — with its review provenance.
#[derive(Debug, Clone, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum Admission {
    #[default]
    Pending,
    Admitted {
        review: ReviewTier,
        by: String,
        /// Wall-clock admission time as Unix seconds (the caller supplies the clock,
        /// keeping this module pure and testable).
        at_unix: i64,
    },
}

/// One item's in-progress formalization draft.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Draft {
    /// The source revision this draft was last touched against (R-src-3).
    /// Staleness (R-draft-2) is `revision != item.revision`.
    pub revision: String,
    /// The candidate PRL — hand-authored (`--set`) or LLM-proposed (`--translate`);
    /// `None` until one is written.
    #[serde(default)]
    pub candidate: Option<String>,
    /// The last mechanical-gate outcome for `candidate` (R-draft-1). Defaults to
    /// `Ungated` so drafts written before this field existed load cleanly.
    #[serde(default)]
    pub gate: GateStatus,
    /// Whether the operator has admitted this formalization (D12). Defaults to
    /// `Pending` so drafts written before this field existed load cleanly.
    #[serde(default)]
    pub admission: Admission,
}

impl Draft {
    /// Whether this draft has been admitted (formalization confirmed by a human).
    pub fn is_admitted(&self) -> bool {
        matches!(self.admission, Admission::Admitted { .. })
    }
}

/// Persisted draft state, keyed by source id.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct DraftState {
    pub schema: u32,
    pub drafts: BTreeMap<String, Draft>,
}

impl DraftState {
    pub fn new() -> Self {
        Self {
            schema: 1,
            drafts: BTreeMap::new(),
        }
    }
}

impl Default for DraftState {
    fn default() -> Self {
        Self::new()
    }
}

/// Load draft state from a companion root, or an empty state if none is written
/// yet.
pub fn load(companion_root: &Path) -> Result<DraftState> {
    let path = companion_root.join(DRAFT_FILE);
    if !path.exists() {
        return Ok(DraftState::new());
    }
    let text =
        std::fs::read_to_string(&path).with_context(|| format!("reading {}", path.display()))?;
    serde_yaml::from_str(&text).with_context(|| format!("parsing {}", path.display()))
}

/// Write draft state to the companion root.
pub fn save(companion_root: &Path, state: &DraftState) -> Result<()> {
    let path = companion_root.join(DRAFT_FILE);
    let yaml = serde_yaml::to_string(state).context("serializing draft state")?;
    std::fs::write(&path, yaml).with_context(|| format!("writing {}", path.display()))
}

/// Open a draft for an item, snapshotting its current revision. **Additive**: an
/// existing draft is returned untouched, so an in-progress candidate and its drift
/// baseline are never silently reset — resume must see the real staleness
/// (R-draft-2). Returns a new state.
pub fn open(state: &DraftState, item: &Item) -> DraftState {
    if state.drafts.contains_key(&item.id) {
        return state.clone();
    }
    let mut drafts = state.drafts.clone();
    drafts.insert(
        item.id.clone(),
        Draft {
            revision: item.revision.clone(),
            candidate: None,
            gate: GateStatus::Ungated,
            admission: Admission::Pending,
        },
    );
    DraftState {
        schema: state.schema,
        drafts,
    }
}

/// Write a candidate PRL with its gate outcome and re-baseline the draft against the
/// item's current revision — writing the candidate is confirming it against the current
/// source, clearing any prior staleness (R-draft-2). A new candidate resets admission
/// to `Pending`: changing the formal claim invalidates any prior human confirmation.
/// Returns a new state.
pub fn set_candidate(
    state: &DraftState,
    item: &Item,
    candidate: impl Into<String>,
    gate: GateStatus,
) -> DraftState {
    let mut drafts = state.drafts.clone();
    drafts.insert(
        item.id.clone(),
        Draft {
            revision: item.revision.clone(),
            candidate: Some(candidate.into()),
            gate,
            admission: Admission::Pending,
        },
    );
    DraftState {
        schema: state.schema,
        drafts,
    }
}

/// Update only the recorded gate outcome for an existing draft, leaving its candidate
/// and revision baseline untouched (a re-check is not an edit). No-op if the draft is
/// absent. Returns a new state.
pub fn set_gate(state: &DraftState, id: &str, gate: GateStatus) -> DraftState {
    let mut drafts = state.drafts.clone();
    if let Some(existing) = drafts.get(id) {
        drafts.insert(
            id.to_string(),
            Draft {
                gate,
                ..existing.clone()
            },
        );
    }
    DraftState {
        schema: state.schema,
        drafts,
    }
}

/// Admit an existing draft's formalization (D12), recording the review tier and
/// provenance. Leaves the candidate, gate outcome, and revision baseline intact —
/// admission is a confirmation, not an edit. No-op if the draft is absent. The caller
/// supplies `at_unix` so this stays a pure function. Returns a new state.
pub fn admit(
    state: &DraftState,
    id: &str,
    review: ReviewTier,
    by: impl Into<String>,
    at_unix: i64,
) -> DraftState {
    let mut drafts = state.drafts.clone();
    if let Some(existing) = drafts.get(id) {
        drafts.insert(
            id.to_string(),
            Draft {
                admission: Admission::Admitted {
                    review,
                    by: by.into(),
                    at_unix,
                },
                ..existing.clone()
            },
        );
    }
    DraftState {
        schema: state.schema,
        drafts,
    }
}

/// Discard a draft, if one exists. Returns a new state.
pub fn discard(state: &DraftState, id: &str) -> DraftState {
    let mut drafts = state.drafts.clone();
    drafts.remove(id);
    DraftState {
        schema: state.schema,
        drafts,
    }
}

/// Whether the source item has moved since the draft was last touched (R-draft-2).
/// A stale draft needs human re-confirmation before formalization continues; the
/// engine never runs off a draft written against a since-changed requirement.
pub fn is_stale(draft: &Draft, item: &Item) -> bool {
    draft.revision != item.revision
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(id: &str, revision: &str) -> Item {
        Item {
            id: id.into(),
            text: format!("prose for {id}"),
            revision: revision.into(),
            title: None,
            verification_hint: None,
        }
    }

    // Verifies: REQ013 — opening is additive; it never clobbers an in-progress
    // candidate or resets the drift baseline.
    #[test]
    fn open_is_additive_and_preserves_candidate() {
        let it = item("REQ001", "rev-1");
        let opened = open(&DraftState::new(), &it);
        assert_eq!(opened.drafts["REQ001"].revision, "rev-1");
        assert_eq!(opened.drafts["REQ001"].candidate, None);

        let edited = set_candidate(&opened, &it, "requirement foo { }", GateStatus::Ungated);
        // Re-opening the same id leaves the operator's work untouched.
        let reopened = open(&edited, &it);
        assert_eq!(
            reopened.drafts["REQ001"].candidate.as_deref(),
            Some("requirement foo { }")
        );
    }

    // Verifies: REQ014 — a draft is stale exactly when the source revision has
    // moved since the draft was last touched, and editing re-baselines it.
    #[test]
    fn stale_when_source_revision_moves() {
        let v1 = item("REQ001", "rev-1");
        let draft_state = set_candidate(
            &DraftState::new(),
            &v1,
            "requirement foo { }",
            GateStatus::Ungated,
        );
        let draft = &draft_state.drafts["REQ001"];

        // Same revision → fresh.
        assert!(!is_stale(draft, &v1));

        // Source item moved under the draft → stale.
        let v2 = item("REQ001", "rev-2");
        assert!(is_stale(draft, &v2));

        // Editing against the new revision re-baselines it back to fresh.
        let rebaselined = set_candidate(
            &draft_state,
            &v2,
            "requirement foo { edited }",
            GateStatus::Ungated,
        );
        assert!(!is_stale(&rebaselined.drafts["REQ001"], &v2));
    }

    // Verifies: REQ013/REQ017 — draft state (including the gate outcome) round-trips
    // through the companion file, and discard removes it.
    #[test]
    fn state_persists_reloads_and_discards() {
        let tmp = tempfile::tempdir().unwrap();
        let it = item("REQ001", "rev-1");
        let state = set_candidate(
            &DraftState::new(),
            &it,
            "requirement foo { }",
            GateStatus::Passed {
                warnings: vec!["line 1: something suspicious".into()],
            },
        );
        save(tmp.path(), &state).unwrap();

        let loaded = load(tmp.path()).unwrap();
        assert_eq!(loaded, state);
        assert_eq!(
            loaded.drafts["REQ001"].candidate.as_deref(),
            Some("requirement foo { }")
        );
        assert!(matches!(
            loaded.drafts["REQ001"].gate,
            GateStatus::Passed { .. }
        ));

        let after = discard(&loaded, "REQ001");
        assert!(after.drafts.is_empty());
    }

    // Verifies: REQ017 — a re-check updates only the gate outcome, leaving the
    // candidate and revision baseline intact.
    #[test]
    fn set_gate_updates_outcome_only() {
        let it = item("REQ001", "rev-1");
        let state = set_candidate(
            &DraftState::new(),
            &it,
            "requirement foo { }",
            GateStatus::Ungated,
        );
        let rechecked = set_gate(
            &state,
            "REQ001",
            GateStatus::Failed {
                errors: vec!["line 2: boom".into()],
            },
        );
        let d = &rechecked.drafts["REQ001"];
        assert_eq!(d.candidate.as_deref(), Some("requirement foo { }"));
        assert_eq!(d.revision, "rev-1");
        assert!(matches!(d.gate, GateStatus::Failed { .. }));
    }

    #[test]
    fn set_gate_is_a_noop_for_absent_draft() {
        let state = DraftState::new();
        let after = set_gate(&state, "REQ404", GateStatus::Ungated);
        assert!(after.drafts.is_empty());
    }

    #[test]
    fn load_absent_state_is_empty() {
        let tmp = tempfile::tempdir().unwrap();
        assert!(load(tmp.path()).unwrap().drafts.is_empty());
    }

    // Verifies: REQ019 — admitting records the review tier and provenance while
    // leaving the candidate and gate intact.
    #[test]
    fn admit_records_review_and_provenance() {
        let it = item("REQ001", "rev-1");
        let state = set_candidate(
            &DraftState::new(),
            &it,
            "requirement foo { }",
            GateStatus::Ungated,
        );
        assert!(!state.drafts["REQ001"].is_admitted());

        let admitted = admit(&state, "REQ001", ReviewTier::Mandatory, "gg", 1_700_000_000);
        let d = &admitted.drafts["REQ001"];
        assert!(d.is_admitted());
        assert_eq!(d.candidate.as_deref(), Some("requirement foo { }"));
        assert!(matches!(
            &d.admission,
            Admission::Admitted { review: ReviewTier::Mandatory, by, at_unix }
                if by == "gg" && *at_unix == 1_700_000_000
        ));
    }

    // Verifies: REQ019 — editing the candidate after admission resets it to Pending;
    // a changed formal claim is no longer the confirmed one.
    #[test]
    fn editing_candidate_revokes_admission() {
        let it = item("REQ001", "rev-1");
        let state = set_candidate(
            &DraftState::new(),
            &it,
            "requirement foo { }",
            GateStatus::Ungated,
        );
        let admitted = admit(&state, "REQ001", ReviewTier::Optional, "gg", 1);
        assert!(admitted.drafts["REQ001"].is_admitted());

        let edited = set_candidate(&admitted, &it, "requirement bar { }", GateStatus::Ungated);
        assert!(!edited.drafts["REQ001"].is_admitted());
    }

    #[test]
    fn admit_is_a_noop_for_absent_draft() {
        let after = admit(&DraftState::new(), "REQ404", ReviewTier::Optional, "gg", 1);
        assert!(after.drafts.is_empty());
    }
}
