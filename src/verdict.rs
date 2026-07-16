//! Step 4 — the verdict object (D7 three-valued evidence record + D9 provenance). First
//! slice: the honest epistemic record, **no engine execution yet**.
//!
//! A verdict is never a judgment — `status ∈ {holds, fails, unknown}` and an `unknown`
//! always carries a reason (D10). Because no verification engine runs in this slice, every
//! verdict produced is honestly `unknown`: either **missing-grounding** (the requirement
//! is not grounded, so no engine could run — R-ground-1) or **no-engine** (grounded, but
//! no engine has executed the property). A sound `holds` needs a prover to actually check
//! the temporal property against the code; grounding only confirms the binding *resolves*.
//! Real `holds`/`fails` — with a strength/basis scale and per-tool evidence tree — arrive
//! when an engine is wired.
//!
//! Implements: REQ023 (verdict object + provenance; honest unknown, no engine yet).

use crate::grounding::Grounding;

/// The three-valued verdict polarity (D7). No engine runs in this slice, so only
/// [`Status::Unknown`] is ever produced here; `Holds`/`Fails` exist for the engine slice.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    Holds,
    Fails,
    Unknown,
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

/// Why a verdict is `unknown` (D10 taxonomy, restricted to what this slice can produce).
/// The richer reasons — inconclusive, inapplicable, divergence-needs-review,
/// assumption-unmet — arrive with real engines.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnknownReason {
    /// The requirement is not grounded — no engine could run (R-ground-1). Never faked
    /// into a verdict, honestly recorded as "not grounded".
    MissingGrounding,
    /// Grounded, but no verification engine has executed the property yet (this slice).
    NoEngine,
}

impl UnknownReason {
    pub fn as_str(&self) -> &'static str {
        match self {
            UnknownReason::MissingGrounding => "missing-grounding",
            UnknownReason::NoEngine => "no-engine",
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

/// A verdict for one requirement (D7). This slice carries status + reason + provenance;
/// the strength/basis scale and per-category evidence tree are added with real engines.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Verdict {
    pub id: String,
    pub status: Status,
    /// Present exactly when `status == Unknown` (an unknown always carries a reason).
    pub reason: Option<UnknownReason>,
    /// Human-readable detail — e.g. the grounding parked-reasons behind a
    /// missing-grounding unknown.
    pub detail: Vec<String>,
    pub provenance: Provenance,
}

/// Produce the honest verdict for a requirement from its live grounding result and pinned
/// provenance. Pure — the caller runs the grounding dry-run and gathers provenance, so this
/// stays testable. No engine runs, so the result is always `unknown` with a reason.
pub fn from_grounding(id: &str, grounding: &Grounding, provenance: Provenance) -> Verdict {
    let (reason, detail) = match grounding {
        Grounding::Parked { reasons } => (UnknownReason::MissingGrounding, reasons.clone()),
        Grounding::Grounded => (UnknownReason::NoEngine, Vec::new()),
    };
    Verdict {
        id: id.to_string(),
        status: Status::Unknown,
        reason: Some(reason),
        detail,
        provenance,
    }
}

/// Render a verdict as a human read-back (D1 round-trip). Deterministic, no LLM.
pub fn render(v: &Verdict) -> String {
    let mut out = format!("{}: {}", v.id, v.status.as_str());
    if let Some(reason) = v.reason {
        out.push_str(&format!(" ({})", reason.as_str()));
        match reason {
            UnknownReason::MissingGrounding => out.push_str(
                " — the requirement is not grounded; no engine can run until every symbol \
                 binds to a confirmed observable",
            ),
            UnknownReason::NoEngine => out
                .push_str(" — grounded, but no verification engine has executed this property yet"),
        }
    }
    for d in &v.detail {
        out.push_str(&format!("\n    - {d}"));
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
