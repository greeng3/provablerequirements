//! Step 2 triage machinery: advisory, freely-re-triageable classification state
//! plus the `Classifier` seam. Triage routes formalization work; it never fakes a
//! proof, so it is ungated companion state the operator confirms/overrides
//! (R-triage-1). The LLM bulk pre-sort is a deferred adapter; the honest floor
//! here seeds every item as prose (R-triage-2).
//!
//! Implements: REQ010 (persist advisory triage state, human-overridable)

use crate::source::{Classification, Item};
use anyhow::{Context, Result};
use std::collections::BTreeMap;
use std::path::Path;

/// Mutable companion state file, written at the companion root (A6 write-freely
/// channel, keyed by source id).
pub const TRIAGE_FILE: &str = "triage.yml";

/// One item's triage record.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct TriageEntry {
    pub classification: Classification,
    /// Source revision this classification was made against (R-src-3); lets a
    /// later slice flag drift. Advisory only — re-triage is always allowed.
    pub revision: String,
}

/// Persisted triage state, keyed by source id.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct TriageState {
    pub schema: u32,
    pub items: BTreeMap<String, TriageEntry>,
}

impl TriageState {
    pub fn new() -> Self {
        Self {
            schema: 1,
            items: BTreeMap::new(),
        }
    }
}

impl Default for TriageState {
    fn default() -> Self {
        Self::new()
    }
}

/// Load triage state from a companion root, or an empty state if none is written
/// yet.
pub fn load(companion_root: &Path) -> Result<TriageState> {
    let path = companion_root.join(TRIAGE_FILE);
    if !path.exists() {
        return Ok(TriageState::new());
    }
    let text =
        std::fs::read_to_string(&path).with_context(|| format!("reading {}", path.display()))?;
    serde_yaml::from_str(&text).with_context(|| format!("parsing {}", path.display()))
}

/// Write triage state to the companion root.
pub fn save(companion_root: &Path, state: &TriageState) -> Result<()> {
    let path = companion_root.join(TRIAGE_FILE);
    let yaml = serde_yaml::to_string(state).context("serializing triage state")?;
    std::fs::write(&path, yaml).with_context(|| format!("writing {}", path.display()))
}

/// Bulk pre-sorts a backlog into advisory buckets (R-triage-1). Returns exactly
/// one bucket per input item, in order. Fallible (an LLM classifier does I/O) and
/// async; dispatched generically so no trait objects are needed. Every output is
/// a seed the operator still confirms.
pub trait Classifier {
    fn classify(
        &self,
        items: &[Item],
    ) -> impl std::future::Future<Output = Result<Vec<Classification>>> + Send;
}

/// Adapter #0: the honest floor. Seeds each item from its source verification
/// hint when present (R-triage-2), else `stays-prose` — never over-claiming
/// formalizability. The operator promotes items with `set`, or an LLM classifier
/// ([`crate::llm::LlmClassifier`]) pre-sorts them.
pub struct ProseFloorClassifier;

impl Classifier for ProseFloorClassifier {
    async fn classify(&self, items: &[Item]) -> Result<Vec<Classification>> {
        Ok(items
            .iter()
            .map(|i| i.verification_hint.unwrap_or(Classification::StaysProse))
            .collect())
    }
}

/// Seed triage for items that have no entry yet, leaving existing entries
/// untouched (re-triage is an explicit `set`, never a silent overwrite). Only the
/// pending items are sent to the classifier. Returns a new state.
pub async fn seed<C: Classifier>(
    state: &TriageState,
    items: &[Item],
    classifier: &C,
) -> Result<TriageState> {
    let pending: Vec<Item> = items
        .iter()
        .filter(|i| !state.items.contains_key(&i.id))
        .cloned()
        .collect();
    if pending.is_empty() {
        return Ok(state.clone());
    }
    let buckets = classifier.classify(&pending).await?;
    let mut next = state.items.clone();
    for (item, classification) in pending.iter().zip(buckets) {
        next.insert(
            item.id.clone(),
            TriageEntry {
                classification,
                revision: item.revision.clone(),
            },
        );
    }
    Ok(TriageState {
        schema: state.schema,
        items: next,
    })
}

/// Set (or override) one item's classification against its current revision
/// (R-triage-1 confirm/override). Returns a new state.
pub fn set(state: &TriageState, item: &Item, classification: Classification) -> TriageState {
    let mut next = state.items.clone();
    next.insert(
        item.id.clone(),
        TriageEntry {
            classification,
            revision: item.revision.clone(),
        },
    );
    TriageState {
        schema: state.schema,
        items: next,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(id: &str, hint: Option<Classification>) -> Item {
        Item {
            id: id.into(),
            text: format!("prose for {id}"),
            revision: format!("rev-{id}"),
            title: None,
            verification_hint: hint,
        }
    }

    // Verifies: REQ010 — the prose floor never over-claims, but honors a hint.
    #[tokio::test]
    async fn prose_floor_defaults_to_prose_and_honors_hint() {
        let items = [
            item("A", None),
            item("B", Some(Classification::FormalizableNow)),
        ];
        let buckets = ProseFloorClassifier.classify(&items).await.unwrap();
        assert_eq!(
            buckets,
            vec![Classification::StaysProse, Classification::FormalizableNow]
        );
    }

    // Verifies: REQ010 — seeding fills only unclassified items; set overrides.
    #[tokio::test]
    async fn seed_is_additive_and_set_overrides() {
        let items = [item("A", None), item("B", None)];
        let seeded = seed(&TriageState::new(), &items, &ProseFloorClassifier)
            .await
            .unwrap();
        assert_eq!(seeded.items.len(), 2);

        // Operator promotes A.
        let promoted = set(&seeded, &items[0], Classification::FormalizableNow);
        assert_eq!(
            promoted.items["A"].classification,
            Classification::FormalizableNow
        );

        // Re-seeding does NOT clobber the operator's override.
        let reseeded = seed(&promoted, &items, &ProseFloorClassifier)
            .await
            .unwrap();
        assert_eq!(
            reseeded.items["A"].classification,
            Classification::FormalizableNow
        );
    }

    // Verifies: REQ010 — triage state round-trips through the companion file.
    #[tokio::test]
    async fn state_persists_and_reloads() {
        let tmp = tempfile::tempdir().unwrap();
        let items = [item("A", Some(Classification::FalsifiableOnly))];
        let state = seed(&TriageState::new(), &items, &ProseFloorClassifier)
            .await
            .unwrap();
        save(tmp.path(), &state).unwrap();

        let loaded = load(tmp.path()).unwrap();
        assert_eq!(loaded, state);
        assert_eq!(
            loaded.items["A"].classification,
            Classification::FalsifiableOnly
        );
    }

    #[test]
    fn load_absent_state_is_empty() {
        let tmp = tempfile::tempdir().unwrap();
        assert!(load(tmp.path()).unwrap().items.is_empty());
    }
}
