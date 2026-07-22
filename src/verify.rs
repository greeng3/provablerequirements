//! Step 4 — verify one admitted requirement on demand: re-gate, re-run the live grounding
//! dry-run, and — only when grounded — run the engine ensemble, aggregating each engine's
//! [`crate::verdict::Evidence`] into one honest [`crate::verdict::Verdict`] (D2b).
//!
//! This is the one flow behind both `provreq verify` (the CLI, which prints it) and
//! `POST /api/requirements/:id/verify` (the server, which serializes it), so the two never
//! diverge on what a requirement's verdict is. The `--draft-contracts` staging side-effect is
//! *not* here: it writes the subject's working tree and belongs to the CLI alone; this flow
//! runs no git and stages no source.
//!
//! Implements: REQ027/REQ029/REQ030/REQ031/REQ032 (the ensemble run), REQ038 (verify-on-demand
//! as a shared flow the web surface calls).

use crate::adopt::resolve;
use crate::draft;
use crate::grounding::{self, Binding, Grounding};
use crate::prl::Requirement;
use crate::rust_adapter::Resolution;
use crate::verdict::{self, Provenance, Verdict};
use anyhow::Result;
use std::collections::BTreeMap;
use std::path::Path;

/// The result of asking to verify one requirement. Every non-verdict variant is an honest
/// "why there is no verdict yet" the operator can act on — never an error, never a fabricated
/// pass. Only [`VerifyOutcome::Verdict`] carries a real answer.
///
/// An *unknown id* is not a variant here: [`verify`] returns `Ok(None)` for it, so the server
/// can answer 404 (distinct from a 409 unadopted subject), mirroring the detail read.
#[derive(Debug)]
pub enum VerifyOutcome {
    /// The item has no draft at all — it has not been formalized. Verify has nothing to run.
    NoDraft,
    /// A draft exists but has not been admitted; an unreviewed formalization is not verified.
    NotAdmitted,
    /// The draft is admitted but carries no candidate PRL to check.
    NoCandidate,
    /// The admitted candidate no longer passes the mechanical gate — it must be re-checked
    /// before any verdict can be trusted. Carries the gate errors.
    GateFailed { errors: Vec<String> },
    /// A real verdict (D7). `stale` flags that the requirement prose moved since admission
    /// (re-admit before trusting it). `grounded` and `resolutions` are the grounding context
    /// the run already computed — the CLI reuses them to stage `--draft-contracts` without a
    /// second resolution pass; the web surface ignores them and serializes the verdict.
    Verdict {
        verdict: Verdict,
        stale: bool,
        grounded: bool,
        resolutions: BTreeMap<String, Resolution>,
    },
}

/// Verify one requirement in `subject` by id. `Ok(None)` when the id is not in the subject (a
/// 404 for the server); `Err` when the subject is not adopted or the companion cannot be read
/// (a 409). Runs the engine ensemble when the requirement is grounded, else records an honest
/// unknown — it never fabricates a pass and never mutates the subject.
pub fn verify(subject: &Path, id: &str) -> Result<Option<VerifyOutcome>> {
    let (companion, items) = resolve(subject)?;
    let state = draft::load(&companion)?;
    let Some(item) = items.iter().find(|i| i.id == id) else {
        return Ok(None);
    };
    let Some(draft) = state.drafts.get(id) else {
        return Ok(Some(VerifyOutcome::NoDraft));
    };
    if !draft.is_admitted() {
        return Ok(Some(VerifyOutcome::NotAdmitted));
    }
    let Some(candidate) = &draft.candidate else {
        return Ok(Some(VerifyOutcome::NoCandidate));
    };
    let requirement = match crate::prl::gate(candidate) {
        Ok(outcome) => outcome.requirement,
        Err(errors) => {
            let errors = errors.iter().map(|e| e.to_string()).collect();
            return Ok(Some(VerifyOutcome::GateFailed { errors }));
        }
    };

    // Live grounding dry-run against every wired observable world (code + model) → verdict.
    let (by_symbol, by_sort, by_model) =
        grounding::resolve_bindings(subject, &companion, &requirement, &draft.bindings);
    let grounding_result = grounding::verdict(
        &requirement,
        &draft.bindings,
        &by_symbol,
        &by_sort,
        &by_model,
    );

    let provenance = Provenance {
        requirement_revision: draft.revision.clone(),
        subject_commit: subject_head_commit(subject),
        tool_version: env!("CARGO_PKG_VERSION").to_string(),
    };
    // Only a GROUNDED requirement reaches an engine: an unresolved binding means there is
    // nothing to check the claim through, and running an engine against it would answer a
    // question nobody asked (R-ground-1).
    let grounded = matches!(grounding_result, Grounding::Grounded);
    let verdict = if grounded {
        run_ensemble(
            subject,
            &companion,
            id,
            &requirement,
            &draft.bindings,
            &by_symbol,
            provenance,
        )
    } else {
        verdict::from_grounding(id, &grounding_result, provenance)
    };
    // Living loop (REQ039): the verdict becomes durable state. Persist it keyed by id — the latest
    // answer replaces any earlier one — so the backlog/detail can show it and later detect when it
    // has drifted, without re-running an engine.
    let store = crate::verdict_store::load(&companion)?;
    let recorded = crate::verdict_store::record(&store, verdict::report(&verdict));
    crate::verdict_store::save(&companion, &recorded)?;

    Ok(Some(VerifyOutcome::Verdict {
        verdict,
        stale: draft::is_stale(draft, item),
        grounded,
        resolutions: by_symbol,
    }))
}

