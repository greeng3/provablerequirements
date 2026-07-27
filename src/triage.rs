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

/// Classify exactly `batch` and merge the buckets onto `state`, replacing any entry those items
/// already had. Deciding *which* items to ask about is [`plan`]'s job, not this one's. Returns a
/// new state.
async fn classify_into<C: Classifier>(
    state: &TriageState,
    batch: &[Item],
    classifier: &C,
) -> Result<TriageState> {
    let buckets = classifier.classify(batch).await?;
    let mut next = state.items.clone();
    for (item, classification) in batch.iter().zip(buckets) {
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

/// What a batched seed run actually accomplished (REQ054).
///
/// A run that stops early is still a run that did something. Reporting only the failure would
/// throw away work the operator paid for, and reporting only the state would hide that the
/// backlog is not fully triaged.
pub struct SeedOutcome {
    /// Every batch that landed, merged onto the state the run started from. Already handed to the
    /// caller's `persist` sink.
    pub state: TriageState,
    /// How many of the planned items the classifier assigned a bucket to.
    pub classified: usize,
    /// How many the run never got to. They keep whatever they had — nothing is defaulted on their
    /// behalf (REQ052), so an item that was untriaged stays untriaged.
    pub unclassified: usize,
    /// Why the run stopped, when it stopped before the end.
    pub stopped: Option<anyhow::Error>,
}

/// Classify `pending` in batches, handing each batch's merged state to `persist` as it lands
/// (REQ054). `state` is what the batches are merged onto and what a stopped run leaves behind.
///
/// One request over a whole backlog is bounded by nothing an operator can predict: it gets slower
/// with backlog size — the one thing bulk pre-sort exists for — and a failure at any point loses
/// every item's worth of model work. Batching makes the per-request bound a per-batch bound and
/// turns a retry into a resume: what landed is already recorded, so [`plan`] finds only the rest.
///
/// Batches merge onto the *current* state, including on a re-classify. An operator who consents to
/// replacing their classifications consents to a complete replacement, not to trading fifty of them
/// for the two a stopped run managed — so at every point the persisted state covers every item it
/// covered before.
pub async fn seed_in_batches<C: Classifier>(
    state: &TriageState,
    pending: &[Item],
    classifier: &C,
    batch_size: usize,
    mut persist: impl FnMut(&TriageState, usize, usize) -> Result<()>,
) -> Result<SeedOutcome> {
    let total = pending.len();
    let mut next = state.clone();
    let mut classified = 0;
    // `batch_size` comes from operator config; 0 would panic in `chunks`.
    for batch in pending.chunks(batch_size.max(1)) {
        next = match classify_into(&next, batch, classifier).await {
            Ok(merged) => merged,
            Err(stopped) => {
                return Ok(SeedOutcome {
                    state: next,
                    classified,
                    unclassified: total - classified,
                    stopped: Some(stopped),
                })
            }
        };
        // A classifier returns exactly one bucket per input item, by contract.
        classified += batch.len();
        persist(&next, classified, total)?;
    }
    Ok(SeedOutcome {
        state: next,
        classified,
        unclassified: 0,
        stopped: None,
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
    /// There is work: exactly these items go in front of the classifier. An ordinary run lists
    /// only the untriaged ones (classification is additive); a re-classify lists them all.
    Classify { pending: Vec<Item> },
}

/// Decide a triage run's scope. Pure, so the caller can report the decision rather than narrate an
/// intention — the same shape as [`crate::adopt::plan`] and [`crate::provision::decide_install`].
pub fn plan(state: &TriageState, items: &[Item], reclassify: bool) -> TriagePlan {
    if reclassify {
        return TriagePlan::Classify {
            pending: items.to_vec(),
        };
    }
    let pending: Vec<Item> = items
        .iter()
        .filter(|i| !state.items.contains_key(&i.id))
        .cloned()
        .collect();
    if pending.is_empty() {
        TriagePlan::Nothing {
            already: items.len(),
        }
    } else {
        TriagePlan::Classify { pending }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The whole production path in one call — decide the scope, then classify it in a single
    /// batch — so these tests exercise what the CLI actually runs.
    async fn seed_all<C: Classifier>(
        state: &TriageState,
        items: &[Item],
        classifier: &C,
    ) -> Result<TriageState> {
        let TriagePlan::Classify { pending } = plan(state, items, false) else {
            return Ok(state.clone());
        };
        let batch = pending.len();
        let outcome = seed_in_batches(state, &pending, classifier, batch, |_, _, _| Ok(())).await?;
        match outcome.stopped {
            Some(stopped) => Err(stopped),
            None => Ok(outcome.state),
        }
    }

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
        let seeded = seed_all(&TriageState::new(), &items, &ProseFloorClassifier)
            .await
            .unwrap();

        assert_eq!(
            plan(&seeded, &items, false),
            TriagePlan::Nothing { already: 2 }
        );

        // A partly-triaged backlog is real work, counted honestly: only the pending item.
        let partial = set(&TriageState::new(), &items[0], Classification::StaysProse);
        match plan(&partial, &items, false) {
            TriagePlan::Classify { pending } => {
                assert_eq!(pending.len(), 1, "only the untriaged item is work");
                assert_eq!(pending[0].id, "B");
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
        let seeded = seed_all(&TriageState::new(), &items, &ProseFloorClassifier)
            .await
            .unwrap();

        match plan(&seeded, &items, true) {
            TriagePlan::Classify { pending } => {
                assert_eq!(pending.len(), 2, "nothing is treated as already done");
            }
            other => panic!("expected work, got {other:?}"),
        }
    }

    // Verifies: REQ054 — a re-classify that stops early must not cost the operator the
    // classifications it did not get around to replacing. Consent to a full replacement is not
    // consent to trading a whole backlog for the one batch that landed, so batches merge onto the
    // current state and every item stays covered throughout.
    #[tokio::test]
    async fn a_stopped_reclassify_still_covers_every_item() {
        let ids: Vec<String> = (0..4).map(|n| format!("REQ{n}")).collect();
        let items: Vec<Item> = ids.iter().map(|id| item(id, None)).collect();
        let before = seed_all(&TriageState::new(), &items, &ProseFloorClassifier)
            .await
            .unwrap();
        let TriagePlan::Classify { pending } = plan(&before, &items, true) else {
            panic!("a reclassify is always work");
        };

        let outcome = seed_in_batches(
            &before,
            &pending,
            &FlakyClassifier {
                fail_after: 1,
                calls: 0.into(),
            },
            2,
            |_, _, _| Ok(()),
        )
        .await
        .unwrap();

        assert!(outcome.stopped.is_some());
        assert_eq!(
            outcome.state.items.len(),
            4,
            "no item lost its classification"
        );
        assert_eq!(
            outcome.state.items["REQ0"].classification,
            Classification::FormalizableNow,
            "the batch that landed replaced its items"
        );
        assert_eq!(
            outcome.state.items["REQ3"].classification,
            Classification::StaysProse,
            "an item whose batch never ran keeps what it had"
        );
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

        let err = seed_all(&before, &items, &FailingClassifier)
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

    /// Succeeds for `fail_after` calls, then fails the way a real endpoint does mid-backlog.
    struct FlakyClassifier {
        fail_after: usize,
        calls: std::sync::atomic::AtomicUsize,
    }
    impl Classifier for FlakyClassifier {
        async fn classify(&self, items: &[Item]) -> Result<Vec<Classification>> {
            let n = self.calls.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            if n >= self.fail_after {
                anyhow::bail!("the model returned no usable classification");
            }
            Ok(vec![Classification::FormalizableNow; items.len()])
        }
    }

    // Verifies: REQ054 — a batch that lands is persisted before the next one is asked for, so a
    // failure mid-backlog keeps the work that succeeded and a retry resumes rather than restarts.
    // This is the defect: one request over all 51 items ran for ten minutes, timed out, and left
    // nothing behind.
    #[tokio::test]
    async fn a_failed_batch_keeps_the_batches_that_landed_and_a_retry_resumes() {
        let ids: Vec<String> = (0..5).map(|n| format!("REQ{n}")).collect();
        let items: Vec<Item> = ids.iter().map(|id| item(id, None)).collect();
        let classifier = FlakyClassifier {
            fail_after: 2,
            calls: 0.into(),
        };

        let mut persisted = Vec::new();
        let outcome = seed_in_batches(
            &TriageState::new(),
            &items,
            &classifier,
            2,
            |s, done, total| {
                persisted.push((s.items.len(), done, total));
                Ok(())
            },
        )
        .await
        .unwrap();

        assert_eq!(
            persisted,
            vec![(2, 2, 5), (4, 4, 5)],
            "each batch is persisted as it lands, and progress is reported against the whole run"
        );
        assert!(outcome.stopped.is_some(), "the failure is still reported");
        assert_eq!(outcome.classified, 4);
        assert_eq!(
            outcome.unclassified, 1,
            "the rest is left honestly untriaged"
        );
        assert_eq!(outcome.state.items.len(), 4);

        // A retry over the persisted state asks only about what is left, and does not redo — or
        // undo — the work that landed.
        let TriagePlan::Classify { pending } = plan(&outcome.state, &items, false) else {
            panic!("one item is still untriaged, so a retry has work");
        };
        let resumed = seed_in_batches(
            &outcome.state,
            &pending,
            &ProseFloorClassifier,
            2,
            |_, _, _| Ok(()),
        )
        .await
        .unwrap();
        assert!(resumed.stopped.is_none());
        assert_eq!(resumed.classified, 1, "only the untriaged item is re-asked");
        assert_eq!(
            resumed.state.items["REQ0"].classification,
            Classification::FormalizableNow,
            "a resume does not overwrite what the earlier batches established"
        );
    }

    // Verifies: REQ054 — the batch size is operator config, so the degenerate value must not take
    // the run down with it (`chunks(0)` panics).
    #[tokio::test]
    async fn a_zero_batch_size_still_classifies_the_backlog() {
        let items = [item("A", None), item("B", None)];
        let outcome = seed_in_batches(
            &TriageState::new(),
            &items,
            &ProseFloorClassifier,
            0,
            |_, _, _| Ok(()),
        )
        .await
        .unwrap();
        assert_eq!(outcome.classified, 2);
    }

    // Verifies: REQ010 — seeding fills only unclassified items; set overrides.
    #[tokio::test]
    async fn seed_is_additive_and_set_overrides() {
        let items = [item("A", None), item("B", None)];
        let seeded = seed_all(&TriageState::new(), &items, &ProseFloorClassifier)
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
        let reseeded = seed_all(&promoted, &items, &ProseFloorClassifier)
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
        let state = seed_all(&TriageState::new(), &items, &ProseFloorClassifier)
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
