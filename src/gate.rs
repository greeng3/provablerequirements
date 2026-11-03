//! `provreq gate` (REQ092) — turn recorded verdicts into an enforcement boundary.
//!
//! `verify` *records* a verdict and moves on; nothing blocks a change that regresses a
//! previously-proven requirement to `unknown`/`refuted`. This is the missing structural piece —
//! Bend 2's whole pitch is that the checker blocks the edit when a law breaks. `gate` re-verifies
//! each requirement the committed baseline recorded as passing and exits non-zero if any no longer
//! does, so a regression fails a commit hook or CI instead of landing silently.
//!
//! The re-verify is the cache-accelerated one (REQ091): an unchanged requirement is served from its
//! stored verdict in milliseconds, so the gate is fast enough to sit in a hook. That dependency is
//! why this could not come first.
//!
//! The comparison is pure and lives here with its tests; the re-verify loop and the baseline read
//! are the impure shell, driven by `run_gate` in the binary.

use crate::verdict::VerdictReport;
use crate::verdict_store::VerdictStore;
use std::path::Path;

/// What a fresh re-verify said about one requirement, reduced to the three facets a regression is
/// judged on. Built from a [`crate::verify::VerifyOutcome`] by [`current_outcome`] — an honest
/// not-yet-verifiable state (no draft, un-admitted, …) collapses to its own non-`holds` status, so a
/// requirement that used to prove and now cannot even produce a verdict reads as the regression it is.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CurrentOutcome {
    pub status: String,
    pub basis: Option<String>,
    pub correspondence: String,
}

/// Why a requirement regressed: its outcome fell from a pass, or (only under `--strict-tier`) it
/// still passes but on weaker evidence than it was recorded with.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RegressionKind {
    /// Was `holds`, now is not — the outcome itself fell.
    Outcome,
    /// Still `holds`, but the trust tier dropped: a weaker basis, or a mechanical result replaced by
    /// an asserted (human-tagged) one.
    Tier,
}

/// One requirement that regressed against its baseline — what the gate blocks on and reports.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Regression {
    pub id: String,
    pub kind: RegressionKind,
    pub old_status: String,
    pub new_status: String,
    pub old_basis: Option<String>,
    pub new_basis: Option<String>,
}

/// The trust rungs ranked strongest-first (REQ091/#498 tier), so a downgrade is a strict drop. An
/// absent or unrecognised basis ranks 0 — the lowest. Under `--strict-tier` this means a
/// `holds→holds` whose *current* basis this binary does not recognise (e.g. a rung added by a newer
/// provreq that wrote the verdict) scores below a known baseline rung and is reported as a downgrade,
/// even if it is in fact stronger. That is the safe direction for an enforcement gate — it
/// over-blocks rather than letting an unreadable tier through — and clears once the gate runs with
/// tooling new enough to rank the basis.
fn basis_rank(basis: Option<&str>) -> u8 {
    match basis {
        Some("proven") => 3,
        Some("model-checked (bounded)") => 2,
        Some("not-falsified") => 1,
        _ => 0,
    }
}

/// Whether `current` is a regression of `baseline`, and why — or `None`.
///
/// Only a baseline that **passed** (`holds`) can regress: a requirement that was `unknown` and stays
/// `unknown` has not fallen, and one that was `unknown` and now `holds` has improved. Against a
/// passing baseline:
///
/// - current is not `holds` → an **outcome** regression (proven → unknown/refuted, or a draft that
///   no longer admits/gates). Always a failure.
/// - current still `holds` but on a weaker tier → a **tier** downgrade, reported only when
///   `strict_tier` is set (the issue's configurable extra): a weaker basis, or a mechanical result
///   that became asserted. A human-tagged test standing in for a lost proof is a real trust loss,
///   even though the outcome is unchanged.
pub fn regression(
    baseline: &VerdictReport,
    current: &CurrentOutcome,
    strict_tier: bool,
) -> Option<Regression> {
    if baseline.status != "holds" {
        return None;
    }
    if current.status != "holds" {
        return Some(Regression {
            id: baseline.id.clone(),
            kind: RegressionKind::Outcome,
            old_status: baseline.status.clone(),
            new_status: current.status.clone(),
            old_basis: baseline.basis.clone(),
            new_basis: current.basis.clone(),
        });
    }
    if strict_tier {
        let basis_dropped =
            basis_rank(current.basis.as_deref()) < basis_rank(baseline.basis.as_deref());
        let correspondence_dropped =
            baseline.correspondence == "mechanical" && current.correspondence == "asserted";
        if basis_dropped || correspondence_dropped {
            return Some(Regression {
                id: baseline.id.clone(),
                kind: RegressionKind::Tier,
                old_status: baseline.status.clone(),
                new_status: current.status.clone(),
                old_basis: baseline.basis.clone(),
                new_basis: current.basis.clone(),
            });
        }
    }
    None
}

