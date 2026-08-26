//! Coverage funnel (R-cov-1): `discovered → triaged → formalized → verified`,
//! keyed by item id, with the honest states kept distinct — un-triaged is not
//! stays-prose is not formalizable-but-not-yet-formalized. Extends the A4
//! traceability model on the triage axis.
//!
//! Implements: REQ011 (report requirement coverage as an honest funnel)

use crate::draft::DraftState;
use crate::source::{Classification, Item};
use crate::triage::TriageState;
use crate::verdict_store::{self, DriftAnchor, VerdictStore, VerdictView};

/// A snapshot of where every discovered item sits in the funnel.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct Coverage {
    pub discovered: usize,
    pub untriaged: usize,
    pub formalizable_now: usize,
    pub falsifiable_only: usize,
    pub stays_prose: usize,
    /// Step 3 — items with an in-progress formalization draft that is not yet admitted.
    /// An overlay on the formalizable subset, kept distinct from `formalized`.
    pub drafting: usize,
    /// Step 3 — admitted formalizations (D12): a draft the operator has confirmed.
    pub formalized: usize,
    /// Step 4/6 — requirements with a stored `holds` verdict that is still **fresh** (REQ039). A
    /// verdict that has drifted (its prose, code, or tool moved) drops out of the count until
    /// re-verified, so the funnel reflects what is *currently* known to hold, never a stale claim.
    pub verified: usize,
    /// Step 6 — requirements with a stored verdict that has **drifted** and no longer applies, of
    /// any polarity (REQ043). The re-verify worklist: a drifted `holds` left `verified`, a drifted
    /// `fails` is a stale refutation — both owe a re-run. Aggregates the per-item drift the living
    /// loop already detects, so the operator sees *how much* is owed, not just per-row markers.
    pub stale: usize,
}

/// Compute the funnel for `items` given the current `triage`, `drafts`, and stored `verdicts`
/// state. `anchor` pins the current world (subject commit + tool version) so a drifted verdict is
/// not counted as verified.
pub fn coverage(
    items: &[Item],
    triage: &TriageState,
    drafts: &DraftState,
    verdicts: &VerdictStore,
    anchor: &DriftAnchor,
) -> Coverage {
    let mut cov = Coverage {
        discovered: items.len(),
        untriaged: 0,
        formalizable_now: 0,
        falsifiable_only: 0,
        stays_prose: 0,
        drafting: 0,
        formalized: 0,
        verified: 0,
        stale: 0,
    };
    for item in items {
        match triage.items.get(&item.id).map(|e| e.classification) {
            None => cov.untriaged += 1,
            Some(Classification::FormalizableNow) => cov.formalizable_now += 1,
            Some(Classification::FalsifiableOnly) => cov.falsifiable_only += 1,
            Some(Classification::StaysProse) => cov.stays_prose += 1,
        }
        match drafts.drafts.get(&item.id) {
            Some(d) if d.is_admitted() => cov.formalized += 1,
            Some(_) => cov.drafting += 1,
            None => {}
        }
        if let Some(view) = verdict_view(item, drafts, verdicts, anchor) {
            if !view.fresh {
                cov.stale += 1;
            } else if view.status == "holds" {
                cov.verified += 1;
            }
        }
    }
    cov
}

/// The stored verdict for one item paired with its freshness against `anchor`, or `None` when the
/// item has never been verified. The single seam both the funnel and the per-item row read through.
/// Reads the item's *currently admitted* formalization fingerprint from `drafts` so a verdict that
/// no longer matches the live formal input is seen as drifted (REQ045).
fn verdict_view(
    item: &Item,
    drafts: &DraftState,
    verdicts: &VerdictStore,
    anchor: &DriftAnchor,
) -> Option<VerdictView> {
    let formalization = crate::draft::admitted_fingerprint(drafts, &item.id);
    verdicts
        .verdicts
        .get(&item.id)
        .map(|stored| verdict_store::view(stored, &item.revision, formalization.as_deref(), anchor))
}

/// Where one item's formalization sits (Step 3): no draft, an in-progress draft, or an admitted
/// formalization. The per-item peer of the funnel's `drafting`/`formalized` totals.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Formalization {
    None,
    Drafting,
    Admitted,
}

