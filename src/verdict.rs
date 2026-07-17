//! Step 4 — the verdict object (D7 three-valued evidence record + D9 provenance).
//!
//! A verdict is never a judgment — `status ∈ {holds, fails, unknown}` and an `unknown`
//! always carries a reason (D10). An ungrounded requirement is `unknown / missing-grounding`
//! (R-ground-1); a grounded one with no engine for its category is `unknown / no-engine`.
//!
//! **Engine-independent by design.** REQ027 wired Kani as category-1 engine #1, but D2's
//! rule is one core meaning lowered to each engine — so this module knows about *bases* and
//! *witnesses*, never about Kani. Each engine maps its own result into these constructors
//! ([`crate::kani::Outcome::into_verdict`]), which is what lets D2b's ensemble add engines
//! with differing soundness directions without touching the core.
//!
//! Implements: REQ023 (verdict object + provenance; honest unknown), REQ027 (a real
//! `holds`/`fails` from a wired engine, with a D8 basis and a D9 witness).

use crate::grounding::Grounding;

/// The three-valued verdict polarity (D7).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    Holds,
    Fails,
    Unknown,
}

/// D8 basis for a `holds` — *how* the polarity was established, which is a different
/// question from the polarity itself. The design's scale, strongest first: `proven`
/// (deductive, ∀ executions) › `model-checked` (∀ over a model M; note *bounded?*) ›
/// `not-falsified` (empirical: N runs / duration / coverage).
///
/// Only the rung an engine can actually earn is representable. Kani is a **bounded** model
/// checker: it establishes the claim over the states it explored, not over all executions,
/// so it yields [`Basis::ModelCheckedBounded`] and **cannot** yield `proven`. Making that
/// structural rather than a convention is the point — an engine cannot overclaim by
/// accident.
///
/// `// ponytail: one rung, because one engine. `proven` (Prusti/Creusot/Verus) and
/// `not-falsified` (empirical) arrive with the engines that earn them — adding them now
/// would be scale we cannot back.`
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Basis {
    ModelCheckedBounded,
}

impl Basis {
    pub fn as_str(&self) -> &'static str {
        match self {
            Basis::ModelCheckedBounded => "model-checked (bounded)",
        }
    }
}

impl Status {
    pub fn as_str(&self) -> &'static str {
        match self {
            Status::Holds => "holds",
            Status::Fails => "fails",
            Status::Unknown => "unknown",
        }
    }
}

/// Why a verdict is `unknown` (D10 taxonomy, restricted to what the wired engines can
/// produce). `inapplicable` is deliberately absent: the fragment check (REQ024) rejects an
/// out-of-fragment claim at the gate, so it never reaches a verdict to carry the reason —
/// adding the variant would be dead code. `divergence-needs-review` and `assumption-unmet`
/// arrive with the D2b ensemble and D8 contingencies respectively.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnknownReason {
    /// The requirement is not grounded — no engine could run (R-ground-1). Never faked
    /// into a verdict, honestly recorded as "not grounded".
    MissingGrounding,
    /// Grounded, but no engine is wired for the requirement's category (2a/2b/3 today).
    NoEngine,
    /// An engine was asked but could not decide: the claim could not be lowered, the
    /// harness would not compile, or the run errored. D10's `inconclusive(…)` — the tool
    /// ran and came back without an answer, which is not the same as the answer being no.
    Inconclusive,
}

impl UnknownReason {
    pub fn as_str(&self) -> &'static str {
        match self {
            UnknownReason::MissingGrounding => "missing-grounding",
            UnknownReason::NoEngine => "no-engine",
            UnknownReason::Inconclusive => "inconclusive",
        }
    }
}

/// D9 provenance — what a verdict was produced against, so it is reproducible and its
/// staleness against changed artifacts is detectable. `subject_commit` is `None` when the
/// subject is not a git repo (best-effort, never fabricated).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Provenance {
    pub requirement_revision: String,
    pub subject_commit: Option<String>,
    pub tool_version: String,
}