/// Run the engine ensemble for a grounded requirement and aggregate the evidence (D2b).
///
/// Dispatch is by engine name, not category, because category 1 is an **ensemble**: Kani AND
/// Creusot AND Prusti all run and their evidence is aggregated. Category 2a routes to TLC. Each
/// engine has its own lowering, and none silently inherits another's. 2b/3 have no wired engine,
/// so they never reach a branch here — they are `no_engine` at the gate below.
pub fn run_ensemble(
    subject: &Path,
    companion: &Path,
    id: &str,
    requirement: &Requirement,
    bindings: &[Binding],
    resolutions: &BTreeMap<String, Resolution>,
    provenance: Provenance,
) -> Verdict {
    let category = grounding::default_category(requirement);
    let engines = crate::engine::engines_for(category);

    // The ensemble runs every engine that is ready; the others are reported but do not block,
    // as long as one can answer (D2b). No engine ready means nothing checked the property — an
    // honest no-engine that names who must act (wiring is ours, installing is the operator's).
    let ready: Vec<&crate::engine::Engine> = engines
        .iter()
        .filter(|e| crate::engine::detect(e).is_ready())
        .collect();
    if ready.is_empty() {
        let detail = engines
            .iter()
            .map(|e| {
                format!(
                    "category {} routes to {} — {}",
                    category.as_label(),
                    e.name,
                    crate::engine::detect(e).describe()
                )
            })
            .collect();
        return verdict::no_engine(id, detail, provenance);
    }

    let evidence = ready
        .iter()
        .map(|e| match e.name {
            "Kani" => kani_evidence(subject, id, requirement, bindings, resolutions),
            "Creusot" => creusot_evidence(subject, id, requirement, bindings, resolutions),
            "Prusti" => prusti_evidence(subject, id, requirement, bindings, resolutions),
            "TLC (TLA+)" => tlc_evidence(subject, companion, id, requirement, bindings),
            // A ready engine with no lowering wired here is a gap in provreq, recorded as
            // inconclusive rather than silently skipped.
            other => verdict::Evidence::inconclusive(
                other,
                vec![format!(
                    "{other} probed as ready but has no lowering wired in provreq"
                )],
            ),
        })
        .collect();
    verdict::aggregate(id, evidence, provenance)
}

/// Category 1 → Kani (REQ027): lower to an additive proof harness, run it, map to evidence.
/// A subject that is not a cargo crate or a claim that cannot be faithfully lowered is honest
/// `inconclusive` evidence, never approximated (D2).
fn kani_evidence(
    subject: &Path,
    id: &str,
    requirement: &Requirement,
    bindings: &[Binding],
    resolutions: &BTreeMap<String, Resolution>,
) -> verdict::Evidence {
    let Some(crate_name) = crate::kani::subject_crate_name(subject) else {
        return verdict::Evidence::inconclusive(
            "Kani",
            vec![
                "the subject is not a cargo crate (`cargo metadata` found no package), so a \
                 Kani harness has nothing to import"
                    .to_string(),
            ],
        );
    };
    let harness = match crate::kani::lower(
        requirement,
        &crate_name,
        bindings,
        resolutions,
        &crate::kani::harness_name(id),
    ) {
        Ok(h) => h,
        Err(e) => return verdict::Evidence::inconclusive("Kani", vec![e.reason]),
    };
    crate::kani::run(subject, &harness).into_evidence()
}

