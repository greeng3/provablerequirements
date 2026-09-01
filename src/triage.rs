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

/// What produced a classification — and therefore how much it is worth.
///
/// A bucket assigned because nothing could judge the item is not the same fact as one a classifier
/// decided, and until #172 the two were written identically. Measured on a fresh subject: with no
/// provider configured, `triage` seeds `stays-prose` and says so *as it runs*; the record it leaves
/// says only `classification: stays-prose`. Once that message scrolls away nothing distinguishes
/// them — and `stays-prose` is the lifecycle state meaning *this item will not be formalized*. On
/// that subject the seed was wrong: configuring a model gave `formalizable-now` for the same item.
///
/// This is the rule the engine verdicts already follow (REQ032/REQ065) applied to triage: a state
/// reached because nothing could be determined must not present as a state something determined.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Origin {
    /// A classifier read the item and judged it.
    Classified,
    /// No classifier ran. Seeded from the item's own verification hint where the source carried
    /// one, and from the prose floor where it did not — see [`ProseFloorClassifier`]. A later run
    /// replaces this without `--reclassify`, because there is no judgement here to overwrite.
    Seeded,
    /// The operator set it by hand ([`set`]). Never replaced by an automatic run.
    Operator,
    /// Read off the companion record, not judged: the item has a stored verdict or an admitted
    /// formalization, so the claim demonstrably *has* been lowered (#265, part of #259). Stronger
    /// than a classification rather than weaker — a model was never asked, because the record
    /// already answered. Recorded distinctly because the alternatives both misstate it:
    /// `Classified` would claim a classifier ran, and `Seeded` means *nothing was determined* and
    /// is replaceable by any later run.
    Demonstrated,
    /// Written before provreq recorded this (#172). **Not** assumed to be either: an old entry may
    /// have come from a real classifier or from a seed, and guessing would be the very thing this
    /// enum exists to stop. Left alone by an automatic run, like a judgement.
    #[default]
    Unrecorded,
}

impl Origin {
    /// Whether an ordinary (non-`--reclassify`) run may replace an entry of this origin. True only
    /// for a seed: there is nothing there to overwrite. A judgement, an operator's own choice, and
    /// an entry whose provenance is unknown are all left alone.
    pub fn is_replaceable_by_a_plain_run(self) -> bool {
        matches!(self, Origin::Seeded)
    }

    /// Whether an automatic run must leave this entry alone whatever flags it was given —
    /// including `--reclassify`, which exists to replace *judgements*. An operator's choice is
    /// theirs, and a demonstration is not a judgement to replace: the record that produced it is
    /// still there, so re-deriving it is the same answer and asking a model is pure waste (#265).
    pub fn survives_a_reclassify(self) -> bool {
        matches!(self, Origin::Operator | Origin::Demonstrated)
    }

    /// How this origin reads on a surface that lists classifications. Empty for a real
    /// classification, which needs no annotation — only the ones carrying less than they appear to.
    pub fn note(self) -> &'static str {
        match self {
            Origin::Classified => "",
            Origin::Seeded => "seeded — no classifier ran",
            Origin::Operator => "set by the operator",
            Origin::Demonstrated => "demonstrated by the record — no classifier was asked",
            Origin::Unrecorded => "origin not recorded",
        }
    }
}

/// One item's triage record.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct TriageEntry {
    pub classification: Classification,
    /// Source revision this classification was made against (R-src-3); lets a
    /// later slice flag drift. Advisory only — re-triage is always allowed.
    pub revision: String,
    /// What produced this classification (#172). Defaults to [`Origin::Unrecorded`] so a
    /// `triage.yml` written before this field existed loads unchanged and claims nothing.
    #[serde(default)]
    pub origin: Origin,
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