/// A verdict for one requirement (D7). Splits **polarity** (`status`) from **basis** (how a
/// `holds` was established) and from the **witness** (what makes a `fails` re-checkable).
///
/// `// ponytail: no per-tool evidence map or cross-check yet — one engine cannot disagree
/// with itself. Both land with D2b's ensemble.`
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Verdict {
    pub id: String,
    pub status: Status,
    /// Present exactly when `status == Unknown` (an unknown always carries a reason).
    pub reason: Option<UnknownReason>,
    /// D8 — how a `holds` was established. Present exactly when `status == Holds`; a
    /// polarity without a basis would be a claim with no strength behind it.
    pub basis: Option<Basis>,
    /// D9 — the re-checkable witness behind a `fails` (a counterexample the operator can
    /// replay). `None` when the engine refuted the claim without producing one.
    pub witness: Option<String>,
    /// Human-readable detail — the grounding parked-reasons behind a missing-grounding
    /// unknown, the engine's own message behind an inconclusive, the violated check behind
    /// a fails.
    pub detail: Vec<String>,
    pub provenance: Provenance,
}

/// The verdict for a requirement **no engine ran for** — either because it is not grounded
/// (nothing could run) or because no engine is wired for its category. Pure.
///
/// A grounded requirement whose engine *is* wired does not come through here: it earns a
/// real verdict via the engine's own mapping into [`holds`]/[`fails`]/[`inconclusive`].
pub fn from_grounding(id: &str, grounding: &Grounding, provenance: Provenance) -> Verdict {
    let (reason, detail) = match grounding {
        Grounding::Parked { reasons } => (UnknownReason::MissingGrounding, reasons.clone()),
        Grounding::Grounded => (UnknownReason::NoEngine, Vec::new()),
    };
    unknown(id, reason, detail, provenance)
}

/// A grounded requirement whose category has no engine that can answer it — either nothing
/// is wired for the category, or the wired engine is not installed. `detail` names which,
/// because the two ask different people to act: wiring is ours, installing is the
/// operator's.
pub fn no_engine(id: &str, detail: Vec<String>, provenance: Provenance) -> Verdict {
    unknown(id, UnknownReason::NoEngine, detail, provenance)
}

/// A `holds` established on `basis`. Engine-agnostic: the engine names the rung it earned,
/// and [`Basis`] makes `proven` unrepresentable for a bounded checker.
pub fn holds(id: &str, basis: Basis, provenance: Provenance) -> Verdict {
    Verdict {
        id: id.to_string(),
        status: Status::Holds,
        reason: None,
        basis: Some(basis),
        witness: None,
        detail: Vec::new(),
        provenance,
    }
}

/// A `fails` — definitive when it carries a valid witness (falsification is the robust
/// half, D8).
pub fn fails(
    id: &str,
    witness: Option<String>,
    detail: Vec<String>,
    provenance: Provenance,
) -> Verdict {
    Verdict {
        id: id.to_string(),
        status: Status::Fails,
        reason: None,
        basis: None,
        witness,
        detail,
        provenance,
    }
}

/// An engine ran but could not decide (D10 `inconclusive`). Never a verdict — the whole
/// point is that "the tool came back empty" and "the claim is false" are different answers.
pub fn inconclusive(id: &str, detail: Vec<String>, provenance: Provenance) -> Verdict {
    unknown(id, UnknownReason::Inconclusive, detail, provenance)
}

fn unknown(
    id: &str,
    reason: UnknownReason,
    detail: Vec<String>,
    provenance: Provenance,
) -> Verdict {
    Verdict {
        id: id.to_string(),
        status: Status::Unknown,
        reason: Some(reason),
        basis: None,
        witness: None,
        detail,
        provenance,
    }
}

