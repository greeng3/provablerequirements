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
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

/// The crate whose code the category-1 engines verify and whose functions code-category grounding
/// resolves against. Defaults to the subject itself; a subject whose dependency graph Kani cannot
/// codegen whole (an FFI-saturated server) can point cat-1 at a C-free member crate with
/// `verify.crate` in `provreq.yml` while keeping its requirements in the umbrella subject (#484).
///
/// Only the code world moves: model, runtime, and UI resolution stay on the subject, because a
/// leaf crate carries no TLA+ spec, trace, or live deployment — those live with the subject.
/// Returned as a path so an unset key is byte-for-byte the pre-#484 behaviour (the subject itself).
pub fn cat1_code_root(subject: &Path, companion: &Path, id: &str) -> PathBuf {
    match read_verify_crate(companion, id) {
        Some(rel) => subject.join(rel),
        None => subject.to_path_buf(),
    }
}

/// The member crate requirement `id` verifies against, or `None` for the subject itself. A
/// per-requirement `verify.crates` entry wins (#483) so requirements whose logic lives in different
/// member crates each target the right one; otherwise the subject-wide `verify.crate` default
/// (#484) applies. A manifest that is missing, unparseable, or silent on both yields `None` — the
/// same tolerance [`crate::kani::Bounds::load`] and [`crate::trace::run::CargoTestConfig::load`]
/// take, since an unconfigured subject is the norm.
fn read_verify_crate(companion: &Path, id: &str) -> Option<String> {
    #[derive(serde::Deserialize)]
    struct Manifest {
        verify: Option<VerifyBlock>,
    }
    #[derive(serde::Deserialize)]
    struct VerifyBlock {
        #[serde(rename = "crate")]
        krate: Option<String>,
        #[serde(default)]
        crates: BTreeMap<String, String>,
    }
    let text = std::fs::read_to_string(companion.join(crate::adopt::MANIFEST_FILE)).ok()?;
    let manifest = serde_yaml::from_str::<Manifest>(&text).ok()?;
    let verify = manifest.verify?;
    verify
        .crates
        .get(id)
        .cloned()
        .or(verify.krate)
        .filter(|c| !c.trim().is_empty())
}

/// A per-requirement cat-2a model target (#489): the standalone TLA+ module and the spec operator
/// `verify` should check instead of the composed `Spec`. Lets a requirement whose property is
/// decidable over a tractable sub-machine earn a verdict where the composed model explodes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModelTarget {
    /// The `.tla` file, relative to the subject, holding the standalone model.
    pub file: String,
    /// The behaviour-spec operator in that module (e.g. `ScanSpec`), used where provreq would
    /// otherwise use the global `Spec`.
    pub spec: String,
}

/// The cat-2a model target declared for requirement `id` in `verify.models`, or `None` when none
/// is declared (the composed-`Spec` default). Same manifest tolerance as the other `verify.*`
/// readers.
pub fn verify_model(companion: &Path, id: &str) -> Option<ModelTarget> {
    #[derive(serde::Deserialize)]
    struct Manifest {
        verify: Option<VerifyBlock>,
    }
    #[derive(serde::Deserialize)]
    struct VerifyBlock {
        #[serde(default)]
        models: BTreeMap<String, ModelEntry>,
    }
    #[derive(serde::Deserialize)]
    struct ModelEntry {
        file: String,
        spec: String,
    }
    let text = std::fs::read_to_string(companion.join(crate::adopt::MANIFEST_FILE)).ok()?;
    let manifest = serde_yaml::from_str::<Manifest>(&text).ok()?;
    let entry = manifest.verify?.models.remove(id)?;
    if entry.file.trim().is_empty() || entry.spec.trim().is_empty() {
        return None;
    }
    Some(ModelTarget {
        file: entry.file,
        spec: entry.spec,
    })
}

/// The narrowed unit a verdict for `id` was checked against, for provenance (#489 / #484), or
/// `None` when it is the whole subject. The code world can be a member crate; the model world can
/// be a standalone module+spec. Recorded so a sub-unit proof is never read as a whole-subject one.
fn verified_scope(
    companion: &Path,
    id: &str,
    requirement: &Requirement,
    subject: &Path,
    code_root: &Path,
) -> Option<String> {
    match grounding::default_category(requirement) {
        crate::grounding::BindCategory::Code if code_root != subject => {
            let rel = code_root.strip_prefix(subject).unwrap_or(code_root);
            Some(format!("crate {}", rel.display()))
        }
        crate::grounding::BindCategory::Model => {
            verify_model(companion, id).map(|t| format!("model {} in {}", t.spec, t.file))
        }
        _ => None,
    }
}

/// The result of asking to verify one requirement. Every non-verdict variant is an honest
/// "why there is no verdict yet" the operator can act on — never an error, never a fabricated
/// pass. Only [`VerifyOutcome::Verdict`] carries a real answer.
///
/// An *unknown id* is not a variant here: [`verify`] returns `Ok(None)` for it, so the server
/// can answer 404 (distinct from a 409 unadopted subject), mirroring the detail read.
// The `Verdict` variant is much larger than the honest-not-yet-verifiable ones, but this enum is a
// one-shot return value — built once per `verify` call and immediately consumed or serialized,
// never held in a collection — so boxing would buy no memory and only add deref noise at every
// match site. `large_enum_variant` targets types stored in quantity, which this is not.
#[allow(clippy::large_enum_variant)]
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
    /// The stored verdict, served without re-running the ensemble because nothing that determines
    /// it has moved (REQ091). The incremental path: a fresh run would reach this exact verdict, so
    /// the minutes a real run costs are skipped. Distinct from `Verdict` so a caller can say the
    /// verdict was served from cache rather than freshly produced — it is never dressed up as a run.
    Cached(crate::verdict::VerdictReport),
}

/// Verify one requirement in `subject` by id. `Ok(None)` when the id is not in the subject (a
/// 404 for the server); `Err` when the subject is not adopted or the companion cannot be read
/// (a 409). Runs the engine ensemble when the requirement is grounded, else records an honest
/// unknown — it never fabricates a pass and never mutates the subject.
pub fn verify(subject: &Path, id: &str, use_cache: bool) -> Result<Option<VerifyOutcome>> {
    verify_with(subject, id, use_cache, true)
}