/// One item's read-only funnel state, for the browse surface: its identity and prose alongside
/// the triage classification (`None` = untriaged), formalization state, and — the living-loop
/// surface (REQ039) — its last stored verdict paired with whether that verdict is still fresh.
/// `verdict` is the *stored* result, not a fresh engine run; a passive listing never runs an engine.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct ItemState {
    pub id: String,
    pub title: Option<String>,
    pub text: String,
    pub classification: Option<Classification>,
    /// What produced that classification (#172). Carried alongside it so a browse surface can tell
    /// a bucket a classifier judged from one seeded because nothing could — they are different
    /// facts and used to serialize identically. `None` when the item is untriaged.
    pub classified_by: Option<crate::triage::Origin>,
    pub formalization: Formalization,
    /// The last stored verdict + its drift status; `None` when the item has never been verified.
    pub verdict: Option<VerdictView>,
}

/// Pair every discovered item with its current triage + formalization + stored-verdict state, in
/// `items` order. Pure over the same inputs as [`coverage`], so the browse API is testable without
/// a server.
pub fn backlog(
    items: &[Item],
    triage: &TriageState,
    drafts: &DraftState,
    verdicts: &VerdictStore,
    anchor: &DriftAnchor,
) -> Vec<ItemState> {
    items
        .iter()
        .map(|item| {
            let formalization = match drafts.drafts.get(&item.id) {
                Some(d) if d.is_admitted() => Formalization::Admitted,
                Some(_) => Formalization::Drafting,
                None => Formalization::None,
            };
            ItemState {
                id: item.id.clone(),
                title: item.title.clone(),
                text: item.text.clone(),
                classification: triage.items.get(&item.id).map(|e| e.classification),
                classified_by: triage.items.get(&item.id).map(|e| e.origin),
                formalization,
                verdict: verdict_view(item, drafts, verdicts, anchor),
            }
        })
        .collect()
}

