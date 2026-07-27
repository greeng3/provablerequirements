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

/// What a triage run should do, decided **before** anything is announced (REQ053).
///
/// Seeding is additive, so an already-classified backlog leaves nothing to ask a model about.
/// Skipping the request is correct; announcing it anyway makes a no-op indistinguishable from a
/// completed run whose answer happened to match what was already recorded.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TriagePlan {
    /// Every item is already classified and the operator did not ask for a re-run.
    Nothing { already: usize },
    /// There is work: classify `count` items, starting from `base`. `base` is the current state
    /// for an ordinary run (additive, so existing buckets are kept) and empty for a re-classify
    /// (every item is put back in front of the classifier).
    Classify { base: TriageState, count: usize },
}

/// Decide a triage run's scope. Pure, so the caller can report the decision rather than narrate an
/// intention — the same shape as [`crate::adopt::plan`] and [`crate::provision::decide_install`].
pub fn plan(state: &TriageState, items: &[Item], reclassify: bool) -> TriagePlan {
    if reclassify {
        return TriagePlan::Classify {
            base: TriageState::new(),
            count: items.len(),
        };
    }
    let count = items
        .iter()
        .filter(|i| !state.items.contains_key(&i.id))
        .count();
    if count == 0 {
        TriagePlan::Nothing {
            already: items.len(),
        }
    } else {
        TriagePlan::Classify {
            base: state.clone(),
            count,
        }
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

    // Verifies: REQ053 — the scope of a run is decided before it is described. A fully-triaged
    // backlog is `Nothing`, so the caller can say "nothing to classify" instead of announcing a
    // model it is not about to ask. This is the defect: `triage` printed "Classifying backlog with
    // <model> via <url> …" and then classified nothing, output indistinguishable from a real run.
    #[tokio::test]
    async fn a_fully_triaged_backlog_plans_no_work() {
        let items = [item("A", None), item("B", None)];
        let seeded = seed(&TriageState::new(), &items, &ProseFloorClassifier)
            .await
            .unwrap();

        assert_eq!(
            plan(&seeded, &items, false),
            TriagePlan::Nothing { already: 2 }
        );

        // A partly-triaged backlog is real work, counted honestly: only the pending item.
        let partial = set(&TriageState::new(), &items[0], Classification::StaysProse);
        match plan(&partial, &items, false) {
            TriagePlan::Classify { count, base } => {
                assert_eq!(count, 1, "only the untriaged item is work");
                assert_eq!(base, partial, "an ordinary run keeps what is already there");
            }
            other => panic!("expected work, got {other:?}"),
        }
    }

    // Verifies: REQ053 — `--reclassify` is the deliberate way out of a fully-triaged state: every
    // item goes back in front of the classifier, from an empty base, so nothing is skipped as
    // "already done". Without it the prose floor is a one-way door.
    #[tokio::test]
    async fn reclassify_puts_every_item_back_in_front_of_the_classifier() {
        let items = [item("A", None), item("B", None)];
        let seeded = seed(&TriageState::new(), &items, &ProseFloorClassifier)
            .await
            .unwrap();

        match plan(&seeded, &items, true) {
            TriagePlan::Classify { count, base } => {
                assert_eq!(count, 2, "every item is re-classified");
                assert!(base.items.is_empty(), "nothing is treated as already done");
            }
            other => panic!("expected work, got {other:?}"),
        }
    }

    // Verifies: REQ052 — a failed classification is not a classification: the error propagates out
    // of seeding, so the caller never reaches the save and existing buckets survive untouched. A
    // run that could not ask the model must not be able to rewrite the operator's backlog.
    #[tokio::test]
    async fn a_failed_classifier_leaves_the_existing_state_untouched() {
        struct FailingClassifier;
        impl Classifier for FailingClassifier {
            async fn classify(&self, _items: &[Item]) -> Result<Vec<Classification>> {
                anyhow::bail!("the model returned no usable classification")
            }
        }

        let items = [item("A", None), item("B", None)];
        let before = set(
            &TriageState::new(),
            &items[0],
            Classification::FormalizableNow,
        );

        let err = seed(&before, &items, &FailingClassifier)
            .await
            .expect_err("a failed classification must not pass as a result");
        assert!(format!("{err:#}").contains("classif"), "{err:#}");

        // The operator's own decision is still there, and B was not invented.
        assert_eq!(
            before.items.get("A").map(|e| e.classification),
            Some(Classification::FormalizableNow)
        );
        assert!(!before.items.contains_key("B"));
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
