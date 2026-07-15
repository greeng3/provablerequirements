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

/// One item's in-progress formalization draft.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Draft {
    /// The source revision this draft was last touched against (R-src-3).
    /// Staleness (R-draft-2) is `revision != item.revision`.
    pub revision: String,
    /// The operator's hand-authored candidate PRL; `None` until they write one.
    /// A later slice fills this from the D11 forward-translate.
    #[serde(default)]
    pub candidate: Option<String>,
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
        },
    );
    DraftState {
        schema: state.schema,
        drafts,
    }
}

/// Write the operator's candidate PRL and re-baseline the draft against the item's
/// current revision — editing the candidate is confirming it against the current
/// source, clearing any prior staleness (R-draft-2). Returns a new state.
pub fn set_candidate(state: &DraftState, item: &Item, candidate: impl Into<String>) -> DraftState {
    let mut drafts = state.drafts.clone();
    drafts.insert(
        item.id.clone(),
        Draft {
            revision: item.revision.clone(),
            candidate: Some(candidate.into()),
        },
    );
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

        let edited = set_candidate(&opened, &it, "requirement foo { }");
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
        let draft_state = set_candidate(&DraftState::new(), &v1, "requirement foo { }");
        let draft = &draft_state.drafts["REQ001"];

        // Same revision → fresh.
        assert!(!is_stale(draft, &v1));

        // Source item moved under the draft → stale.
        let v2 = item("REQ001", "rev-2");
        assert!(is_stale(draft, &v2));

        // Editing against the new revision re-baselines it back to fresh.
        let rebaselined = set_candidate(&draft_state, &v2, "requirement foo { edited }");
        assert!(!is_stale(&rebaselined.drafts["REQ001"], &v2));
    }

    // Verifies: REQ013 — draft state round-trips through the companion file, and
    // discard removes it.
    #[test]
    fn state_persists_reloads_and_discards() {
        let tmp = tempfile::tempdir().unwrap();
        let it = item("REQ001", "rev-1");
        let state = set_candidate(&DraftState::new(), &it, "requirement foo { }");
        save(tmp.path(), &state).unwrap();

        let loaded = load(tmp.path()).unwrap();
        assert_eq!(loaded, state);
        assert_eq!(
            loaded.drafts["REQ001"].candidate.as_deref(),
            Some("requirement foo { }")
        );

        let after = discard(&loaded, "REQ001");
        assert!(after.drafts.is_empty());
    }

    #[test]
    fn load_absent_state_is_empty() {
        let tmp = tempfile::tempdir().unwrap();
        assert!(load(tmp.path()).unwrap().drafts.is_empty());
    }
}