/// Bulk pre-sorts a backlog into advisory buckets (R-triage-1). Returns exactly one **slot** per
/// input item, in order, and a slot may be empty: `None` is a classifier declining to place that
/// item, which leaves it un-triaged rather than pushing it into a bucket (REQ052, #226). There is
/// no bucket that asserts nothing, so a classifier with nothing to say must be able to say nothing.
/// Fallible (an LLM classifier does I/O) and async; dispatched generically so no trait objects are
/// needed. Every output is a seed the operator still confirms.
pub trait Classifier {
    fn classify(
        &self,
        items: &[Item],
    ) -> impl std::future::Future<Output = Result<Vec<Option<Classification>>>> + Send;

    /// What this classifier's answers are worth, recorded on every entry it writes (#172). The
    /// default is [`Origin::Classified`], because a classifier that reads an item and judges it is
    /// the ordinary case; the honest floor overrides it, since it judges nothing.
    fn origin(&self) -> Origin {
        Origin::Classified
    }
}

/// Adapter #0: the honest floor. Seeds each item from its source verification
/// hint when present (R-triage-2), else `stays-prose` — never over-claiming
/// formalizability. The operator promotes items with `set`, or an LLM classifier
/// ([`crate::llm::LlmClassifier`]) pre-sorts them.
pub struct ProseFloorClassifier;

impl Classifier for ProseFloorClassifier {
    /// Deliberately still fills every slot, unlike the LLM classifier (#226). This is the adapter
    /// an operator gets when no model is configured, and it exists to give the backlog a starting
    /// state at all; returning `None` throughout would make `triage` a no-op that reports nothing.
    /// What makes that honest is [`Origin::Seeded`] on every entry it writes — a bucket nothing
    /// judged, labelled as such wherever it is shown (#172, #180). The LLM classifier has no such
    /// excuse: it *did* read the item, so declining to place one is a real answer.
    async fn classify(&self, items: &[Item]) -> Result<Vec<Option<Classification>>> {
        Ok(items
            .iter()
            .map(|i| Some(i.verification_hint.unwrap_or(Classification::StaysProse)))
            .collect())
    }