/// As [`verify`], but `persist` chooses whether a fresh run records its verdict. `provreq gate`
/// (REQ092) re-verifies with `persist = false` so checking for a regression never mutates the
/// working-tree `verdicts.yml` it compares against (#510); every other caller records.
pub fn verify_with(
    subject: &Path,
    id: &str,
    use_cache: bool,
    persist: bool,
) -> Result<Option<VerifyOutcome>> {
    let (companion, items) = resolve(subject)?;
    let state = draft::load(&companion)?;
    let Some(item) = items.iter().find(|i| i.id == id) else {
        return Ok(None);
    };

    // Tagged-test evidence is independent of formalization (Phase 4c, decision 4): a `Verifies:`
    // tag is a human's claim that a test checks this requirement, and provreq can run it whether or
    // not the requirement was ever formalized. Collected once, then merged into whatever the formal
    // path produces — or standing alone when there is no formalization at all.
    let prefixes: BTreeSet<String> = items
        .iter()
        .map(|i| crate::trace::tags::id_prefix(&i.id))
        .collect();
    let tagged = tagged_evidence(subject, &companion, id, &prefixes);

    let Some(draft) = state.drafts.get(id) else {
        // Un-formalized. A tagged passing test still earns an (asserted) verdict — the payoff that
        // makes provreq useful for the requirements that will never be formalized. With no tag,
        // there is genuinely nothing to run.
        if tagged.is_empty() {
            return Ok(Some(VerifyOutcome::NoDraft));
        }
        let provenance = Provenance {
            requirement_revision: item.revision.clone(),
            subject_commit: subject_head_commit(subject),
            tool_version: env!("CARGO_PKG_VERSION").to_string(),
            // The asserted route runs the subject's own tests against the subject; member-crate /
            // standalone-model targeting is a formal-path concern, so an asserted-only verdict is
            // always whole-subject.
            verified_scope: None,
        };
        let verdict = verdict::aggregate(id, tagged, provenance);
        return finish(
            subject,
            &companion,
            // An asserted-only verdict is whole-subject (the member-crate / model targeting is a
            // formal-path concern), so its scope is the subject itself.
            subject,
            verdict,
            false,
            false,
            BTreeMap::new(),
            None,
            None,
            persist,
        )
        .map(Some);
    };
    // A draft that exists but is not admitted / has no candidate / no longer gates is a
    // formalization that is in progress or broken — a different situation from "un-formalized". A
    // tagged test must not paper over that, so these keep their workflow signal regardless of tags.
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

    // The cat-1 code root (#484): the subject, or a configured member crate for an FFI-heavy
    // subject. A configured-but-absent path is an operator misconfiguration, surfaced clearly
    // rather than left to look like an ungrounded requirement.
    let code_root = cat1_code_root(subject, &companion, id);
    if code_root != subject && !code_root.is_dir() {
        anyhow::bail!(
            "verify.crate points at {}, which does not exist — check the path in provreq.yml \
             (it is relative to the subject)",
            code_root.display()
        );
    }

    // Live grounding dry-run against every wired observable world (code + model) → verdict.
    let resolved = grounding::resolve_bindings(
        subject,
        &code_root,
        &companion,
        &requirement,
        &draft.bindings,
    );
    let grounding_result = grounding::verdict(&requirement, &draft.bindings, &resolved);

    let provenance = Provenance {
        requirement_revision: draft.revision.clone(),
        subject_commit: subject_head_commit(subject),
        tool_version: env!("CARGO_PKG_VERSION").to_string(),
        verified_scope: verified_scope(&companion, id, &requirement, subject, &code_root),
    };

    // Incremental verify (REQ091): the grounding dry-run above is cheap, but the ensemble below is
    // the minutes-long part. When a stored verdict exists and every axis that determines it is
    // unchanged — judged against the same fingerprints `finish` stamps, with the code axis read from
    // the *working tree* so an uncommitted edit is a miss — serve it instead of re-running.
    if use_cache {
        let current = current_provenance(
            subject,
            &companion,
            &code_root,
            &provenance,
            draft,
            &resolved,
            &tagged,
        );
        let store = crate::verdict_store::load(&companion)?;
        if let Some(stored) = store.verdicts.get(id)
            && crate::verdict_store::reusable(stored, &current)
        {
            return Ok(Some(VerifyOutcome::Cached(stored.clone())));
        }
    }

    // Only a GROUNDED requirement reaches an engine: an unresolved binding means there is nothing
    // to check the claim through (R-ground-1). Tagged evidence needs no grounding — it is a test a
    // human vouched for — so it is merged whether or not the formal claim grounded, and can rescue
    // a requirement from the unknown a missing binding or an absent engine would otherwise leave.
    let grounded = matches!(grounding_result, Grounding::Grounded);
    let verdict = if grounded {
        match ensemble_evidence(
            subject,
            &code_root,
            &companion,
            id,
            &requirement,
            &draft.bindings,
            &resolved,
        ) {
            Ensemble::Ran(mut evidence) => {
                evidence.extend(tagged);
                verdict::aggregate(id, evidence, provenance)
            }
            Ensemble::NoEngine(detail) => {
                if tagged.is_empty() {
                    verdict::no_engine(id, detail, provenance)
                } else {
                    verdict::aggregate(id, tagged, provenance)
                }
            }
        }
    } else if tagged.is_empty() {
        verdict::from_grounding(id, &grounding_result, provenance)
    } else {
        verdict::aggregate(id, tagged, provenance)
    };

    let stale = draft::is_stale(draft, item);
    finish(
        subject,
        &companion,
        &code_root,
        verdict,
        grounded,
        stale,
        resolved.code.clone(),
        Some(draft),
        Some(&resolved),
        persist,
    )
    .map(Some)
}

/// The provenance a fresh run *would* stamp, built up front so the incremental path can compare it
/// to the stored verdict's before deciding to run (REQ091). Every field here is computed by the
/// same function [`finish`] uses, so a cache hit means the stored verdict is byte-for-byte what a
/// re-run would record. Only the axes [`crate::verdict_store::reusable`] consults are populated
/// honestly; the rest are left at the report default it ignores.
fn current_provenance(
    subject: &Path,
    companion: &Path,
    code_root: &Path,
    base: &Provenance,
    draft: &draft::Draft,
    resolved: &grounding::Resolutions,
    tagged: &[verdict::Evidence],
) -> crate::verdict::ProvenanceReport {
    crate::verdict::ProvenanceReport {
        requirement_revision: base.requirement_revision.clone(),
        subject_commit: base.subject_commit.clone(),
        tool_version: base.tool_version.clone(),
        verified_scope: base.verified_scope.clone(),
        formalization: draft::formal_fingerprint(draft),
        environment: Some(crate::proving_env::ProvingEnv::current(companion)),
        spec_fingerprint: resolved.spec_fingerprint.clone(),
        trace_fingerprint: crate::monitor::current_fingerprint(subject, companion),
        ui_fingerprint: crate::ui::current_fingerprint(companion),
        source_fingerprint: None,
        // The tagged-source axis a `verify.crate`-narrowed scope hash does not always cover (#508):
        // computed from the freshly-collected tagged evidence so it equals what `finish` stamps.
        tagged_source_fingerprint: tagged_source_fingerprint_of(subject, tagged),
        scope_fingerprint: scope_fingerprint(subject, code_root),
    }
}