/// Reduce a re-verify's [`crate::verify::VerifyOutcome`] to the three facets [`regression`] judges.
/// A real verdict (fresh or cache-served) carries its own labels; every honest not-yet-verifiable
/// state collapses to a non-`holds` status named for what it is, so it reads as the fall it is when
/// the baseline had proved.
pub fn current_outcome(outcome: &crate::verify::VerifyOutcome) -> CurrentOutcome {
    use crate::verify::VerifyOutcome as O;
    let labelled = |status: &str| CurrentOutcome {
        status: status.to_string(),
        basis: None,
        correspondence: String::new(),
    };
    match outcome {
        O::Verdict { verdict, .. } => {
            let report = crate::verdict::report(verdict);
            CurrentOutcome {
                status: report.status,
                basis: report.basis,
                correspondence: report.correspondence,
            }
        }
        O::Cached(report) => CurrentOutcome {
            status: report.status.clone(),
            basis: report.basis.clone(),
            correspondence: report.correspondence.clone(),
        },
        O::NoDraft => labelled("no-draft"),
        O::NotAdmitted => labelled("not-admitted"),
        O::NoCandidate => labelled("no-candidate"),
        O::GateFailed { .. } => labelled("gate-failed"),
    }
}

/// The baseline the gate compares against, and a one-line description of where it came from.
///
/// The committed `verdicts.yml` at `base_ref` (default `HEAD`) is the baseline the issue asks for:
/// in CI the checked-out tree already *is* that, but reading the ref explicitly also lets a
/// pre-commit hook compare against the last commit rather than a working copy a bad local `verify`
/// may already have overwritten. When the ref cannot be read — not a git repo, or the file was never
/// committed — the gate falls back to the recorded store on disk, and says so: a gate with no
/// committed baseline still enforces against what was last recorded, rather than refusing to run.
pub fn load_baseline(
    subject: &Path,
    companion: &Path,
    base_ref: &str,
) -> anyhow::Result<(VerdictStore, String)> {
    if let Some(store) = committed_baseline(subject, companion, base_ref) {
        return Ok((store, format!("committed verdicts at {base_ref}")));
    }
    let store = crate::verdict_store::load(companion)?;
    Ok((
        store,
        "the recorded verdicts on disk (no committed baseline found)".to_string(),
    ))
}