/// Render a verdict as a human read-back (D1 round-trip). Deterministic, no LLM.
///
/// A `holds` always renders its basis, so a bounded result can never be *read* as a proof
/// even at a glance — the overclaim D8 guards against is a reading error as much as a
/// modelling one.
pub fn render(v: &Verdict) -> String {
    let mut out = format!("{}: {}", v.id, v.status.as_str());
    if let Some(basis) = v.basis {
        out.push_str(&format!(
            " — {}: verified over the states the engine explored, NOT proven for all \
             executions",
            basis.as_str()
        ));
    }
    if let Some(reason) = v.reason {
        out.push_str(&format!(" ({})", reason.as_str()));
        match reason {
            UnknownReason::MissingGrounding => out.push_str(
                " — the requirement is not grounded; no engine can run until every symbol \
                 binds to a confirmed observable",
            ),
            UnknownReason::NoEngine => out
                .push_str(" — grounded, but no verification engine has executed this property yet"),
            UnknownReason::Inconclusive => out
                .push_str(" — an engine ran but could not decide; this is not evidence either way"),
        }
    }
    for d in &v.detail {
        out.push_str(&format!("\n    - {d}"));
    }
    if let Some(w) = &v.witness {
        out.push_str("\n  witness (D9 — replay it against the subject to re-check this):\n");
        for line in w.lines() {
            out.push_str(&format!("    {line}\n"));
        }
        out.pop();
    }
    let commit = v
        .provenance
        .subject_commit
        .as_deref()
        .unwrap_or("(not a git subject)");
    out.push_str(&format!(
        "\n  provenance: requirement@{} subject@{} provreq@{}",
        v.provenance.requirement_revision, commit, v.provenance.tool_version
    ));
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn prov() -> Provenance {
        Provenance {
            requirement_revision: "rev-1".into(),
            subject_commit: Some("abc123".into()),
            tool_version: "0.0.1".into(),
        }
    }

    // Verifies: REQ023 — a grounded requirement with no engine yields unknown/no-engine,
    // never a fabricated holds.
    #[test]
    fn grounded_but_no_engine_is_unknown_no_engine() {
        let v = from_grounding("SR001", &Grounding::Grounded, prov());
        assert_eq!(v.status, Status::Unknown);
        assert_eq!(v.reason, Some(UnknownReason::NoEngine));
        assert!(v.detail.is_empty());
    }

    // Verifies: REQ023 (R-ground-1) — an ungrounded requirement yields
    // unknown/missing-grounding and carries the parked reasons, never a verdict.
    #[test]
    fn parked_grounding_is_unknown_missing_grounding_with_reasons() {
        let parked = Grounding::Parked {
            reasons: vec!["has_session: no code span matches `fn nope`".into()],
        };
        let v = from_grounding("SR002", &parked, prov());
        assert_eq!(v.status, Status::Unknown);
        assert_eq!(v.reason, Some(UnknownReason::MissingGrounding));
        assert_eq!(v.detail.len(), 1);
        assert!(v.detail[0].contains("has_session"));
    }

    // Verifies: REQ023 — the read-back names the status, the reason, the parked detail,
    // and the pinned provenance.
    #[test]
    fn render_shows_status_reason_and_provenance() {
        let parked = Grounding::Parked {
            reasons: vec!["logged_in: unbound".into()],
        };
        let text = render(&from_grounding("SR003", &parked, prov()));
        assert!(text.contains("SR003: unknown (missing-grounding)"));
        assert!(text.contains("logged_in: unbound"));
        assert!(text.contains("requirement@rev-1"));
        assert!(text.contains("subject@abc123"));
        assert!(text.contains("provreq@0.0.1"));
    }

    // Verifies: REQ023 — a subject with no git commit renders honestly, never fabricated.
    #[test]
    fn render_handles_missing_subject_commit() {
        let p = Provenance {
            subject_commit: None,
            ..prov()
        };
        let text = render(&from_grounding("SR004", &Grounding::Grounded, p));
        assert!(text.contains("subject@(not a git subject)"));
    }
}