/// Category 1 → Creusot (REQ031): the ensemble's deductive member. Lower to an additive in-crate
/// proof harness, run it, map to evidence. A claim that cannot be faithfully lowered — or a
/// subject with no crate root — is honest `inconclusive`, never approximated (D2).
fn creusot_evidence(
    subject: &Path,
    id: &str,
    requirement: &Requirement,
    bindings: &[Binding],
    resolutions: &BTreeMap<String, Resolution>,
) -> verdict::Evidence {
    let harness = match crate::creusot::lower(
        requirement,
        bindings,
        resolutions,
        &crate::creusot::harness_name(id),
    ) {
        Ok(h) => h,
        Err(e) => return verdict::Evidence::inconclusive("Creusot", vec![e.reason]),
    };
    crate::creusot::run(subject, &harness).into_evidence()
}

/// Category 1 → Prusti (REQ032): the ensemble's second deductive member. Lower to an additive
/// in-crate proof harness, run it, map to evidence. A subject without `prusti-contracts` is honest
/// `inconclusive`, never approximated (D2).
fn prusti_evidence(
    subject: &Path,
    id: &str,
    requirement: &Requirement,
    bindings: &[Binding],
    resolutions: &BTreeMap<String, Resolution>,
) -> verdict::Evidence {
    let harness = match crate::prusti::lower(
        requirement,
        bindings,
        resolutions,
        &crate::prusti::harness_name(id),
    ) {
        Ok(h) => h,
        Err(e) => return verdict::Evidence::inconclusive("Prusti", vec![e.reason]),
    };
    crate::prusti::run(subject, &harness).into_evidence()
}

/// Category 2a → TLC (REQ029): locate the subject's `Spec`, lower to an additive TLA+ module with
/// a temporal property, run TLC beside the spec, map to evidence. A missing `Spec` or an
/// un-lowerable claim is honestly `inconclusive`, never approximated (D2).
fn tlc_evidence(
    subject: &Path,
    companion: &Path,
    id: &str,
    requirement: &Requirement,
    bindings: &[Binding],
) -> verdict::Evidence {
    let site = match crate::tlc::locate_spec(subject, companion) {
        Ok(site) => site,
        Err(reason) => return verdict::Evidence::inconclusive("TLC (TLA+)", vec![reason]),
    };
    let check = match crate::tlc::lower(
        requirement,
        &site.module,
        bindings,
        &crate::tlc::module_name(id),
    ) {
        Ok(c) => c,
        Err(e) => return verdict::Evidence::inconclusive("TLC (TLA+)", vec![e.reason]),
    };
    crate::tlc::run(&site, &check).into_evidence()
}

/// Best-effort subject git HEAD for verdict provenance (D9). `None` when the subject is not a git
/// repo — never fabricated. Public so the browse surfaces can build the same [`DriftAnchor`] the
/// verdict was pinned against ([`crate::verdict_store`]).
pub fn subject_head_commit(subject: &Path) -> Option<String> {
    let output = std::process::Command::new("git")
        .arg("-C")
        .arg(subject)
        .args(["rev-parse", "HEAD"])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let commit = String::from_utf8_lossy(&output.stdout).trim().to_string();
    (!commit.is_empty()).then_some(commit)
}

#[cfg(test)]
mod tests {
    use super::*;

    // Verifies: REQ038 — verifying an id the subject does not contain is `Ok(None)` (a 404 for
    // the server), distinct from an unadopted-subject error.
    #[test]
    fn unknown_id_is_none() {
        let subject = adopted_subject_with_one_item();
        let out = verify(subject.path(), "REQ999").unwrap();
        assert!(out.is_none());
    }

    // Verifies: REQ038 — an item with no draft is an honest NoDraft, never an error and never a
    // fabricated verdict.
    #[test]
    fn undrafted_item_is_no_draft() {
        let subject = adopted_subject_with_one_item();
        let out = verify(subject.path(), "REQ001").unwrap().unwrap();
        assert!(matches!(out, VerifyOutcome::NoDraft), "got {out:?}");
    }

    // Verifies: REQ038 — an unadopted subject is an error (the server maps it to 409), not None.
    #[test]
    fn unadopted_subject_is_error() {
        let empty = tempfile::tempdir().unwrap();
        assert!(verify(empty.path(), "REQ001").is_err());
    }

    /// A minimal adopted subject with one item and no drafts — mirrors the server test fixture.
    fn adopted_subject_with_one_item() -> tempfile::TempDir {
        use std::fs;
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        fs::write(
            root.join(".doorstop.yml"),
            "settings:\n  prefix: REQ\n  digits: 3\n",
        )
        .unwrap();
        fs::write(
            root.join("REQ001.yml"),
            "active: true\nlevel: 1.0\nnormative: true\nref: ''\nreviewed: null\ntext: |\n  A requirement.\n",
        )
        .unwrap();
        fs::write(
            root.join(crate::adopt::MANIFEST_FILE),
            "subject_requirements_root: .\n",
        )
        .unwrap();
        dir
    }
}