/// Stamp the drift fingerprints on the verdict's stored copy, persist it (REQ039 — the verdict
/// becomes durable state, latest per id), and return the outcome. Shared by the formal path and the
/// un-formalized tagged-only path (Phase 4c) so both record a verdict identically. `draft`/`resolved`
/// are `None` on the tagged-only path, where there is no formalization or grounding resolution to
/// fingerprint — those axes are simply absent, never faked.
#[allow(clippy::too_many_arguments)]
fn finish(
    subject: &Path,
    companion: &Path,
    code_root: &Path,
    verdict: Verdict,
    grounded: bool,
    stale: bool,
    resolutions: BTreeMap<String, Resolution>,
    draft: Option<&draft::Draft>,
    resolved: Option<&grounding::Resolutions>,
    persist: bool,
) -> Result<VerifyOutcome> {
    // The gate re-verifies with `persist = false` (#510): compute the outcome and write nothing, so
    // checking for a regression never mutates the `verdicts.yml` it compares against. The fingerprint
    // stamping and store write are the only side effects, so both live inside this guard.
    if persist {
        let mut report = verdict::report(&verdict);
        report.provenance.formalization = draft.and_then(draft::formal_fingerprint);
        report.provenance.environment = Some(crate::proving_env::ProvingEnv::current(companion));
        report.provenance.spec_fingerprint = resolved.and_then(|r| r.spec_fingerprint.clone());
        report.provenance.trace_fingerprint =
            crate::monitor::current_fingerprint(subject, companion);
        report.provenance.ui_fingerprint = crate::ui::current_fingerprint(companion);
        report.provenance.source_fingerprint = subject_source_fingerprint(subject);
        // The precise tagged-source axis (Phase 4c): the whole-tree source fingerprint already stales
        // this verdict when a tagged file changes, but this pins exactly which tagged source backed it.
        report.provenance.tagged_source_fingerprint = tagged_source_fingerprint(subject, &verdict);
        // The working-tree code hash is the incremental cache key (REQ091) — stamped the same way the
        // cache check reads it, so the next `verify` can tell an unchanged scope from a moved one.
        report.provenance.scope_fingerprint = scope_fingerprint(subject, code_root);
        let store = crate::verdict_store::load(companion)?;
        // The prior stored verdict, read before the record overwrites it, is what makes the artifact
        // breadcrumb (below) an event and not a per-run trace: it is appended only when the verdict an
        // operator reads — its status, basis, and correspondence — actually changes (#535).
        let prior = store
            .verdicts
            .get(&report.id)
            .map(|v| (v.status.clone(), v.basis.clone(), v.correspondence.clone()));
        let breadcrumb = crate::provreq::VerdictBreadcrumb {
            status: report.status.clone(),
            basis: report.basis.clone(),
            correspondence: report.correspondence.clone(),
            at_unix: chrono::Utc::now().timestamp(),
        };
        let id = report.id.clone();
        let recorded = crate::verdict_store::record(&store, report);
        crate::verdict_store::save(companion, &recorded)?;
        record_verdict_breadcrumb(subject, &id, prior.as_ref(), &breadcrumb)?;
    }
    Ok(VerifyOutcome::Verdict {
        verdict,
        stale,
        grounded,
        resolutions,
    })
}

/// Whether a verdict is worth a review-log breadcrumb: the first verdict a requirement ever earns,
/// or any later one that differs from the one on record in its status, its basis, or its
/// correspondence. A re-verify that reaches the identical verdict is not an event — the review log
/// is an event stream, not a per-run trace. Basis and correspondence count because an upgrade an
/// operator cares about — a bounded `holds` becoming a proven one, a mechanical correspondence
/// becoming asserted — keeps the status fixed while changing what the verdict claims (#535).
fn is_verdict_event(
    prior: Option<(&str, Option<&str>, &str)>,
    new: (&str, Option<&str>, &str),
) -> bool {
    prior != Some(new)
}

/// Append a `provreq-verdict` breadcrumb to the requirement's Provreq artifact when the verdict
/// changes (Phase 4e, REQ078; basis/correspondence changes added in #535). Provreq-only: a Doorstop
/// subject has no review log to write to, so this is a no-op there and the verdict stays recorded in
/// `verdicts.yml` regardless. The verdict is already persisted when this runs, so a write failure
/// surfaces without losing the verdict.
fn record_verdict_breadcrumb(
    subject: &Path,
    id: &str,
    prior: Option<&(String, Option<String>, String)>,
    breadcrumb: &crate::provreq::VerdictBreadcrumb,
) -> Result<()> {
    let prior_ref = prior.map(|(s, b, c)| (s.as_str(), b.as_deref(), c.as_str()));
    let new = (
        breadcrumb.status.as_str(),
        breadcrumb.basis.as_deref(),
        breadcrumb.correspondence.as_str(),
    );
    if !is_verdict_event(prior_ref, new) {
        return Ok(());
    }
    let req_root = crate::adopt::requirements_root(subject);
    if crate::provreq::discover(&req_root).is_empty() {
        return Ok(());
    }
    crate::provreq::ProvreqSource::new(req_root).record_verdict(id, breadcrumb)
}

/// The asserted evidence a resolved `Verifies:` tag yields for requirement `id`: scan the subject
/// for tags, keep the `Verifies:` ones whose id matches `id` (up to the same `-`/`_`-insensitive
/// canonical form the ids are compared by), and run each. `Implements:` tags are not evidence —
/// they say code *realises* the requirement, not that it *checks* it. Empty when nothing tags `id`.
fn tagged_evidence(
    subject: &Path,
    companion: &Path,
    id: &str,
    prefixes: &BTreeSet<String>,
) -> Vec<verdict::Evidence> {
    let want = canonical_id(id);
    let cargo = crate::trace::run::CargoTestConfig::load(companion);
    crate::trace::scan(subject, companion, prefixes)
        .iter()
        .filter(|t| t.kind == crate::trace::TraceKind::Verifies && canonical_id(&t.req_id) == want)
        .map(|t| crate::trace::run::evidence_for(subject, t, &cargo))
        .collect()
}

/// A requirement id compared `-`/`_`-insensitively and case-folded, so a `Verifies: REQ-021` tag
/// matches requirement `REQ021` (the same normalisation the traceability reader used).
fn canonical_id(id: &str) -> String {
    id.chars()
        .filter(|c| *c != '-' && *c != '_')
        .collect::<String>()
        .to_ascii_uppercase()
}

/// A fingerprint of just the tagged source files this verdict's asserted evidence rests on
/// (Phase 4c). `None` when the verdict has no asserted evidence. Reads the files by their
/// subject-relative locations; a file that cannot be read contributes its path but no content, so
/// the axis still moves if the file later appears or disappears.
fn tagged_source_fingerprint(subject: &Path, verdict: &Verdict) -> Option<String> {
    tagged_source_fingerprint_of(subject, &verdict.evidence)
}

/// The tagged-source fingerprint over a slice of evidence — the shared core of
/// [`tagged_source_fingerprint`], so the incremental cache check can compute the *current* value
/// from the freshly-collected tagged evidence before a verdict exists (#508), matching byte-for-byte
/// what `finish` later stamps from the full verdict. Only asserted (tagged) evidence carries a
/// `source_location`, so taking the whole verdict or just the tagged vec yields the same hash.
fn tagged_source_fingerprint_of(subject: &Path, evidence: &[verdict::Evidence]) -> Option<String> {
    let mut files: Vec<&std::path::PathBuf> = evidence
        .iter()
        .filter_map(|e| e.source_location.as_ref().map(|l| &l.file))
        .collect();
    files.sort();
    files.dedup();
    if files.is_empty() {
        return None;
    }
    let mut blob = String::new();
    for file in files {
        blob.push_str(&file.to_string_lossy());
        blob.push('\0');
        if let Ok(text) = std::fs::read_to_string(subject.join(file)) {
            blob.push_str(&text);
        }
        blob.push('\0');
    }
    Some(crate::source::content_hash(&blob))
}

