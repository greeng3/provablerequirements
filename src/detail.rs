//! Item detail: the read-only formalization view behind `GET /api/requirements/:id` (REQ035).
//!
//! One item drilled into — its prose plus whatever formalization state is persisted for it: the
//! candidate PRL, the stored mechanical-gate outcome, the D12 read-back CNL (re-rendered from the
//! candidate, the same deterministic surface `draft --readback` prints), and the stored grounding
//! bindings. Pure over an item + its draft, so it is testable without a server.
//!
//! It surfaces *persisted* state only: it runs no engine, and it shows bindings as stored rather
//! than resolving them against the subject's live source — grounding validation (D13 resolve/park)
//! is its own later surface. That keeps a detail read cheap and side-effect-free.
//!
//! Implements: REQ035 (read-only item detail: prose, PRL, gate, read-back, bindings)

use crate::draft::{self, Admission, Draft, GateStatus, ReviewTier};
use crate::grounding::{self, Binding, Grounding};
use crate::prl::{self, Requirement};
use crate::rust_adapter::{Resolution, TypeResolution};
use crate::source::{Classification, Item};
use crate::status::Formalization;
use crate::tla_adapter::ModelResolution;
use std::collections::BTreeMap;

/// The review provenance of an admitted formalization (D12). `None` in [`Detail`] when the draft
/// is not admitted, so the UI never invents a reviewer for an in-progress draft.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct AdmissionInfo {
    pub review: ReviewTier,
    pub by: String,
}

/// One item's full read-only formalization detail.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct Detail {
    pub id: String,
    pub title: Option<String>,
    pub text: String,
    pub revision: String,
    /// The item's prose moved since the draft was last touched (R-draft-2) — the same staleness
    /// `verify` flags. `false` when there is no draft to be stale against.
    pub stale: bool,
    pub classification: Option<Classification>,
    pub formalization: Formalization,
    pub admission: Option<AdmissionInfo>,
    /// The candidate PRL, hand-authored or LLM-proposed; `None` until one is written.
    pub candidate: Option<String>,
    /// The last stored mechanical-gate outcome; `None` when the item has no draft at all.
    pub gate: Option<GateStatus>,
    /// The deterministic CNL read-back of the candidate's meaning (D12), rendered only when the
    /// candidate currently gates; `None` otherwise (no candidate, or it no longer parses).
    pub readback: Option<String>,
    pub bindings: Vec<Binding>,
    /// The live D13 grounding dry-run — each binding resolved against the subject's real source.
    /// `None` unless the candidate gates and has bindings (nothing to resolve otherwise). The
    /// server fills this; [`build`] leaves it `None` so it stays pure and filesystem-free.
    pub grounding: Option<GroundingReport>,
}

/// One binding resolved against the subject's live source (D13): whether it resolved and the
/// adapter's own read-back of *what* it resolved to — the "is that what you meant?" surface.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct BindingResolution {
    pub symbol: String,
    pub observable: String,
    pub category: String,
    pub resolved: bool,
    pub summary: String,
}

/// The live grounding dry-run for a candidate: whether the requirement grounds, and the
/// per-binding resolution behind that verdict.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct GroundingReport {
    pub grounded: bool,
    pub bindings: Vec<BindingResolution>,
}

/// Map already-computed resolutions into a [`GroundingReport`]. Pure over the resolution maps —
/// the caller runs the adapters via [`grounding::resolve_bindings`] — so it is testable without a
/// filesystem, mirroring [`grounding::verdict`]. Each binding is answered by its own resolver
/// (sort → type, predicate → function, model → TLA+), in the same order the CLI dry-run uses.
///
/// Implements: REQ036 (live D13 grounding dry-run in the item detail surface)
pub fn grounding_report(
    requirement: &Requirement,
    bindings: &[Binding],
    by_symbol: &BTreeMap<String, Resolution>,
    by_sort: &BTreeMap<String, TypeResolution>,
    by_model: &BTreeMap<String, ModelResolution>,
) -> GroundingReport {
    let grounded = matches!(
        grounding::verdict(requirement, bindings, by_symbol, by_sort, by_model),
        Grounding::Grounded
    );
    let resolutions = bindings
        .iter()
        .map(|b| {
            let (resolved, summary) = if let Some(r) = by_sort.get(&b.symbol) {
                (r.is_resolved(), r.describe(&b.symbol, &b.observable))
            } else if let Some(r) = by_symbol.get(&b.symbol) {
                (r.is_resolved(), r.describe(&b.symbol, &b.observable))
            } else if let Some(r) = by_model.get(&b.symbol) {
                (r.is_resolved(), r.describe(&b.symbol, &b.observable))
            } else {
                (
                    false,
                    format!(
                        "{} → `{}` (category {}): dry-run deferred — engine not wired yet",
                        b.symbol,
                        b.observable,
                        b.category.as_label()
                    ),
                )
            };
            BindingResolution {
                symbol: b.symbol.clone(),
                observable: b.observable.clone(),
                category: b.category.as_label().to_string(),
                resolved,
                summary,
            }
        })
        .collect();
    GroundingReport {
        grounded,
        bindings: resolutions,
    }
}