/// The re-verify worklist: every item whose stored verdict has drifted, in `items` order.
///
/// The funnel's `stale` count and this list are the same fact at two resolutions, and the count
/// alone was never actionable. `status` tells the operator to run `provreq verify <ID>` without
/// ever saying what `<ID>` is — guessable on a one-item subject, not on a 51-item backlog. The
/// browser has had the per-item reasons since REQ039; the command line had only the number, so the
/// living loop was complete on one surface and absent on the other for the same subject at the
/// same moment (#179).
///
/// A filter over [`backlog`] rather than a second traversal: one definition of "drifted" — the
/// `fresh` flag [`verdict_store::view`] already computes — keeps the list and the count from ever
/// disagreeing about which items are owed.
pub fn stale_worklist(
    items: &[Item],
    triage: &TriageState,
    drafts: &DraftState,
    verdicts: &VerdictStore,
    anchor: &DriftAnchor,
) -> Vec<ItemState> {
    backlog(items, triage, drafts, verdicts, anchor)
        .into_iter()
        .filter(|state| state.verdict.as_ref().is_some_and(|v| !v.fresh))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::draft::{self, DraftState};
    use crate::triage::{set, TriageState};
    use crate::verdict::{ProvenanceReport, VerdictReport};

    fn item(id: &str) -> Item {
        Item {
            id: id.into(),
            text: id.into(),
            revision: id.into(),
            title: None,
            verification_hint: None,
        }
    }

    fn anchor() -> DriftAnchor {
        DriftAnchor {
            spec_fingerprint: None,
            trace_fingerprint: None,
            ui_fingerprint: None,
            environment: crate::proving_env::ProvingEnv::default(),
            subject_commit: Some("head".into()),
            tool_version: "0.0.1".into(),
            source_fingerprint: None,
        }
    }

    /// A stored `holds` verdict pinned to `revision` + the test anchor (so it reads fresh unless the
    /// item's revision is changed).
    fn holds_verdict(id: &str, revision: &str) -> VerdictReport {
        VerdictReport {
            id: id.into(),
            status: "holds".into(),
            basis: Some("proven".into()),
            reason: None,
            witness: None,
            detail: vec![],
            evidence: vec![],
            correspondence: "mechanical".into(),
            provenance: ProvenanceReport {
                spec_fingerprint: None,
                trace_fingerprint: None,
                ui_fingerprint: None,
                tagged_source_fingerprint: None,
                environment: None,
                requirement_revision: revision.into(),
                subject_commit: Some("head".into()),
                tool_version: "0.0.1".into(),
                source_fingerprint: None,
                formalization: None,
            },
        }
    }

    fn store(entries: Vec<VerdictReport>) -> VerdictStore {
        let mut s = VerdictStore::new();
        for v in entries {
            s = verdict_store::record(&s, v);
        }
        s
    }

    // Verifies: REQ011 — untriaged, stays-prose, and formalizable are distinct
    // funnel states, and unbuilt stages report an honest zero.
    #[tokio::test]
    async fn funnel_keeps_states_distinct() {
        let items = [item("A"), item("B"), item("C")];
        let no_drafts = DraftState::new();
        let no_verdicts = VerdictStore::new();

        // Nothing triaged yet.
        let empty = coverage(
            &items,
            &TriageState::new(),
            &no_drafts,
            &no_verdicts,
            &anchor(),
        );
        assert_eq!(empty.discovered, 3);
        assert_eq!(empty.untriaged, 3);

        // Seed all to prose, then promote A.
        let seeded = items.iter().fold(TriageState::new(), |s, i| {
            set(&s, i, Classification::StaysProse)
        });
        let promoted = set(&seeded, &items[0], Classification::FormalizableNow);
        let cov = coverage(&items, &promoted, &no_drafts, &no_verdicts, &anchor());
        assert_eq!(cov.untriaged, 0);
        assert_eq!(cov.formalizable_now, 1);
        assert_eq!(cov.stays_prose, 2);
        assert_eq!(cov.drafting, 0);
        assert_eq!(cov.formalized, 0);
        assert_eq!(cov.verified, 0);
    }

    // Verifies: REQ013/REQ011/REQ019 — an in-progress draft counts as `drafting`;
    // once admitted it moves to `formalized` and out of `drafting`.
    #[test]
    fn admitted_draft_moves_from_drafting_to_formalized() {
        let items = [item("A"), item("B")];
        let no_verdicts = VerdictStore::new();
        let drafts = draft::open(&DraftState::new(), &items[0]);
        let cov = coverage(
            &items,
            &TriageState::new(),
            &drafts,
            &no_verdicts,
            &anchor(),
        );
        assert_eq!(cov.drafting, 1);
        assert_eq!(cov.formalized, 0);

        let admitted = draft::admit(&drafts, "A", draft::ReviewTier::Optional, "gg", 1);
        let cov = coverage(
            &items,
            &TriageState::new(),
            &admitted,
            &no_verdicts,
            &anchor(),
        );
        assert_eq!(cov.drafting, 0);
        assert_eq!(cov.formalized, 1);
    }

    // Verifies: REQ039 — the funnel counts only fresh `holds` verdicts, and a per-item row surfaces
    // the stored verdict with its drift status. A verdict whose prose moved drops out of `verified`.
    #[test]
    fn verified_counts_only_fresh_holds_and_row_surfaces_drift() {
        let items = [item("A"), item("B")];
        // A: fresh holds (revision matches). B: holds but its prose moved (revision != stored).
        let verdicts = store(vec![holds_verdict("A", "A"), holds_verdict("B", "old-rev")]);

        let cov = coverage(
            &items,
            &TriageState::new(),
            &DraftState::new(),
            &verdicts,
            &anchor(),
        );
        assert_eq!(
            cov.verified, 1,
            "only A's fresh holds counts; B's drifted out"
        );
        assert_eq!(
            cov.stale, 1,
            "B's drifted verdict is tallied as re-verify work"
        );

        let rows = backlog(
            &items,
            &TriageState::new(),
            &DraftState::new(),
            &verdicts,
            &anchor(),
        );
        let a = rows[0].verdict.as_ref().expect("A has a stored verdict");
        assert!(a.fresh && a.status == "holds");
        let b = rows[1].verdict.as_ref().expect("B has a stored verdict");
        assert!(!b.fresh, "B's prose moved");
        assert!(b.stale_reasons.iter().any(|r| r.contains("prose moved")));
    }

    // Verifies: REQ045 — a verdict verified against an admitted candidate drops out of `verified`
    // (into `stale`) once that candidate is edited, even though prose/code/tool never moved. Drives
    // the whole seam: coverage reads the item's live admitted fingerprint from the draft state.
    #[test]
    fn editing_the_admitted_candidate_drifts_its_verdict() {
        let it = item("A");
        let drafted = draft::set_candidate(
            &DraftState::new(),
            &it,
            "requirement r { category: 1 }",
            draft::GateStatus::Ungated,
        );
        let admitted = draft::admit(&drafted, "A", draft::ReviewTier::Optional, "gg", 1);
        let fp = draft::formal_fingerprint(&admitted.drafts["A"]).unwrap();

        // A holds verdict pinned to that exact formalization; every other axis is current.
        let mut v = holds_verdict("A", "A");
        v.provenance.formalization = Some(fp);
        let verdicts = store(vec![v]);

        // While the same candidate is admitted, the verdict is fresh and counts as verified.
        let cov = coverage(
            std::slice::from_ref(&it),
            &TriageState::new(),
            &admitted,
            &verdicts,
            &anchor(),
        );
        assert_eq!(cov.verified, 1);
        assert_eq!(cov.stale, 0);

        // Editing the candidate re-baselines the draft to pending — no live admitted formalization —
        // so the verdict is now about a formalization that is gone: it drifts.
        let edited = draft::set_candidate(
            &admitted,
            &it,
            "requirement r { category: 2 }",
            draft::GateStatus::Ungated,
        );
        let cov = coverage(
            std::slice::from_ref(&it),
            &TriageState::new(),
            &edited,
            &verdicts,
            &anchor(),
        );
        assert_eq!(
            cov.verified, 0,
            "the edited item is no longer known to hold"
        );
        assert_eq!(cov.stale, 1, "its stale verdict is now re-verify work");

        let rows = backlog(&[it], &TriageState::new(), &edited, &verdicts, &anchor());
        let row = rows[0].verdict.as_ref().unwrap();
        assert!(!row.fresh);
        assert!(row
            .stale_reasons
            .iter()
            .any(|r| r.contains("no longer admitted")));
    }

    // Verifies: REQ043 — a drifted verdict is stale regardless of polarity: a fresh `fails` is a
    // known result (neither verified nor stale), but once its prose moves it becomes re-verify work.
    #[test]
    fn stale_counts_drifted_verdicts_of_any_polarity() {
        let items = [item("A")];
        let mut fails = holds_verdict("A", "A");
        fails.status = "fails".into();

        // Fresh `fails`: a known refutation — neither verified nor stale.
        let fresh = store(vec![fails.clone()]);
        let cov = coverage(
            &items,
            &TriageState::new(),
            &DraftState::new(),
            &fresh,
            &anchor(),
        );
        assert_eq!(cov.verified, 0);
        assert_eq!(
            cov.stale, 0,
            "a fresh fails is a current answer, not re-verify work"
        );

        // Same `fails` pinned to an old revision → prose moved → stale.
        fails.provenance.requirement_revision = "old-rev".into();
        let drifted = store(vec![fails]);
        let cov = coverage(
            &items,
            &TriageState::new(),
            &DraftState::new(),
            &drifted,
            &anchor(),
        );
        assert_eq!(
            cov.stale, 1,
            "a drifted fails owes a re-run just like a drifted holds"
        );
    }

    // Verifies: #179 — the worklist names the drifted items and their reasons, agrees with the
    // funnel's count, and leaves out items that are fresh or never verified. Measured gap: `status`
    // said `stale 1` and told the operator to run `provreq verify <ID>` without ever naming `<ID>`.
    #[test]
    fn the_stale_worklist_names_the_drifted_items_and_why() {
        let items = [item("A"), item("B"), item("C")];
        let mut drifted = holds_verdict("B", "B");
        drifted.provenance.requirement_revision = "old-rev".into();
        // A: fresh holds. B: drifted. C: never verified.
        let verdicts = store(vec![holds_verdict("A", "A"), drifted]);

        let work = stale_worklist(
            &items,
            &TriageState::new(),
            &DraftState::new(),
            &verdicts,
            &anchor(),
        );
        assert_eq!(
            work.iter().map(|s| s.id.as_str()).collect::<Vec<_>>(),
            ["B"],
            "a fresh verdict is a current answer and an unverified item owes nothing"
        );
        let view = work[0].verdict.as_ref().expect("the drifted verdict");
        assert_eq!(view.status, "holds", "the worklist shows what drifted");
        assert!(
            view.stale_reasons.iter().any(|r| r.contains("prose moved")),
            "naming the item without the axis just moves the hunt: {:?}",
            view.stale_reasons
        );

        let cov = coverage(
            &items,
            &TriageState::new(),
            &DraftState::new(),
            &verdicts,
            &anchor(),
        );
        assert_eq!(
            cov.stale,
            work.len(),
            "the count and the list are one fact at two resolutions"
        );
    }

    // Verifies: REQ034 — the per-item backlog pairs each item, in order, with its triage
    // classification (None when untriaged) and its formalization state.
    #[test]
    fn backlog_pairs_each_item_with_its_triage_and_formalization() {
        let items = [item("A"), item("B")];
        let triage = set(
            &TriageState::new(),
            &items[0],
            Classification::FormalizableNow,
        );
        let drafts = draft::open(&DraftState::new(), &items[0]);

        let rows = backlog(&items, &triage, &drafts, &VerdictStore::new(), &anchor());
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].id, "A");
        assert_eq!(
            rows[0].classification,
            Some(Classification::FormalizableNow)
        );
        assert_eq!(rows[0].formalization, Formalization::Drafting);
        // B is untriaged and undrafted — both honest "none" states.
        assert_eq!(rows[1].id, "B");
        assert_eq!(rows[1].classification, None);
        assert_eq!(rows[1].formalization, Formalization::None);
    }
}