/// Run the engine ensemble for a grounded requirement and aggregate the evidence (D2b).
///
/// Dispatch is by engine name, not category, because category 1 is an **ensemble**: Kani AND
/// Creusot AND Prusti all run and their evidence is aggregated. Category 2a routes to TLC. Each
/// engine has its own lowering, and none silently inherits another's. 2b routes to MonPoly (#233),
/// and 3 to a real browser through a WebDriver grid (#245) — the last category to be wired, which
/// means the `no_engine` gate below no longer has an unwired category behind it. It now fires only
/// when an engine that *is* wired is not installed here.
pub fn run_ensemble(
    subject: &Path,
    companion: &Path,
    id: &str,
    requirement: &Requirement,
    bindings: &[Binding],
    resolved: &grounding::Resolutions,
    provenance: Provenance,
) -> Verdict {
    let code_root = cat1_code_root(subject, companion, id);
    match ensemble_evidence(
        subject,
        &code_root,
        companion,
        id,
        requirement,
        bindings,
        resolved,
    ) {
        Ensemble::Ran(evidence) => verdict::aggregate(id, evidence, provenance),
        Ensemble::NoEngine(detail) => verdict::no_engine(id, detail, provenance),
    }
}

/// The engine ensemble's own evidence, or the reason none ran (Phase 4c split it out of
/// [`run_ensemble`] so the verify flow can merge tagged-test evidence in before aggregating).
enum Ensemble {
    Ran(Vec<verdict::Evidence>),
    NoEngine(Vec<String>),
}