    /// Nothing here judged anything: an item is seeded from its own source hint, or from the floor.
    /// Both are worth strictly less than a classification and are recorded as such (#172).
    fn origin(&self) -> Origin {
        Origin::Seeded
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
    let origin = classifier.origin();
    let mut next = state.items.clone();
    for (item, classification) in batch.iter().zip(buckets) {
        // A declined item is left exactly as it was — which for an untriaged item means still
        // untriaged, and for one already classified means the earlier answer stands (#226). Writing
        // an entry here would be the tool inventing a judgement out of the model's silence, and
        // erasing an existing one would let a decline destroy a classification somebody made.
        let Some(classification) = classification else {
            continue;
        };
        next.insert(
            item.id.clone(),
            TriageEntry {
                classification,
                revision: item.revision.clone(),
                origin,
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
                });
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
            origin: Origin::Operator,
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
    /// No item may be touched: everything is already classified and the operator did not ask for
    /// a re-run — or asked for one, and every entry is their own choice, which a re-run never
    /// replaces (#257).
    Nothing { already: usize },
    /// There is work: exactly these items go in front of the classifier. An ordinary run lists
    /// only the untriaged and seeded ones; a re-classify lists the judged ones too, and
    /// `operator_kept` names the hand-set entries it is leaving alone so the caller can say so
    /// (REQ010) — empty on an ordinary run, where nothing claimed to replace them.
    Classify {
        pending: Vec<Item>,
        operator_kept: Vec<String>,
        /// Entries the record demonstrates, which a re-classify also leaves alone (#265). Named
        /// apart from `operator_kept` because a run that keeps entries must say *which kind* it
        /// kept (#257): "we kept your choices" and "we kept what the store already proves" are
        /// different facts, and reporting them as one would credit the operator for neither.
        demonstrated_kept: Vec<String>,
    },
}

/// Decide a triage run's scope. Pure, so the caller can report the decision rather than narrate an
/// intention — the same shape as [`crate::adopt::plan`] and [`crate::provision::decide_install`].
///
/// An ordinary run takes the untriaged items **and the seeded ones** (#172). A seed is what the tool
/// wrote when nothing could judge the item, so replacing it overwrites no judgement — whereas
/// `--reclassify` exists to replace judgements and is consent-gated for that reason. Without this,
/// an operator who ran `triage` before configuring a provider had only one way to get their backlog
/// judged: `--reclassify`, which also discards every classification they had set by hand. That is a
/// choice between keeping nothing and re-running everything, and neither is what they wanted.
///
/// A re-classify replaces *judgements* — the classifier's own answers, and entries whose provenance
/// was never recorded — and **never an operator's choice** (#257). `Origin::Operator` has said
/// "never replaced by an automatic run" since #172, while this function put every item back in
/// front of the classifier: one flag press, behind a consent prompt that never mentioned the
/// hand-set entries and that `--yes` skips entirely, replaced all of them with whatever the model
/// said this time. `--set` is the way to change an operator's answer, so nothing is lost by keeping
/// them — and the kept ids are returned so the caller reports the keeping instead of doing it
/// silently (REQ010).
pub fn plan(state: &TriageState, items: &[Item], reclassify: bool) -> TriagePlan {
    if reclassify {
        let origin_of = |i: &Item| state.items.get(&i.id).map(|e| e.origin);
        let (kept, pending): (Vec<&Item>, Vec<&Item>) = items
            .iter()
            .partition(|i| origin_of(i).is_some_and(Origin::survives_a_reclassify));
        if pending.is_empty() {
            return TriagePlan::Nothing {
                already: items.len(),
            };
        }
        let kept_with = |want: Origin| -> Vec<String> {
            kept.iter()
                .filter(|i| origin_of(i) == Some(want))
                .map(|i| i.id.clone())
                .collect()
        };
        return TriagePlan::Classify {
            pending: pending.into_iter().cloned().collect(),
            operator_kept: kept_with(Origin::Operator),
            demonstrated_kept: kept_with(Origin::Demonstrated),
        };
    }
    let pending: Vec<Item> = items
        .iter()
        .filter(|i| match state.items.get(&i.id) {
            None => true,
            Some(entry) => entry.origin.is_replaceable_by_a_plain_run(),
        })
        .cloned()
        .collect();
    if pending.is_empty() {
        TriagePlan::Nothing {
            already: items.len(),
        }
    } else {
        TriagePlan::Classify {
            pending,
            operator_kept: Vec::new(),
            demonstrated_kept: Vec::new(),
        }
    }
}

/// Record what the companion record already **demonstrates** about each item, before any classifier
/// is asked (#265, part of #259).
///
/// `is_demonstrated` answers the only question that matters here: does the record show this claim
/// has already been lowered? In production that is "the verdict store holds a verdict for it" or
/// "its draft is admitted with a candidate and bindings" — either way the claim is
/// [`Classification::FormalizableNow`] by demonstration, not by judgement. Taken as a predicate so
/// this stays pure and testable without a subject on disk, like [`plan`].
///
/// This exists because a classifier reading prose cannot see it. Measured in PR #258: REQ047 was
/// classified `falsifiable-only` while the verdict store held a Kani `holds` for REQ047 — the tool
/// contradicting its own record. No prompt fixes that, because the refutation is not in the prose.
///
/// An operator's own choice is never overwritten, here as everywhere else (REQ010).
///
/// Returns the new state and the ids this call actually **changed** — not the ones it inserted.
/// Those differ, and the difference is not academic: on the real subject REQ047 already carried an
/// `unrecorded` entry, so a run that reported how much the map grew announced nothing while
/// upgrading that entry's provenance. A second run over the same record changes nothing and so
/// reports nothing, which is what keeps the message a report rather than a recurring banner.
pub fn apply_demonstrated(
    state: &TriageState,
    items: &[Item],
    is_demonstrated: impl Fn(&Item) -> bool,
) -> (TriageState, Vec<String>) {
    let mut next = state.items.clone();
    let mut changed = Vec::new();
    for item in items.iter().filter(|i| is_demonstrated(i)) {
        let entry = TriageEntry {
            classification: Classification::FormalizableNow,
            revision: item.revision.clone(),
            origin: Origin::Demonstrated,
        };
        match next.get(&item.id) {
            Some(e) if e.origin == Origin::Operator => continue,
            Some(e) if *e == entry => continue,
            _ => {}
        }
        next.insert(item.id.clone(), entry);
        changed.push(item.id.clone());
    }
    (
        TriageState {
            schema: state.schema,
            items: next,
        },
        changed,
    )
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
        let TriagePlan::Classify { pending, .. } = plan(state, items, false) else {
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
            expects_code_trace: None,
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
            vec![
                Some(Classification::StaysProse),
                Some(Classification::FormalizableNow)
            ],
            "the prose floor still fills every slot — declining is the LLM classifier's answer, \
             not this one's (#226)"
        );
    }

    /// A classifier that actually judges — the default [`Origin::Classified`]. Stands in for the
    /// LLM one wherever a test needs a *classification* rather than a seed.
    struct JudgingClassifier;

    impl Classifier for JudgingClassifier {
        async fn classify(&self, items: &[Item]) -> Result<Vec<Option<Classification>>> {
            Ok(vec![Some(Classification::FormalizableNow); items.len()])
        }
    }

    /// Places nothing at all — the shape of a model that read every item and could commit to none.
    struct DecliningClassifier;

    impl Classifier for DecliningClassifier {
        async fn classify(&self, items: &[Item]) -> Result<Vec<Option<Classification>>> {
            Ok(vec![None; items.len()])
        }
    }

    // Verifies: REQ052 (#226) — a classifier that declines to place an item leaves it un-triaged,
    // and leaves an item somebody already classified exactly as it was. Two ways to get this wrong,
    // both worse than doing nothing: inventing an entry out of the model's silence, or letting that
    // silence erase a judgement that already existed.
    #[tokio::test]
    async fn a_declined_item_is_left_exactly_as_it_was() {
        let items = [item("A", None), item("B", None)];
        let before = seed_all(&TriageState::new(), &items, &JudgingClassifier)
            .await
            .unwrap();
        assert_eq!(before.items.len(), 2, "both start classified");

        let after = seed_all(&before, &items, &DecliningClassifier)
            .await
            .unwrap();
        assert_eq!(
            after.items.get("A").map(|e| e.classification),
            Some(Classification::FormalizableNow),
            "a decline must not destroy an existing classification"
        );

        let fresh = seed_all(&TriageState::new(), &items, &DecliningClassifier)
            .await
            .unwrap();
        assert!(
            fresh.items.is_empty(),
            "nothing was placed, so nothing is written — the items stay untriaged, which is the \
             one state that claims nothing about them"
        );
    }

    // Verifies (#172): a seed is recorded as a seed, and a judgement as a judgement. They used to
    // serialize identically, so once the "no `llm:` config — seeding with the prose-floor default"
    // message scrolled away nothing could tell them apart — and the seed can be WRONG: measured on
    // a real subject, the floor said `stays-prose` where a live model said `formalizable-now`.
    #[tokio::test]
    async fn a_seed_and_a_judgement_are_not_recorded_the_same_way() {
        let items = [item("A", None)];
        let seeded = seed_all(&TriageState::new(), &items, &ProseFloorClassifier)
            .await
            .unwrap();
        assert_eq!(seeded.items["A"].origin, Origin::Seeded);

        let judged = seed_all(&TriageState::new(), &items, &JudgingClassifier)
            .await
            .unwrap();
        assert_eq!(judged.items["A"].origin, Origin::Classified);

        // And the operator's own choice is neither.
        let chosen = set(&TriageState::new(), &items[0], Classification::StaysProse);
        assert_eq!(chosen.items["A"].origin, Origin::Operator);
    }

    // Verifies (#172): a seeded backlog is work for a real classifier WITHOUT `--reclassify`.
    // Before this, an operator who ran `triage` before configuring a provider had one way out —
    // `--reclassify`, which also discards every classification they had set by hand. That is a
    // choice between keeping nothing and re-running everything.
    #[tokio::test]
    async fn a_seeded_backlog_is_still_work_for_a_real_classifier() {
        let items = [item("A", None), item("B", None)];
        let seeded = seed_all(&TriageState::new(), &items, &ProseFloorClassifier)
            .await
            .unwrap();

        let TriagePlan::Classify { pending, .. } = plan(&seeded, &items, false) else {
            panic!("a seed is not a triage — both items are still work");
        };
        assert_eq!(pending.len(), 2);

        // And running it upgrades them in place, no consent gate needed: nothing was overwritten
        // but a seed.
        let judged = seed_all(&seeded, &items, &JudgingClassifier).await.unwrap();
        assert_eq!(judged.items["A"].origin, Origin::Classified);
        assert_eq!(
            judged.items["A"].classification,
            Classification::FormalizableNow
        );
    }

    // Verifies (#172, #257): what a plain run must NOT touch. An operator's own choice and an entry
    // whose provenance predates this field are both left alone — the first because it is a decision,
    // the second because guessing it was a seed would be the very over-claim this exists to stop.
    // `--reclassify`, which is consent-gated, replaces the unrecorded one; the operator's choice it
    // keeps, and names.
    #[tokio::test]
    async fn a_plain_run_replaces_only_seeds() {
        let items = [item("A", None), item("B", None)];
        let mut state = set(&TriageState::new(), &items[0], Classification::StaysProse);
        state.items.insert(
            "B".to_string(),
            TriageEntry {
                classification: Classification::StaysProse,
                revision: "rev-B".into(),
                origin: Origin::Unrecorded,
            },
        );

        assert_eq!(
            plan(&state, &items, false),
            TriagePlan::Nothing { already: 2 },
            "neither an operator's choice nor an unrecorded entry is a seed"
        );
        let TriagePlan::Classify {
            pending,
            operator_kept,
            ..
        } = plan(&state, &items, true)
        else {
            panic!("--reclassify is the consent-gated way to replace the unrecorded one");
        };
        assert_eq!(
            pending.iter().map(|i| i.id.as_str()).collect::<Vec<_>>(),
            vec!["B"],
            "the unrecorded entry is re-judged; the operator's is not"
        );
        assert_eq!(
            operator_kept,
            vec!["A".to_string()],
            "and the kept entry is named, so the caller reports the keeping (REQ010)"
        );
    }

    // Verifies: REQ010 (#257) — `--reclassify` replaces judgements, and an operator's own choice
    // is not a judgement it may replace. `Origin::Operator` has documented "never replaced by an
    // automatic run" since #172, while `plan` put every item back in front of the classifier — so
    // one flag press (consent-gated by a prompt that never mentioned them, skipped entirely by
    // `--yes`) replaced every hand-set classification with whatever the model said this time. The
    // committed tree carries 12 such entries. `--set` is the way to change an operator's answer.
    #[tokio::test]
    async fn reclassify_never_replaces_an_operators_choice() {
        let items = [item("A", None), item("B", None)];
        let mut state = set(&TriageState::new(), &items[0], Classification::StaysProse);
        state.items.insert(
            "B".to_string(),
            TriageEntry {
                classification: Classification::FormalizableNow,
                revision: "rev-B".into(),
                origin: Origin::Classified,
            },
        );

        let TriagePlan::Classify { pending, .. } = plan(&state, &items, true) else {
            panic!("the judged item is still real re-classify work");
        };
        assert_eq!(
            pending.iter().map(|i| i.id.as_str()).collect::<Vec<_>>(),
            vec!["B"],
            "the judgement goes back to the classifier; the operator's choice does not"
        );
    }

    // Verifies: REQ010 (#257) — a backlog that is *all* operator-set plans no work under
    // `--reclassify`, so the caller can say so instead of announcing a model it is not about to
    // ask (REQ053's shape, one flag over).
    #[tokio::test]
    async fn a_fully_operator_set_backlog_reclassifies_nothing() {
        let items = [item("A", None)];
        let state = set(&TriageState::new(), &items[0], Classification::StaysProse);
        assert_eq!(
            plan(&state, &items, true),
            TriagePlan::Nothing { already: 1 },
            "there is nothing here --reclassify may touch"
        );
    }

    // Verifies (#172): a `triage.yml` written before this field existed loads, and claims nothing.
    // Defaulting it to either real value would assert a provenance nobody recorded.
    #[test]
    fn an_entry_written_before_origin_existed_loads_as_unrecorded() {
        let yaml = "schema: 1\nitems:\n  A:\n    classification: stays-prose\n    revision: r1\n";
        let state: TriageState = serde_yaml::from_str(yaml).expect("old state must still load");
        assert_eq!(state.items["A"].origin, Origin::Unrecorded);
        assert!(
            !state.items["A"].origin.is_replaceable_by_a_plain_run(),
            "an unknown provenance is not assumed to be a seed"
        );
    }

    // Verifies: REQ053 — the scope of a run is decided before it is described. A fully-triaged
    // backlog is `Nothing`, so the caller can say "nothing to classify" instead of announcing a
    // model it is not about to ask. This is the defect: `triage` printed "Classifying backlog with
    // <model> via <url> …" and then classified nothing, output indistinguishable from a real run.
    #[tokio::test]
    async fn a_fully_triaged_backlog_plans_no_work() {
        let items = [item("A", None), item("B", None)];
        // JUDGED, not seeded. This fixture used the prose floor and #172 changed what that means: a
        // seed is not a triage, so a fully-seeded backlog is now real work for a real classifier
        // (see `a_seeded_backlog_is_still_work_for_a_real_classifier`). REQ053's point is unchanged
        // — do not announce a model and then classify nothing — and it needs a backlog something
        // actually decided.
        let judged = seed_all(&TriageState::new(), &items, &JudgingClassifier)
            .await
            .unwrap();

        assert_eq!(
            plan(&judged, &items, false),
            TriagePlan::Nothing { already: 2 }
        );

        // A partly-triaged backlog is real work, counted honestly: only the pending item.
        let partial = set(&TriageState::new(), &items[0], Classification::StaysProse);
        match plan(&partial, &items, false) {
            TriagePlan::Classify { pending, .. } => {
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
            TriagePlan::Classify { pending, .. } => {
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
        let TriagePlan::Classify { pending, .. } = plan(&before, &items, true) else {
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
            async fn classify(&self, _items: &[Item]) -> Result<Vec<Option<Classification>>> {
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
        async fn classify(&self, items: &[Item]) -> Result<Vec<Option<Classification>>> {
            let n = self.calls.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            if n >= self.fail_after {
                anyhow::bail!("the model returned no usable classification");
            }
            Ok(vec![Some(Classification::FormalizableNow); items.len()])
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
        let TriagePlan::Classify { pending, .. } = plan(&outcome.state, &items, false) else {
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

    // Verifies: #265/REQ010 — an item the record already answers never reaches the classifier, and
    // is recorded as demonstrated rather than as a judgement or a seed.
    #[test]
    fn a_demonstrated_item_is_recorded_off_the_record() {
        let items = [item("A", None), item("B", None)];
        let (next, _) = apply_demonstrated(&TriageState::new(), &items, |i| i.id == "A");

        let a = next.items.get("A").expect("A recorded");
        assert_eq!(a.classification, Classification::FormalizableNow);
        assert_eq!(a.origin, Origin::Demonstrated);
        assert!(!next.items.contains_key("B"), "B has no record to read");
    }

    // Verifies: #265 — the whole point. A demonstrated item is excluded from the classifier's
    // batch, so a model cannot contradict the store the way it did for REQ047.
    #[test]
    fn a_demonstrated_item_is_not_planned_for_the_classifier() {
        let items = [item("A", None), item("B", None)];
        let (state, _) = apply_demonstrated(&TriageState::new(), &items, |i| i.id == "A");

        let TriagePlan::Classify { pending, .. } = plan(&state, &items, false) else {
            panic!("B still needs classifying");
        };
        let ids: Vec<&str> = pending.iter().map(|i| i.id.as_str()).collect();
        assert_eq!(ids, ["B"], "only the item the record cannot answer");
    }

    // Verifies: #265 — `--reclassify` replaces judgements, but a demonstration is not a judgement;
    // re-asking a model about an item the record answers is the waste this removes.
    #[test]
    fn reclassify_keeps_demonstrated_entries_and_names_them_separately() {
        let items = [item("A", None), item("B", None), item("C", None)];
        let (mut state, _) = apply_demonstrated(&TriageState::new(), &items, |i| i.id == "A");
        state = set(&state, &items[1], Classification::StaysProse);

        let TriagePlan::Classify {
            pending,
            operator_kept,
            demonstrated_kept,
        } = plan(&state, &items, true)
        else {
            panic!("C is still classifiable");
        };
        let ids: Vec<&str> = pending.iter().map(|i| i.id.as_str()).collect();
        assert_eq!(ids, ["C"]);
        assert_eq!(operator_kept, ["B"], "the operator's own choice");
        assert_eq!(
            demonstrated_kept,
            ["A"],
            "named apart from the operator's (#257)"
        );
    }

    // Verifies: #265 — an automatic run never overwrites an operator's choice, and a demonstration
    // is an automatic run.
    #[test]
    fn a_demonstration_never_overwrites_an_operator() {
        let items = [item("A", None)];
        let state = set(&TriageState::new(), &items[0], Classification::StaysProse);

        let (next, _) = apply_demonstrated(&state, &items, |_| true);

        let a = next.items.get("A").expect("A still recorded");
        assert_eq!(a.classification, Classification::StaysProse);
        assert_eq!(a.origin, Origin::Operator);
    }

    // Verifies: #265 — a demonstrated entry carries a note saying where it came from, like every
    // other origin that is not a plain classification (#172).
    #[test]
    fn demonstrated_says_what_it_is() {
        assert!(!Origin::Demonstrated.note().is_empty());
        assert!(!Origin::Demonstrated.is_replaceable_by_a_plain_run());
    }

    // Verifies: #265 — an item that ALREADY had an entry is still a change worth reporting. The
    // live run on the real subject caught this: REQ047 carried an `unrecorded` entry, so counting
    // how much the map grew reported nothing while the origin was in fact being upgraded.
    #[test]
    fn a_demonstration_reports_the_entries_it_changed_not_the_ones_it_added() {
        let items = [item("A", None)];
        let mut state = TriageState::new();
        state.items.insert(
            "A".into(),
            TriageEntry {
                classification: Classification::FalsifiableOnly,
                revision: "rev-A".into(),
                origin: Origin::Unrecorded,
            },
        );

        let (next, changed) = apply_demonstrated(&state, &items, |_| true);
        assert_eq!(changed, ["A"], "the origin was upgraded — say so");
        assert_eq!(next.items["A"].origin, Origin::Demonstrated);
        assert_eq!(
            next.items["A"].classification,
            Classification::FormalizableNow
        );

        let (_, again) = apply_demonstrated(&next, &items, |_| true);
        assert!(
            again.is_empty(),
            "a second run changed nothing and must announce nothing"
        );
    }
}
