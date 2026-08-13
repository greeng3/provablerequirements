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
    let resolved = grounding::resolve_bindings(subject, &companion, &requirement, &draft.bindings);
    let grounding_result = grounding::verdict(&requirement, &draft.bindings, &resolved);

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
            &resolved,
            provenance,
        )
    } else {
        verdict::from_grounding(id, &grounding_result, provenance)
    };
    // Living loop (REQ039): the verdict becomes durable state. Persist it keyed by id — the latest
    // answer replaces any earlier one — so the backlog/detail can show it and later detect when it
    // has drifted, without re-running an engine. Stamp the formalization fingerprint on the stored
    // copy (REQ045) so drift can later catch the candidate or its bindings moving — a persistence
    // concern the in-memory verdict never carries.
    let mut report = verdict::report(&verdict);
    report.provenance.formalization = draft::formal_fingerprint(draft);
    // Stamp the environment that actually produced this verdict (REQ049) — where it was proved,
    // not what the subject declares it offers. Without it, a verdict proved under one engine build
    // and one proved under another are indistinguishable in the store.
    report.provenance.environment = Some(crate::proving_env::ProvingEnv::current(&companion));
    // Stamp the fingerprint of any spec that lives outside the subject (#120). The subject's commit
    // does not cover such a file, so without this a verdict proved against a sibling repo's model
    // would stay `fresh` while that model moved. `None` when every spec is in-tree, where the
    // subject commit already covers them and a second axis would only flag the same drift twice.
    report.provenance.spec_fingerprint = resolved.spec_fingerprint.clone();
    // Stamp the fingerprint of the monitor's trace (#230), for the same reason and a faster clock:
    // the subject's commit does not cover a log the subject produced, and that log keeps growing.
    // `None` when no monitor is configured, which is every subject with no category-2b requirement.
    report.provenance.trace_fingerprint = crate::monitor::current_fingerprint(subject, &companion);
    // Stamp the fingerprint of the declared UI check (#239). No file to hash on the far end, so
    // this is the declaration: it catches the steps or the deployment URL being edited out from
    // under a verdict that was only ever about the check as it stood.
    report.provenance.ui_fingerprint = crate::ui::current_fingerprint(&companion);
    // Stamp the source fingerprint (REQ071, #271) — the code-drift axis at source granularity, so
    // the commit that lands this very record does not stale it. The commit above stays for
    // reproducibility; this is what freshness is decided on.
    report.provenance.source_fingerprint = subject_source_fingerprint(subject);
    let store = crate::verdict_store::load(&companion)?;
    let recorded = crate::verdict_store::record(&store, report);
    crate::verdict_store::save(&companion, &recorded)?;

    Ok(Some(VerifyOutcome::Verdict {
        verdict,
        stale: draft::is_stale(draft, item),
        grounded,
        resolutions: resolved.code,
    }))
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
    let category = grounding::default_category(requirement);
    let engines = crate::engine::engines_for(category);

    // The ensemble runs every engine that is ready; the others are reported but do not block,
    // as long as one can answer (D2b). No engine ready means nothing checked the property — an
    // honest no-engine that names who must act (wiring is ours, installing is the operator's).
    let ready: Vec<&crate::engine::Engine> = engines
        .iter()
        .filter(|e| crate::engine::detect(e, Some(companion)).is_ready())
        .collect();
    if ready.is_empty() {
        let detail = engines
            .iter()
            .map(|e| {
                format!(
                    "category {} routes to {} — {}",
                    category.as_label(),
                    e.name,
                    crate::engine::detect(e, Some(companion)).describe()
                )
            })
            .collect();
        return verdict::no_engine(id, detail, provenance);
    }

    let evidence = ready
        .iter()
        .map(|e| match e.name {
            "Kani" => kani_evidence(subject, companion, id, requirement, bindings, resolved),
            "Creusot" => creusot_evidence(subject, id, requirement, bindings, resolved),
            "Prusti" => prusti_evidence(subject, id, requirement, bindings, resolved),
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
    verdict::aggregate(id, evidence, provenance)
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
        if let Resolution::Resolved { at, .. } = res {
            if !sources.contains_key(&at.file) {
                if let Ok(text) = std::fs::read_to_string(subject.join(&at.file)) {
                    sources.insert(at.file.clone(), text);
                }
            }
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
            )
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
            )
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
    let site = match crate::tlc::locate_spec(subject, companion, &spec_paths) {
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
    let check = match crate::tlc::lower(
        requirement,
        &site.module,
        bindings,
        &constants,
        &crate::tlc::module_name(id),
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

    // The records whose drift other axes own, relative to the subject root. Best-effort like the
    // listing itself: an unadopted subject simply has no companion to exclude.
    let mut excluded: Vec<std::path::PathBuf> = Vec::new();
    if let Ok(Some(companion)) = crate::adopt::find_companion(subject) {
        if let Ok(rel) = companion.strip_prefix(subject) {
            excluded.push(rel.to_path_buf());
        }
    }
    if let Ok(docs) = crate::doorstop::discover(subject) {
        for doc in docs {
            if let Ok(rel) = doc.dir.strip_prefix(subject) {
                excluded.push(rel.to_path_buf());
            }
        }
    }

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