/// Run every ready engine for a grounded requirement and collect its [`verdict::Evidence`], or
/// report that none is ready. The aggregation into a verdict is the caller's (so tagged evidence
/// can join first).
fn ensemble_evidence(
    subject: &Path,
    code_root: &Path,
    companion: &Path,
    id: &str,
    requirement: &Requirement,
    bindings: &[Binding],
    resolved: &grounding::Resolutions,
) -> Ensemble {
    let category = grounding::default_category(requirement);
    let engines = crate::engine::engines_for(category);

    // The code world can be a member crate (#484); every other world lives with the subject. Only
    // the category-1 engines run against `code_root` — its cargo crate is what Kani codegens and
    // whose `creusot-contracts` dependency Creusot's readiness reads — so detection and evidence for
    // this category use it, while model/runtime/UI engines stay on the subject.
    let engine_root = if category == crate::grounding::BindCategory::Code {
        code_root
    } else {
        subject
    };

    // The ensemble runs every engine that is ready; the others are reported but do not block,
    // as long as one can answer (D2b). No engine ready means nothing checked the property — an
    // honest no-engine that names who must act (wiring is ours, installing is the operator's).
    let ready: Vec<&crate::engine::Engine> = engines
        .iter()
        .filter(|e| crate::engine::detect_for_subject(e, engine_root, Some(companion)).is_ready())
        .collect();
    if ready.is_empty() {
        let detail = engines
            .iter()
            .map(|e| {
                format!(
                    "category {} routes to {} — {}",
                    category.as_label(),
                    e.name,
                    crate::engine::detect_for_subject(e, engine_root, Some(companion)).describe()
                )
            })
            .collect();
        return Ensemble::NoEngine(detail);
    }

    let evidence = ready
        .iter()
        .map(|e| match e.name {
            "Kani" => kani_evidence(engine_root, companion, id, requirement, bindings, resolved),
            "Creusot" => creusot_evidence(engine_root, id, requirement, bindings, resolved),
            "Prusti" => prusti_evidence(engine_root, id, requirement, bindings, resolved),
            "TLC (TLA+)" => tlc_evidence(subject, companion, id, requirement, bindings),
            "MonPoly" => monpoly_evidence(subject, companion, requirement, bindings),
            "Selenium (WebDriver)" => ui_evidence(companion, requirement, bindings),
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
    Ensemble::Ran(evidence)
}

/// Category 1 → Kani (REQ027): lower to an additive proof harness, run it, map to evidence.
/// A subject that is not a cargo crate or a claim that cannot be faithfully lowered is honest
/// `inconclusive` evidence, never approximated (D2).
fn kani_evidence(
    subject: &Path,
    companion: &Path,
    id: &str,
    requirement: &Requirement,
    bindings: &[Binding],
    resolved: &grounding::Resolutions,
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
        &resolved.code,
        &resolved.sorts,
        &crate::kani::harness_name(id),
    ) {
        Ok(h) => h,
        Err(e) => return verdict::Evidence::inconclusive("Kani", vec![e.reason]),
    };
    // The subject's own bounds (#117): what a bounded `holds` from this run is worth.
    let bounds = crate::kani::Bounds::load(companion);
    crate::kani::run(subject, &harness, &bounds).into_evidence(&bounds)
}

/// Category 1 → Creusot (REQ031): the ensemble's deductive member. Lower to an additive in-crate
/// proof harness, run it, map to evidence. A claim that cannot be faithfully lowered — or a
/// subject with no crate root — is honest `inconclusive`, never approximated (D2).
fn creusot_evidence(
    subject: &Path,
    id: &str,
    requirement: &Requirement,
    bindings: &[Binding],
    resolved: &grounding::Resolutions,
) -> verdict::Evidence {
    // Redirect any predicate whose logic mirror is staged in the subject onto that mirror, since
    // pearlite may only call `#[logic]` items. Creusot-only: Kani executes these functions and must
    // keep calling the program ones. A predicate with no staged mirror is left pointing at the
    // program function and fails as before, naming it — never silently.
    let sources = predicate_sources(subject, &resolved.code);
    let (bindings, code) = crate::creusot::with_mirrors(bindings, &resolved.code, &sources);
    let harness = match crate::creusot::lower(
        requirement,
        &bindings,
        &code,
        &resolved.sorts,
        &crate::creusot::harness_name(id),
    ) {
        Ok(h) => h,
        Err(e) => return verdict::Evidence::inconclusive("Creusot", vec![e.reason]),
    };
    crate::creusot::run(subject, &harness).into_evidence()
}

/// The text of every file a resolved predicate lives in, keyed by subject-relative path. Used to
/// see which logic mirrors are actually staged. A file that cannot be read is simply absent, which
/// reads as "no mirror here" — the conservative answer, since it keeps the program function.
fn predicate_sources(
    subject: &Path,
    resolutions: &BTreeMap<String, Resolution>,
) -> BTreeMap<String, String> {
    let mut sources = BTreeMap::new();
    for res in resolutions.values() {
        if let Resolution::Resolved { at, .. } = res
            && !sources.contains_key(&at.file)
            && let Ok(text) = std::fs::read_to_string(subject.join(&at.file))
        {
            sources.insert(at.file.clone(), text);
        }
    }
    sources
}

/// Category 1 → Prusti (REQ032): the ensemble's second deductive member. Lower to an additive
/// in-crate proof harness, run it, map to evidence. A subject without `prusti-contracts` is honest
/// `inconclusive`, never approximated (D2).
fn prusti_evidence(
    subject: &Path,
    id: &str,
    requirement: &Requirement,
    bindings: &[Binding],
    resolved: &grounding::Resolutions,
) -> verdict::Evidence {
    let harness = match crate::prusti::lower(
        requirement,
        bindings,
        &resolved.code,
        &resolved.sorts,
        &crate::prusti::harness_name(id),
    ) {
        Ok(h) => h,
        Err(e) => return verdict::Evidence::inconclusive("Prusti", vec![e.reason]),
    };
    crate::prusti::run(subject, &harness).into_evidence()
}

/// Category 2a → TLC (REQ029): locate the subject's `Spec`, take the operator's constant model,
/// lower to an additive TLA+ module with a temporal property, run TLC against the spec, map to
/// evidence. A missing `Spec`, a constant provreq cannot write, or an un-lowerable claim is
/// honestly `inconclusive`, never approximated (D2).
/// Run MonPoly over the operator's declared trace (#233).
///
/// This is where #230's two refusals finally reach a verdict rather than a return type: a missing
/// trace and an empty one both come back through [`crate::monitor::Extent::read`] as an honest
/// `inconclusive` naming the path — never as a monitor that saw no violations.
fn monpoly_evidence(
    subject: &Path,
    companion: &Path,
    requirement: &Requirement,
    bindings: &[Binding],
) -> verdict::Evidence {
    let engine = crate::monitor::ENGINE;
    let monitor = match crate::monitor::Monitor::load(subject, companion) {
        Ok(Some(m)) => m,
        Ok(None) => {
            return verdict::Evidence::inconclusive(
                engine,
                vec![
                    "this subject declares no `monitor:` block in provreq.yml, so there is no \
                     trace to monitor. A category-2b claim is about events in a log; provreq reads \
                     one the subject already produces and never runs the subject itself"
                        .to_string(),
                ],
            );
        }
        Err(reason) => return verdict::Evidence::inconclusive(engine, vec![reason]),
    };
    // Read the trace BEFORE lowering: a claim that lowers perfectly over a log that is missing or
    // empty is still a claim about nothing, and the extent is what the verdict must carry (#229).
    let extent = match crate::monitor::Extent::read(&monitor) {
        Ok(e) => e,
        Err(reason) => return verdict::Evidence::inconclusive(engine, vec![reason]),
    };
    // One property per requirement is what every other engine here checks; a `require` block with
    // several is refused rather than half-checked.
    let [prop] = &requirement.require[..] else {
        return verdict::Evidence::inconclusive(
            engine,
            vec![format!(
                "the requirement carries {} `require` claims, and provreq lowers one metric \
                 deadline per run — split them, rather than have a verdict report on some of them",
                requirement.require.len()
            )],
        );
    };
    let claim = match crate::monitor::lower(requirement, prop, &monitor, bindings) {
        Ok(c) => c,
        Err(e) => return verdict::Evidence::inconclusive(engine, vec![e.reason]),
    };
    crate::monitor::run(&monitor, &claim).into_evidence(&extent)
}

/// Category 3 → a real browser, driven through a WebDriver grid (#245).
///
/// Mirrors [`monpoly_evidence`] and differs from it in exactly one way that matters: there is no
/// artifact to read before running. A monitor's trace exists before the monitor does, so its extent
/// is checked first; a driver *makes* the run it then judges, so what a verdict covers is only
/// knowable afterwards — which is why the reach line is built from the outcome rather than read
/// ahead of it.
fn ui_evidence(
    companion: &Path,
    requirement: &Requirement,
    bindings: &[Binding],
) -> verdict::Evidence {
    let engine = crate::ui::ENGINE;
    let ui = match crate::ui::Ui::load(companion) {
        Ok(Some(ui)) => ui,
        Ok(None) => {
            return verdict::Evidence::inconclusive(
                engine,
                vec![
                    "this subject declares no `ui:` block in provreq.yml, so there is no \
                     deployment to drive and no steps to drive it with. A category-3 claim is \
                     about what a running system shows; the operator names it, and provreq never \
                     starts one"
                        .to_string(),
                ],
            );
        }
        Err(reason) => return verdict::Evidence::inconclusive(engine, vec![reason]),
    };
    // One property per requirement, as every other engine here checks — a `require` block with
    // several is refused rather than half-driven.
    let [prop] = &requirement.require[..] else {
        return verdict::Evidence::inconclusive(
            engine,
            vec![format!(
                "the requirement carries {} `require` claims, and provreq drives one script per \
                 run — split them, rather than have a verdict report on some of them",
                requirement.require.len()
            )],
        );
    };
    let claim = match crate::ui::lower(requirement, prop, &ui, bindings) {
        Ok(c) => c,
        Err(e) => return verdict::Evidence::inconclusive(engine, vec![e.reason]),
    };
    crate::ui::run(&ui, &claim).into_evidence(&ui, &claim)
}

fn tlc_evidence(
    subject: &Path,
    companion: &Path,
    id: &str,
    requirement: &Requirement,
    bindings: &[Binding],
) -> verdict::Evidence {
    let spec_paths = crate::spec_paths::SpecPaths::load(subject, companion);
    // A cat-2a requirement may target a standalone sub-machine (#489): its own module + spec
    // operator, checked instead of the composed `Spec`, for a property that is decidable there but
    // explodes on the whole model. Absent a target, the global `Spec` is located as before.
    let spec_operator = verify_model(companion, id)
        .map(|t| t.spec)
        .unwrap_or_else(|| crate::tlc::SPEC_OPERATOR.to_string());
    let site = match crate::tlc::locate_operator(subject, companion, &spec_paths, &spec_operator) {
        Ok(site) => site,
        Err(reason) => return verdict::Evidence::inconclusive("TLC (TLA+)", vec![reason]),
    };
    let constants = match crate::tlc::Constants::load(companion) {
        Ok(c) => c,
        Err(reason) => return verdict::Evidence::inconclusive("TLC (TLA+)", vec![reason]),
    };
    // Held against what the model declares before it can reach a verdict (#211): TLC ignores an
    // assignment for a name its spec does not declare, so an unchecked one would be reported as
    // part of a model that never included it.
    if let Err(reason) = constants.check_declared(&site.declared_constants) {
        return verdict::Evidence::inconclusive("TLC (TLA+)", vec![reason]);
    }
    let check = match crate::tlc::lower_with_spec(
        requirement,
        &site.module,
        bindings,
        &constants,
        &crate::tlc::module_name(id),
        &spec_operator,
    ) {
        Ok(c) => c,
        Err(e) => return verdict::Evidence::inconclusive("TLC (TLA+)", vec![e.reason]),
    };
    crate::tlc::run(&site, &check).into_evidence(&constants)
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

/// Best-effort fingerprint of the subject's tracked source at HEAD (#271, REQ071), excluding the
/// companion tree and the doorstop document dirs — the records whose drift other axes already own
/// (the per-item revision for prose, the store's own persistence for verdicts). This is what makes
/// the code-drift axis source-granular: a commit that changes none of the fingerprinted source —
/// most immediately, the commit that lands `verdicts.yml` itself — is not code drift.
///
/// `None` when the subject is not a git repo, exactly like [`subject_head_commit`] — never
/// fabricated. Reads HEAD, so a dirty tree is as invisible here as it is to the commit axis.
///
/// Implements: REQ071
pub fn subject_source_fingerprint(subject: &Path) -> Option<String> {
    // The tracked tree at HEAD, NUL-separated so paths need no unquoting: `<mode> <type>
    // <blob>\t<path>\0`. The blob hashes carry the content, so hashing the filtered listing
    // fingerprints exactly the surviving files' contents and paths.
    let listing = std::process::Command::new("git")
        .arg("-C")
        .arg(subject)
        .args(["ls-tree", "-r", "-z", "HEAD"])
        .output()
        .ok()
        .filter(|o| o.status.success())?;

    let excluded = subject_record_dirs(subject);

    let text = String::from_utf8_lossy(&listing.stdout);
    let filtered: Vec<&str> = text
        .split('\0')
        .filter(|entry| {
            let path = entry.split('\t').nth(1).unwrap_or("");
            !excluded
                .iter()
                .any(|ex| std::path::Path::new(path).starts_with(ex))
        })
        .collect();
    hash_lines(&filtered)
}

/// The subject-relative directories whose contents belong to another drift axis, not to the code:
/// the companion tree (the store's own persistence), the doorstop document dirs, and the Provreq
/// requirement collections (the per-item revision axis owns their prose). Shared by the HEAD source
/// fingerprint and the working-tree [`scope_fingerprint`] so the two exclude exactly the same records
/// — a file in this set must never move either fingerprint, or storing a verdict would stale it.
fn subject_record_dirs(subject: &Path) -> Vec<PathBuf> {
    let mut excluded: Vec<PathBuf> = Vec::new();
    if let Ok(Some(companion)) = crate::adopt::find_companion(subject)
        && let Ok(rel) = companion.strip_prefix(subject)
    {
        excluded.push(rel.to_path_buf());
    }
    if let Ok(docs) = crate::doorstop::discover(subject) {
        for doc in docs {
            if let Ok(rel) = doc.dir.strip_prefix(subject) {
                excluded.push(rel.to_path_buf());
            }
        }
    }
    // Asked of both sources rather than the resolved one: a subject part-way through a migration
    // holds both, and a requirement left out of this list stales every verdict the moment it is
    // edited (#296).
    for dir in crate::provreq::discover(subject) {
        if let Ok(rel) = dir.strip_prefix(subject) {
            excluded.push(rel.to_path_buf());
        }
    }
    excluded
}

/// A fingerprint of the code a verdict covers, taken over the **working tree** (REQ091) — the cache
/// key the incremental `verify` compares. Walks `code_root` on disk (not HEAD), applying the one
/// shared pruning rule ([`crate::subject_tree`]) and excluding the record dirs
/// ([`subject_record_dirs`]) exactly as the HEAD source fingerprint does, then content-hashes the
/// surviving files' paths and bytes. Reading the working tree is the whole point: an uncommitted
/// edit to the covered code must be a cache miss, so a stored verdict is never served against code
/// the tree already contradicts. `None` — and so never a cache hit — when `code_root` cannot be
/// walked.
///
/// The scope is `code_root`, which on today's single-crate subjects is the whole subject; a
/// `verify.crate` member crate (#484) narrows it. ponytail: this over-approximates — it covers the
/// whole scope unit, not only the exact items `verified_scope` names. Narrowing it to those items
/// would skip more re-runs, but demands call-graph analysis to stay sound (a changed helper outside
/// the named set still affects the proof); it is a safe refinement, not a correctness fix, and waits
/// for a subject where crate-level scoping is not already the whole subject (#483).
fn scope_fingerprint(subject: &Path, code_root: &Path) -> Option<String> {
    if !code_root.is_dir() {
        return None;
    }
    let excluded = subject_record_dirs(subject);
    let is_excluded = |path: &Path| {
        path.strip_prefix(subject)
            .map(|rel| excluded.iter().any(|ex| rel.starts_with(ex)))
            .unwrap_or(false)
    };

    let mut files: Vec<PathBuf> = walkdir::WalkDir::new(code_root)
        .sort_by_file_name()
        .into_iter()
        .filter_entry(|e| {
            if e.file_type().is_dir() {
                !crate::subject_tree::is_pruned_dir(e.path(), e.depth()) && !is_excluded(e.path())
            } else {
                !crate::subject_tree::is_pruned_file(e.path())
            }
        })
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file() && !is_excluded(e.path()))
        .map(|e| e.into_path())
        .collect();
    files.sort();

    // The same blob shape `tagged_source_fingerprint` uses: each file's scope-relative path then its
    // bytes, NUL-separated, through the shared content hash. Path-relative to `code_root` so moving
    // the subject on disk does not change the fingerprint.
    let mut blob = String::new();
    for file in &files {
        let rel = file.strip_prefix(code_root).unwrap_or(file);
        blob.push_str(&rel.to_string_lossy());
        blob.push('\0');
        if let Ok(bytes) = std::fs::read(file) {
            blob.push_str(&String::from_utf8_lossy(&bytes));
        }
        blob.push('\0');
    }
    Some(crate::source::content_hash(&blob))
}

/// Hash a listing through `git hash-object --stdin` — content-addressed and stable across runs,
/// with no hashing dependency of our own (std's hashers are seeded per process on purpose).
fn hash_lines(lines: &[&str]) -> Option<String> {
    use std::io::Write as _;
    let mut child = std::process::Command::new("git")
        .args(["hash-object", "--stdin"])
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .spawn()
        .ok()?;
    {
        let mut stdin = child.stdin.take()?;
        for line in lines {
            stdin.write_all(line.as_bytes()).ok()?;
            stdin.write_all(b"\n").ok()?;
        }
    }
    let out = child
        .wait_with_output()
        .ok()
        .filter(|o| o.status.success())?;
    let hash = String::from_utf8_lossy(&out.stdout).trim().to_string();
    (!hash.is_empty()).then_some(hash)
}

/// Every out-of-commit fingerprint for the current world, read in one place.
///
/// [`crate::verdict_store`] deliberately touches no filesystem, so the reading lives here — and
/// gathering all three together means the browse surfaces cannot drift into anchoring against a
/// different set of axes than `verify` stamped.
pub fn current_fingerprints(
    subject: &Path,
    companion: &Path,
) -> crate::verdict_store::Fingerprints {
    crate::verdict_store::Fingerprints {
        spec: crate::tla_adapter::current_external_fingerprint(subject, companion),
        trace: crate::monitor::current_fingerprint(subject, companion),
        ui: crate::ui::current_fingerprint(companion),
        source: subject_source_fingerprint(subject),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Verifies: REQ038 — verifying an id the subject does not contain is `Ok(None)` (a 404 for
    // the server), distinct from an unadopted-subject error.
    #[test]
    fn unknown_id_is_none() {
        let subject = adopted_subject_with_one_item();
        let out = verify(subject.path(), "REQ999", false).unwrap();
        assert!(out.is_none());
    }

    // Verifies: #484 — the cat-1 code root defaults to the subject, and a `verify.crate` in the
    // companion manifest points it at a member crate (joined onto the subject). An unset key or an
    // unrelated `verify.cargo` block leaves it on the subject.
    #[test]
    fn cat1_code_root_defaults_to_subject_and_honours_verify_crate() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();

        // No manifest at all → the subject itself.
        assert_eq!(cat1_code_root(root, root, "REQ001"), root);

        // verify.crate set → subject joined with the member path.
        std::fs::write(root.join("provreq.yml"), "verify:\n  crate: crates/core\n").unwrap();
        assert_eq!(
            cat1_code_root(root, root, "REQ001"),
            root.join("crates/core")
        );

        // A verify block without `crate` (only cargo) → still the subject.
        std::fs::write(
            root.join("provreq.yml"),
            "verify:\n  cargo:\n    features: [proof]\n",
        )
        .unwrap();
        assert_eq!(cat1_code_root(root, root, "REQ001"), root);
    }

    // Verifies: #483 — a per-requirement `verify.crates` entry overrides the subject-wide default
    // for that requirement, requirements without an entry fall back to the default, and with no
    // default a requirement with no entry is the subject itself.
    #[test]
    fn cat1_code_root_honours_per_requirement_override() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();

        std::fs::write(
            root.join("provreq.yml"),
            "verify:\n  crate: crates/core\n  crates:\n    PER-0007: crates/proto\n",
        )
        .unwrap();
        // The named requirement uses its override.
        assert_eq!(
            cat1_code_root(root, root, "PER-0007"),
            root.join("crates/proto")
        );
        // A requirement with no entry falls back to the subject default.
        assert_eq!(
            cat1_code_root(root, root, "PER-0002"),
            root.join("crates/core")
        );

        // A per-requirement map with no subject default: named req overrides, others are the subject.
        std::fs::write(
            root.join("provreq.yml"),
            "verify:\n  crates:\n    PER-0007: crates/proto\n",
        )
        .unwrap();
        assert_eq!(
            cat1_code_root(root, root, "PER-0007"),
            root.join("crates/proto")
        );
        assert_eq!(cat1_code_root(root, root, "PER-0002"), root);
    }

    // Verifies: #489 — a per-requirement `verify.models` entry is read as a model target, and a
    // requirement with no entry (or no manifest) has none.
    #[test]
    fn verify_model_reads_a_per_requirement_target() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        assert_eq!(verify_model(root, "PER-0022"), None);

        std::fs::write(
            root.join("provreq.yml"),
            "verify:\n  models:\n    PER-0022: { file: specs/ScanLivenessModel.tla, spec: ScanSpec }\n",
        )
        .unwrap();
        assert_eq!(
            verify_model(root, "PER-0022"),
            Some(ModelTarget {
                file: "specs/ScanLivenessModel.tla".into(),
                spec: "ScanSpec".into(),
            })
        );
        assert_eq!(
            verify_model(root, "OTHER"),
            None,
            "a requirement with no entry has no target"
        );
    }

    // Verifies: #489 / #484 — the provenance scope names the narrowed unit a verdict was checked
    // against (a member crate for cat-1, a standalone model+spec for cat-2a), or nothing when it is
    // the whole subject.
    #[test]
    fn verified_scope_names_the_crate_the_model_or_nothing() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        let code_req = crate::prl::gate(
            "requirement r { category: 1 vocabulary { state ok } require { always ok } }",
        )
        .unwrap()
        .requirement;

        // cat-1: whole subject → None; a member crate → "crate <rel>".
        assert_eq!(verified_scope(root, "R1", &code_req, root, root), None);
        assert_eq!(
            verified_scope(root, "R1", &code_req, root, &root.join("crates/core")),
            Some("crate crates/core".to_string())
        );

        // cat-2a: a declared model target → "model <spec> in <file>"; no entry → None.
        std::fs::write(
            root.join("provreq.yml"),
            "verify:\n  models:\n    R2: { file: specs/M.tla, spec: ScanSpec }\n",
        )
        .unwrap();
        let model_req = crate::prl::gate(
            "requirement r { category: 2a vocabulary { state accepted(m), succeeded(m) } \
             require { each m: Message . accepted(m) leads_to succeeded(m) } }",
        )
        .unwrap()
        .requirement;
        assert_eq!(
            verified_scope(root, "R2", &model_req, root, root),
            Some("model ScanSpec in specs/M.tla".to_string())
        );
        assert_eq!(verified_scope(root, "RX", &model_req, root, root), None);
    }

    // Verifies: REQ038 — an item with no draft is an honest NoDraft, never an error and never a
    // fabricated verdict.
    #[test]
    fn undrafted_item_is_no_draft() {
        let subject = adopted_subject_with_one_item();
        let out = verify(subject.path(), "REQ001", false).unwrap().unwrap();
        assert!(matches!(out, VerifyOutcome::NoDraft), "got {out:?}");
    }

    // Verifies: REQ038 — an unadopted subject is an error (the server maps it to 409), not None.
    #[test]
    fn unadopted_subject_is_error() {
        let empty = tempfile::tempdir().unwrap();
        assert!(verify(empty.path(), "REQ001", false).is_err());
    }

    // Verifies: REQ071 / #271 — the fingerprint covers the tracked source and nothing the other
    // drift axes already own: a commit touching only the companion tree or the requirement
    // documents leaves it unchanged, a commit touching source moves it.
    #[test]
    fn source_fingerprint_ignores_the_records_other_axes_own() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        std::fs::create_dir_all(root.join("requirements-doorstop")).unwrap();
        std::fs::write(
            root.join("requirements-doorstop/.doorstop.yml"),
            "settings:\n  prefix: REQ\n  digits: 3\n",
        )
        .unwrap();
        std::fs::write(
            root.join("requirements-doorstop/REQ001.yml"),
            "active: true\nlevel: 1.0\nnormative: true\nref: ''\nreviewed: null\ntext: |\n  A requirement.\n",
        )
        .unwrap();
        std::fs::create_dir_all(root.join("ProvableRequirements-doorstop")).unwrap();
        std::fs::write(
            root.join("ProvableRequirements-doorstop/provreq.yml"),
            "version: 1\n",
        )
        .unwrap();
        std::fs::write(root.join("lib.rs"), "fn a() {}\n").unwrap();
        git(root, &["init", "-q"]);
        git(root, &["add", "-A"]);
        commit(root, "base");
        let base = subject_source_fingerprint(root).expect("a repo has a fingerprint");

        // A companion-only commit — the shape that previously staled every stored verdict.
        std::fs::write(
            root.join("ProvableRequirements-doorstop/verdicts.yml"),
            "verdicts: {}\n",
        )
        .unwrap();
        git(root, &["add", "-A"]);
        commit(root, "store a verdict");
        assert_eq!(
            subject_source_fingerprint(root).as_ref(),
            Some(&base),
            "the companion tree's drift is the store's own concern, not code drift"
        );

        // A requirement-document commit — the per-item revision axis owns this.
        std::fs::write(
            root.join("requirements-doorstop/REQ001.yml"),
            "active: true\nlevel: 1.0\nnormative: true\nref: ''\nreviewed: null\ntext: |\n  Moved prose.\n",
        )
        .unwrap();
        git(root, &["add", "-A"]);
        commit(root, "edit prose");
        assert_eq!(
            subject_source_fingerprint(root).as_ref(),
            Some(&base),
            "requirement documents belong to the prose axis"
        );

        // A source commit — the thing the axis exists to see.
        std::fs::write(root.join("lib.rs"), "fn a() { let _ = 1; }\n").unwrap();
        git(root, &["add", "-A"]);
        commit(root, "edit source");
        assert_ne!(
            subject_source_fingerprint(root).as_ref(),
            Some(&base),
            "source movement must be visible"
        );
    }

    // Verifies: REQ091 — the scope fingerprint reads the working tree (so an uncommitted edit is a
    // cache miss, unlike the HEAD source fingerprint), prunes the build/OS-record trees by the one
    // shared rule, and is stable when nothing under the scope changed. No git: it is deliberately
    // blind to commits and alive to the files on disk.
    #[test]
    fn scope_fingerprint_tracks_the_working_tree_and_prunes_noise() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        std::fs::create_dir_all(root.join("src")).unwrap();
        std::fs::write(root.join("src/lib.rs"), "fn a() {}\n").unwrap();

        let base = scope_fingerprint(root, root).expect("a readable dir has a fingerprint");
        assert_eq!(
            scope_fingerprint(root, root).as_ref(),
            Some(&base),
            "stable when nothing changed"
        );

        // A pruned build tree and an OS record file are not the subject's code.
        std::fs::create_dir_all(root.join("target")).unwrap();
        std::fs::write(root.join("target/junk"), "artifact\n").unwrap();
        std::fs::write(root.join("src/.DS_Store"), "junk").unwrap();
        assert_eq!(
            scope_fingerprint(root, root).as_ref(),
            Some(&base),
            "pruned build output and OS records do not move the scope hash"
        );

        // An UNCOMMITTED edit to covered code moves it — the reason the cache reads the tree, not HEAD.
        std::fs::write(root.join("src/lib.rs"), "fn a() { let _ = 1; }\n").unwrap();
        assert_ne!(
            scope_fingerprint(root, root).as_ref(),
            Some(&base),
            "an uncommitted source edit must be a cache miss"
        );

        // A scope that cannot be read is None — never a stale hit.
        assert_eq!(scope_fingerprint(root, &root.join("nope")), None);
    }

    // Verifies: REQ071 / #296 — the same exemption, for a subject storing requirements as Provreq
    // artifacts. Found by the phase-1 spike: the exclusion list knew only about Doorstop documents,
    // so editing a requirement in a Provreq subject moved the *code*-drift fingerprint and staled
    // every stored verdict — which is exactly the failure #271 fixed for Doorstop. The requirement
    // axis owns requirement prose whatever format it is written in.
    #[test]
    fn source_fingerprint_ignores_provreq_requirements_too() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        let coll = root.join("artifacts/req");
        std::fs::create_dir_all(&coll).unwrap();
        let fixture = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/provreq-subject/artifacts/req");
        for name in [crate::provreq::COLLECTION_FILE, "REQ-queueIsDrained.md"] {
            std::fs::copy(fixture.join(name), coll.join(name)).unwrap();
        }
        std::fs::write(root.join("lib.rs"), "fn a() {}\n").unwrap();
        git(root, &["init", "-q"]);
        git(root, &["add", "-A"]);
        commit(root, "base");
        let base = subject_source_fingerprint(root).expect("a repo has a fingerprint");

        let artifact = coll.join("REQ-queueIsDrained.md");
        let edited = std::fs::read_to_string(&artifact)
            .unwrap()
            .replace("eventually removed", "removed within one second");
        std::fs::write(&artifact, edited).unwrap();
        git(root, &["add", "-A"]);
        commit(root, "edit requirement prose");
        assert_eq!(
            subject_source_fingerprint(root).as_ref(),
            Some(&base),
            "a requirement edit is prose drift, not code drift, in either format"
        );

        std::fs::write(root.join("lib.rs"), "fn a() { let _ = 1; }\n").unwrap();
        git(root, &["add", "-A"]);
        commit(root, "edit source");
        assert_ne!(
            subject_source_fingerprint(root).as_ref(),
            Some(&base),
            "source movement must still be visible"
        );
    }

    // Verifies: REQ078 / #342 / #535 — the breadcrumb fires on the first verdict and on any change to
    // the verdict an operator reads (status, basis, or correspondence), and stays silent on a
    // re-verify that reaches the identical verdict (the review log is an event stream, not a per-run
    // trace).
    #[test]
    fn a_verdict_event_is_first_or_changed_verdict() {
        let bounded = ("holds", Some("model-checked (bounded)"), "mechanical");
        assert!(is_verdict_event(None, bounded), "first verdict is an event");
        assert!(
            is_verdict_event(Some(bounded), ("fails", None, "mechanical")),
            "a status change is an event"
        );
        assert!(
            is_verdict_event(Some(bounded), ("holds", Some("proven"), "mechanical")),
            "a basis upgrade (bounded -> proven) is an event (#535)"
        );
        assert!(
            is_verdict_event(
                Some(("holds", Some("proven"), "mechanical")),
                ("holds", Some("proven"), "asserted"),
            ),
            "a correspondence change is an event (#535)"
        );
        assert!(
            !is_verdict_event(
                Some(("holds", Some("proven"), "mechanical")),
                ("holds", Some("proven"), "mechanical"),
            ),
            "an identical verdict is not an event"
        );
    }

    // Verifies: REQ071 / #271 — `None` for a non-repo, exactly like `subject_head_commit`: never
    // fabricated.
    #[test]
    fn a_non_repo_has_no_source_fingerprint() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(subject_source_fingerprint(dir.path()), None);
    }

    fn git(root: &Path, args: &[&str]) {
        let out = std::process::Command::new("git")
            .arg("-C")
            .arg(root)
            .args(args)
            .output()
            .unwrap();
        assert!(out.status.success(), "git {args:?}: {out:?}");
    }

    fn commit(root: &Path, msg: &str) {
        git(
            root,
            &[
                "-c",
                "user.email=t@t",
                "-c",
                "user.name=t",
                "commit",
                "-qm",
                msg,
            ],
        );
    }

    /// A minimal adopted subject with one item and no drafts — mirrors the server test fixture.
    // Verifies: REQ092 / #510 — the gate's re-verify mode (`persist = false`) computes a verdict
    // without writing the store, so gating a regression never mutates the working-tree `verdicts.yml`
    // it checks against. The persisting mode (`persist = true`, the default `verify`) still records.
    #[test]
    fn finish_persist_flag_controls_whether_the_store_is_written() {
        let subject = adopted_subject_with_one_item();
        let (companion, _) = resolve(subject.path()).unwrap();
        let provenance = Provenance {
            requirement_revision: "r1".into(),
            subject_commit: None,
            tool_version: "0.0.0".into(),
            verified_scope: None,
        };
        let verdict = verdict::no_engine("REQ001", vec!["no engine here".into()], provenance);

        finish(
            subject.path(),
            &companion,
            subject.path(),
            verdict.clone(),
            false,
            false,
            BTreeMap::new(),
            None,
            None,
            false,
        )
        .unwrap();
        assert!(
            crate::verdict_store::load(&companion)
                .unwrap()
                .verdicts
                .is_empty(),
            "persist = false must not write the store"
        );

        finish(
            subject.path(),
            &companion,
            subject.path(),
            verdict,
            false,
            false,
            BTreeMap::new(),
            None,
            None,
            true,
        )
        .unwrap();
        assert!(
            crate::verdict_store::load(&companion)
                .unwrap()
                .verdicts
                .contains_key("REQ001"),
            "persist = true records the verdict"
        );
    }

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