/// Assemble one item's detail from its persisted draft (if any) and its triage classification.
pub fn build(item: &Item, classification: Option<Classification>, draft: Option<&Draft>) -> Detail {
    let stale = draft.map(|d| draft::is_stale(d, item)).unwrap_or(false);
    let formalization = match draft {
        Some(d) if d.is_admitted() => Formalization::Admitted,
        Some(_) => Formalization::Drafting,
        None => Formalization::None,
    };
    let admission = match draft.map(|d| &d.admission) {
        Some(Admission::Admitted { review, by, .. }) => Some(AdmissionInfo {
            review: *review,
            by: by.clone(),
        }),
        _ => None,
    };
    let candidate = draft.and_then(|d| d.candidate.clone());
    // Re-gate the candidate to render the read-back — the read-back needs the parsed AST, and a
    // deterministic re-render is the honest D12 surface (independent of the forward LLM).
    let readback = candidate
        .as_deref()
        .and_then(|c| prl::gate(c).ok())
        .map(|outcome| prl::render(&outcome.requirement));

    Detail {
        id: item.id.clone(),
        title: item.title.clone(),
        text: item.text.clone(),
        revision: item.revision.clone(),
        stale,
        classification,
        formalization,
        admission,
        candidate,
        gate: draft.map(|d| d.gate.clone()),
        readback,
        bindings: draft.map(|d| d.bindings.clone()).unwrap_or_default(),
        grounding: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::draft::DraftState;

    fn item(id: &str) -> Item {
        Item {
            id: id.into(),
            text: "A logged-in user always has a session.".into(),
            revision: "r1".into(),
            title: Some("Login invariant".into()),
            verification_hint: None,
        }
    }

    const CANDIDATE: &str = "requirement r { category: 1 \
        vocabulary { state logged_in state has_session } \
        require { always (not logged_in or has_session) } }";

    // Verifies: REQ035 — an item with no draft is honest: no candidate, no gate, no read-back,
    // and an unformalized state — never a fabricated one.
    #[test]
    fn item_without_a_draft_has_no_formalization() {
        let d = build(&item("REQ001"), None, None);
        assert_eq!(d.formalization, Formalization::None);
        assert!(d.candidate.is_none());
        assert!(d.gate.is_none());
        assert!(d.readback.is_none());
        assert!(d.admission.is_none());
        assert!(!d.stale);
    }

    // Verifies: REQ035 — a drafted candidate surfaces its PRL and a rendered read-back CNL; an
    // admitted draft reports its review provenance.
    #[test]
    fn admitted_draft_surfaces_candidate_readback_and_review() {
        let it = item("REQ001");
        let drafts = draft::set_candidate(
            &draft::open(&DraftState::new(), &it),
            &it,
            CANDIDATE,
            GateStatus::Passed { warnings: vec![] },
        );
        let admitted = draft::admit(&drafts, "REQ001", ReviewTier::Mandatory, "gg", 1);

        let d = build(
            &it,
            Some(Classification::FormalizableNow),
            admitted.drafts.get("REQ001"),
        );
        assert_eq!(d.formalization, Formalization::Admitted);
        assert_eq!(d.candidate.as_deref(), Some(CANDIDATE));
        assert!(matches!(d.gate, Some(GateStatus::Passed { .. })));
        // The read-back is the deterministic CNL of the claim, not the raw PRL.
        let readback = d.readback.expect("a gated candidate renders a read-back");
        assert!(!readback.is_empty());
        assert_eq!(d.admission.map(|a| a.by), Some("gg".to_string()));
    }

    // Verifies: REQ036 — the grounding report resolves each binding and parks the whole when any
    // one does not resolve, carrying an honest per-binding read-back either way.
    #[test]
    fn grounding_report_reports_per_binding_and_parks_on_any_unresolved() {
        use crate::grounding::{BindCategory, Fidelity};
        use crate::rust_adapter::{CodeMatch, ParamMode};

        let requirement = prl::gate(CANDIDATE).unwrap().requirement;
        let bindings = vec![
            Binding {
                symbol: "logged_in".into(),
                category: BindCategory::Code,
                observable: "login".into(),
                fidelity: Fidelity::Definitional,
            },
            Binding {
                symbol: "has_session".into(),
                category: BindCategory::Code,
                observable: "has_session".into(),
                fidelity: Fidelity::Definitional,
            },
        ];
        let mut by_symbol = BTreeMap::new();
        by_symbol.insert(
            "logged_in".to_string(),
            Resolution::Resolved {
                at: CodeMatch {
                    file: "src/lib.rs".into(),
                    line: 1,
                    text: "fn login() -> bool { true }".into(),
                },
                params: vec![ParamMode::ByValue; 0],
            },
        );
        by_symbol.insert("has_session".to_string(), Resolution::NotFound);

        let report = grounding_report(
            &requirement,
            &bindings,
            &by_symbol,
            &BTreeMap::new(),
            &BTreeMap::new(),
        );
        assert!(!report.grounded, "an unresolved binding parks the whole");
        assert_eq!(report.bindings.len(), 2);
        assert!(report.bindings[0].resolved);
        assert!(!report.bindings[1].resolved);
    }
}