/// The `verdicts.yml` committed at `base_ref`, read via `git show`. `None` — never fabricated — when
/// the subject is not a git repo, the ref or path is unknown there, or the content does not parse.
fn committed_baseline(subject: &Path, companion: &Path, base_ref: &str) -> Option<VerdictStore> {
    let toplevel = {
        let out = std::process::Command::new("git")
            .arg("-C")
            .arg(subject)
            .args(["rev-parse", "--show-toplevel"])
            .output()
            .ok()
            .filter(|o| o.status.success())?;
        std::path::PathBuf::from(String::from_utf8_lossy(&out.stdout).trim().to_string())
    };
    // `git show <ref>:<path>` resolves the path from the repo root, not from `subject`, so the path
    // must be relative to the toplevel the subject lives under.
    let verdicts = companion.join(crate::verdict_store::VERDICT_FILE);
    let rel = verdicts.strip_prefix(&toplevel).ok()?;
    // git object paths are forward-slashed on every platform.
    let spec = format!("{base_ref}:{}", rel.to_string_lossy().replace('\\', "/"));
    let out = std::process::Command::new("git")
        .arg("-C")
        .arg(subject)
        .args(["show", &spec])
        .output()
        .ok()
        .filter(|o| o.status.success())?;
    serde_yaml::from_slice(&out.stdout).ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::verdict::ProvenanceReport;

    fn baseline(status: &str, basis: Option<&str>, correspondence: &str) -> VerdictReport {
        VerdictReport {
            id: "REQ001".into(),
            status: status.into(),
            basis: basis.map(str::to_string),
            reason: None,
            witness: None,
            detail: vec![],
            evidence: vec![],
            correspondence: correspondence.into(),
            provenance: ProvenanceReport {
                requirement_revision: "r1".into(),
                subject_commit: None,
                tool_version: "0.0.1".into(),
                verified_scope: None,
                formalization: None,
                environment: None,
                spec_fingerprint: None,
                trace_fingerprint: None,
                source_fingerprint: None,
                ui_fingerprint: None,
                tagged_source_fingerprint: None,
                scope_fingerprint: None,
            },
        }
    }

    fn current(status: &str, basis: Option<&str>, correspondence: &str) -> CurrentOutcome {
        CurrentOutcome {
            status: status.into(),
            basis: basis.map(str::to_string),
            correspondence: correspondence.into(),
        }
    }

    // Verifies: REQ092 — a proven requirement that falls to `unknown` is an outcome regression, the
    // core thing the gate blocks on.
    #[test]
    fn a_proven_requirement_falling_to_unknown_regresses() {
        let base = baseline("holds", Some("proven"), "mechanical");
        let now = current("unknown", None, "");
        let r = regression(&base, &now, false).expect("a fall from holds is a regression");
        assert_eq!(r.kind, RegressionKind::Outcome);
        assert_eq!(
            (r.old_status.as_str(), r.new_status.as_str()),
            ("holds", "unknown")
        );
    }

    // Verifies: REQ092 — a proven requirement refuted (`fails`) is a regression too; any non-holds
    // outcome against a passing baseline fails the gate.
    #[test]
    fn a_proven_requirement_refuted_regresses() {
        let base = baseline("holds", Some("proven"), "mechanical");
        assert!(regression(&base, &current("fails", None, ""), false).is_some());
    }

    // Verifies: REQ092 — a requirement that was never passing cannot regress: an `unknown` baseline
    // that stays `unknown`, or even becomes `fails`, has not fallen *from a proof*. The gate guards
    // proven ground, it does not demand that everything become proven.
    #[test]
    fn a_non_passing_baseline_never_regresses() {
        let base = baseline("unknown", None, "");
        assert!(regression(&base, &current("unknown", None, ""), false).is_none());
        assert!(regression(&base, &current("fails", None, ""), true).is_none());
    }

    // Verifies: REQ092 — an unchanged pass is not a regression, with or without the strict-tier flag.
    #[test]
    fn an_unchanged_pass_is_clean() {
        let base = baseline("holds", Some("proven"), "mechanical");
        let now = current("holds", Some("proven"), "mechanical");
        assert!(regression(&base, &now, false).is_none());
        assert!(regression(&base, &now, true).is_none());
    }

    // Verifies: REQ092 — a weaker basis (proven → model-checked) still holds, so it is *not* an
    // outcome regression and is silent by default; it is reported only under `--strict-tier`.
    #[test]
    fn a_tier_downgrade_fires_only_under_strict_tier() {
        let base = baseline("holds", Some("proven"), "mechanical");
        let now = current("holds", Some("model-checked (bounded)"), "mechanical");
        assert!(
            regression(&base, &now, false).is_none(),
            "a still-passing requirement is not an outcome regression"
        );
        let r = regression(&base, &now, true).expect("strict-tier catches the downgrade");
        assert_eq!(r.kind, RegressionKind::Tier);
    }

    // Verifies: REQ092 — under strict-tier, a mechanical proof replaced by an asserted (human-tagged)
    // result is a trust downgrade even though both `holds`: a tagged test is not the proof it stood in
    // for.
    #[test]
    fn mechanical_becoming_asserted_is_a_strict_tier_downgrade() {
        let base = baseline("holds", Some("proven"), "mechanical");
        let now = current("holds", Some("proven"), "asserted");
        assert!(regression(&base, &now, false).is_none());
        assert_eq!(
            regression(&base, &now, true).expect("caught").kind,
            RegressionKind::Tier
        );
    }

    // Verifies: REQ092 — a stronger result is never a regression: improving not-falsified to proven
    // passes even under strict-tier.
    #[test]
    fn a_stronger_result_is_not_a_downgrade() {
        let base = baseline("holds", Some("not-falsified"), "mechanical");
        let now = current("holds", Some("proven"), "mechanical");
        assert!(regression(&base, &now, true).is_none());
    }

    // Verifies: REQ092 — a baseline that proved but now cannot even produce a verdict (the draft was
    // un-admitted) is an outcome regression: `current_outcome` names the state so it reads as a fall.
    #[test]
    fn losing_the_admitted_draft_reads_as_a_fall() {
        let now = current_outcome(&crate::verify::VerifyOutcome::NotAdmitted);
        assert_eq!(now.status, "not-admitted");
        let base = baseline("holds", Some("proven"), "mechanical");
        assert_eq!(
            regression(&base, &now, false)
                .expect("a lost proof is a regression")
                .kind,
            RegressionKind::Outcome
        );
    }
}
