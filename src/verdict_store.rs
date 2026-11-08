//! Step 6 — the living loop: verdicts as durable state that drifts.
//!
//! A verdict is produced on demand (Step 4) but does not stay true forever: the requirement prose
//! can move, the subject source can move, or the tool can change underneath it. The D9 provenance
//! every verdict carries (`requirement_revision`, `subject_commit`, the source fingerprint, and
//! `tool_version`, plus the per-axis fingerprints below) is exactly the anchor to detect that —
//! this module persists the verdict keyed by item id and compares its provenance against the
//! current world to decide whether it is still fresh.
//!
//! Persisted as a companion `verdicts.yml`, mirroring `drafts.yml`/`triage.yml`. The stored shape
//! IS the wire shape ([`crate::verdict::VerdictReport`]) — the web surface and the store never
//! diverge on what a verdict looks like. Re-verifying overwrites the stored verdict; a stale one is
//! never silently discarded, only flagged, so the operator decides when to re-run.
//!
//! Implements: REQ039 (persist verdicts; detect drift against provenance; surface freshness)

use crate::verdict::VerdictReport;
use anyhow::{Context, Result};
use std::collections::BTreeMap;
use std::path::Path;

/// The companion file the verdict store persists to.
pub const VERDICT_FILE: &str = "verdicts.yml";

/// Every item's last verdict, keyed by item id. Additive over time — a re-verify replaces one
/// entry, and an item never verified simply has none.
#[derive(Debug, Clone, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
pub struct VerdictStore {
    #[serde(default)]
    pub verdicts: BTreeMap<String, VerdictReport>,
}

impl VerdictStore {
    pub fn new() -> Self {
        Self::default()
    }
}

/// Load the verdict store from the companion root. A missing file is an empty store — nothing has
/// been verified yet, which is honest, not an error.
pub fn load(companion_root: &Path) -> Result<VerdictStore> {
    let path = companion_root.join(VERDICT_FILE);
    if !path.exists() {
        return Ok(VerdictStore::new());
    }
    let text =
        std::fs::read_to_string(&path).with_context(|| format!("reading {}", path.display()))?;
    serde_yaml::from_str(&text).with_context(|| format!("parsing {}", path.display()))
}

/// Write the verdict store to the companion root.
pub fn save(companion_root: &Path, store: &VerdictStore) -> Result<()> {
    let path = companion_root.join(VERDICT_FILE);
    let yaml = serde_yaml::to_string(store).context("serializing verdict store")?;
    std::fs::write(&path, yaml).with_context(|| format!("writing {}", path.display()))
}

/// Record a verdict, returning a new store with it stored under its own id (immutable insert). A
/// later verdict for the same item replaces the earlier one — the store holds the *latest* answer.
pub fn record(store: &VerdictStore, verdict: VerdictReport) -> VerdictStore {
    let mut verdicts = store.verdicts.clone();
    verdicts.insert(verdict.id.clone(), verdict);
    VerdictStore { verdicts }
}

/// What a verdict was produced against, distilled to what drift needs: the subject commit and the
/// tool version (the requirement revision is per-item, compared separately against each item).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DriftAnchor {
    pub subject_commit: Option<String>,
    /// The fingerprint of the subject's tracked source *now* (#271) — the tree at HEAD minus the
    /// companion tree and the requirement documents, whose drift other axes own. `None` when the
    /// subject is not a git repo. When both sides carry one, this decides the code-drift axis
    /// instead of the commit, so committing the verdict record does not stale the record.
    pub source_fingerprint: Option<String>,
    pub tool_version: String,
    /// The environment the tool is running in *now* (REQ049), so a stored verdict produced
    /// somewhere else can be seen for what it is.
    pub environment: crate::proving_env::ProvingEnv,
    /// The fingerprint of the model's out-of-subject specs *now* (#120), or `None` when the subject
    /// has none. A spec outside the subject tree is not covered by the subject's commit, so this is
    /// the only axis that can see it move.
    pub spec_fingerprint: Option<String>,
    /// The fingerprint of the monitor's declared trace *now* (#230), or `None` when the subject has
    /// no monitor configured or its trace cannot be read. Same reason as `spec_fingerprint`: the
    /// subject's commit does not cover a log the subject produced, so this is the only axis that
    /// can see it move — and a trace moves far more often than a spec does.
    pub trace_fingerprint: Option<String>,
    /// The fingerprint of the subject's declared UI check *now* (#239), or `None` when the subject
    /// has no category-3 check configured. The odd one out: a running deployment has no bytes to
    /// hash, so this covers the **declaration** — it sees the check change and is blind to the
    /// deployment moving underneath it at the same URL.
    pub ui_fingerprint: Option<String>,
}

/// The out-of-commit fingerprints an anchor is built from — the axes that exist precisely because
/// the subject's commit does not cover what they point at.
///
/// Grouped rather than passed as three positional `Option<String>`s, which is a silent swap waiting
/// to happen: mixing up the spec and trace axes compiles cleanly and mislabels every drift the
/// operator is later shown.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Fingerprints {
    pub spec: Option<String>,
    pub trace: Option<String>,
    pub ui: Option<String>,
    /// The subject's source fingerprint (#271) — in-commit, unlike its siblings, but grouped here
    /// for the same reason they are: every anchor axis read in one place, one silent-swap surface.
    pub source: Option<String>,
}

impl DriftAnchor {
    /// The anchor for the current world: this build's version, the caller-supplied subject HEAD
    /// (best-effort — `None` when the subject is not a git repo, never fabricated), and the
    /// caller-probed environment and fingerprints (passed in, so this module stays
    /// filesystem-free).
    pub fn current(
        subject_commit: Option<String>,
        environment: crate::proving_env::ProvingEnv,
        fingerprints: Fingerprints,
    ) -> Self {
        Self {
            subject_commit,
            source_fingerprint: fingerprints.source,
            tool_version: env!("CARGO_PKG_VERSION").to_string(),
            environment,
            spec_fingerprint: fingerprints.spec,
            trace_fingerprint: fingerprints.trace,
            ui_fingerprint: fingerprints.ui,
        }
    }
}

/// A stored verdict paired with whether it still holds against the current world — the living-loop
/// surface. Carries the verdict's own labels plus the freshness verdict and, when stale, the
/// concrete reasons the operator must re-verify.
///
/// It carries the verdict's **grounds** too — the same `detail`, `witness`, and per-engine
/// `evidence` a just-run verdict shows (#218). This surface used to carry the labels alone, which
/// meant the model a category-2a verdict was checked under, and the counterexample behind a
/// `refuted`, were visible only to whoever happened to press Verify, in the session they pressed it.
/// Every later reader — the normal case, and the reason a verdict is stored at all — got the
/// conclusion with the grounds removed. A conclusion is not a verdict without them.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct VerdictView {
    pub status: String,
    pub basis: Option<String>,
    pub reason: Option<String>,
    /// The verdict's own lines — the aggregate reading, and for category 2a the model it was
    /// checked under (#121, #211). The same strings the CLI prints, so the two surfaces cannot
    /// word the same verdict differently.
    pub detail: Vec<String>,
    /// The counterexample behind a `refuted` verdict, when the engine produced one. The single
    /// most actionable thing on the record: a stored `refuted` without it names a failure and
    /// leaves the operator nothing to act on.
    pub witness: Option<String>,
    /// The per-engine breakdown (D2b) — which engine reached what, on what basis. An aggregate
    /// verdict from an ensemble says less than the ensemble did.
    pub evidence: Vec<crate::verdict::EvidenceReport>,
    /// The verdict-level correspondence (Phase 4c) — `"asserted"` when the verdict rests only on a
    /// human-tagged test, so no surface can read it as a mechanical result.
    #[serde(default)]
    pub correspondence: String,
    /// The verdict is still anchored to the current world (nothing it depended on moved).
    pub fresh: bool,
    /// When not fresh, the concrete drifts — prose moved, code moved, tool changed — so the
    /// operator sees *why* a re-verify is owed, never just that one is.
    pub stale_reasons: Vec<String>,
    /// Where this verdict was proved (REQ049), rendered for display — `None` when the verdict
    /// predates the environment axis and no environment was ever recorded.
    ///
    /// Carried separately from `fresh` on purpose: "proved in `lab-2`, still the current
    /// environment" is a *checked* claim, while "we do not know where this was proved" is an
    /// unchecked one. Both leave `fresh` true, so without this the surface would render them
    /// identically and the stronger reading would win by default.
    pub environment: Option<String>,
}

/// The code-drift axis outcome (REQ071) — payload-free so a bounded Kani harness can prove the rule
/// against the real decision. `view` renders each variant's stale message.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CodeDrift {
    /// No code drift on this axis.
    Fresh,
    /// Both sides carry a source fingerprint and they differ — the fingerprinted source moved.
    SourceMoved,
    /// No fingerprint to compare, and the subject commit moved.
    CommitMoved,
    /// No fingerprint, and the stored commit can no longer be read.
    CommitUnreadable,
    /// No fingerprint, and the subject is a git repo now but was not when the verdict was stored.
    NowGitRepo,
}

/// The code-drift decision (REQ071): when both sides carry a source fingerprint, drift is decided by
/// the fingerprint alone — the commit is *not* consulted, so landing the verdict record (which moves
/// the commit but not the fingerprinted source) is not drift. Only when a fingerprint is absent does
/// it fall back to the commit comparison, so freshness is never widened on a basis the stored verdict
/// does not carry. Generic over what a fingerprint/commit *is* because the decision reads only their
/// equality and presence; the proof instantiates it at `u8` tokens, `view` at `&str`.
pub fn code_drift<T: PartialEq>(
    stored_fp: Option<T>,
    cur_fp: Option<T>,
    stored_commit: Option<T>,
    cur_commit: Option<T>,
) -> CodeDrift {
    match (stored_fp, cur_fp) {
        (Some(was), Some(now)) => {
            if was != now {
                CodeDrift::SourceMoved
            } else {
                CodeDrift::Fresh
            }
        }
        _ => match (stored_commit, cur_commit) {
            (Some(was), Some(now)) if was != now => CodeDrift::CommitMoved,
            (Some(_), None) => CodeDrift::CommitUnreadable,
            (None, Some(_)) => CodeDrift::NowGitRepo,
            _ => CodeDrift::Fresh,
        },
    }
}

/// The code-drift axis inputs, distilled to what the decision reads: the stored and current source
/// fingerprints and subject commits, modelled by `u8` tokens because [`code_drift`] reads only their
/// equality and presence. The sort REQ071's category-1 harness ranges over.
#[cfg(any(kani, creusot, prusti))]
pub struct DriftInputs {
    pub stored_fp: Option<u8>,
    pub cur_fp: Option<u8>,
    pub stored_commit: Option<u8>,
    pub cur_commit: Option<u8>,
}

#[cfg(kani)]
impl kani::Arbitrary for DriftInputs {
    fn any() -> Self {
        DriftInputs {
            stored_fp: kani::any(),
            cur_fp: kani::any(),
            stored_commit: kani::any(),
            cur_commit: kani::any(),
        }
    }
}

/// The commit-only fallback rule REQ071 keeps for a verdict with no source fingerprint.
#[cfg(any(kani, creusot, prusti))]
fn commit_rule(stored: Option<u8>, cur: Option<u8>) -> CodeDrift {
    match (stored, cur) {
        (Some(was), Some(now)) if was != now => CodeDrift::CommitMoved,
        (Some(_), None) => CodeDrift::CommitUnreadable,
        (None, Some(_)) => CodeDrift::NowGitRepo,
        _ => CodeDrift::Fresh,
    }
}

/// REQ071 stated as a property and checked against the real [`code_drift`]: when both sides carry a
/// source fingerprint, the outcome is decided by the fingerprint alone (`SourceMoved` iff they
/// differ, else `Fresh`) — written without reading either commit, so proving it for every input
/// establishes that the commit cannot change the outcome there (the axis never widens). Otherwise the
/// outcome is the commit-only fallback. The harness ranges `i` over every bounded input, so a pass
/// means the rule holds over the whole (bounded) space, not a spot check.
#[cfg(any(kani, creusot, prusti))]
pub fn code_drift_is_sound(i: &DriftInputs) -> bool {
    let got = code_drift(i.stored_fp, i.cur_fp, i.stored_commit, i.cur_commit);
    match (i.stored_fp, i.cur_fp) {
        (Some(was), Some(now)) => {
            got == if was != now {
                CodeDrift::SourceMoved
            } else {
                CodeDrift::Fresh
            }
        }
        _ => got == commit_rule(i.stored_commit, i.cur_commit),
    }
}

/// Pair a stored verdict with its freshness against the current world. Pure over the stored
/// verdict, the item's current revision, the current admitted formalization fingerprint, and the
/// [`DriftAnchor`] — no filesystem, so it is testable without a subject. A verdict is stale when any
/// provenance axis it was produced against has moved.
///
/// `current_formalization` is the fingerprint of the item's *currently admitted* formal input
/// ([`crate::draft::admitted_fingerprint`]) — `None` when nothing is admitted for it right now
/// (edited back to pending, un-admitted, or discarded), which is itself drift for a stored verdict.
pub fn view(
    stored: &VerdictReport,
    current_revision: &str,
    current_formalization: Option<&str>,
    anchor: &DriftAnchor,
) -> VerdictView {
    let mut stale_reasons = Vec::new();

    if stored.provenance.requirement_revision != current_revision {
        stale_reasons.push(
            "the requirement prose moved since this verdict — re-verify against the current text"
                .to_string(),
        );
    }

    // Formalization drift (REQ045): the verdict is only about the exact candidate + bindings it was
    // produced against. A verdict from before this axis existed (`formalization: None`) can't be
    // checked, so it is left alone — never flagged on a basis we cannot establish.
    if let Some(was) = &stored.provenance.formalization {
        match current_formalization {
            Some(now) if now != was => stale_reasons.push(
                "the formalization changed since this verdict (the candidate PRL or its bindings \
                 moved) — re-verify against the current draft"
                    .to_string(),
            ),
            None => stale_reasons.push(
                "the formalization this verdict checked is no longer admitted (edited, \
                 un-admitted, or discarded) — re-verify"
                    .to_string(),
            ),
            _ => {}
        }
    }

    // UI-check drift (#239): a category-3 verdict is `not-falsified` over one run of one
    // deployment, driven by exactly the steps the operator declared. Change a selector or point
    // `base_url` somewhere else and the verdict is about a check that no longer exists. Same rule
    // as every axis above — a verdict carrying no UI fingerprint is left alone rather than flagged
    // on a basis we cannot establish.
    if let Some(was) = &stored.provenance.ui_fingerprint {
        match &anchor.ui_fingerprint {
            Some(now) if now != was => stale_reasons.push(
                "the declared UI check moved since this verdict — a driver only ever ran the steps \
                 as they were then, so re-verify against the current check"
                    .to_string(),
            ),
            None => stale_reasons.push(
                "this verdict was reached against a UI check that is no longer declared (`ui` in \
                 provreq.yml) — re-verify"
                    .to_string(),
            ),
            _ => {}
        }
    }

    // Code drift (REQ071, #271): when both sides carry a source fingerprint, it decides this axis.
    // The commit is a coarser clock than the code — the commit that lands `verdicts.yml` itself
    // moves it, so under bare commit comparison a subject keeping its record in-tree could never
    // hold a fresh verdict. A verdict from before this axis keeps the commit comparison instead:
    // freshness is never *widened* on a basis the stored verdict does not carry. The decision is
    // the payload-free `code_drift` (REQ071), proved to ignore the commit whenever both fingerprints
    // are present; `view` only renders the chosen axis's message.
    match code_drift(
        stored.provenance.source_fingerprint.as_deref(),
        anchor.source_fingerprint.as_deref(),
        stored.provenance.subject_commit.as_deref(),
        anchor.subject_commit.as_deref(),
    ) {
        CodeDrift::Fresh => {}
        CodeDrift::SourceMoved => stale_reasons
            .push("the subject's source moved since this verdict — re-verify".to_string()),
        CodeDrift::CommitMoved => stale_reasons.push(format!(
            "the subject code moved since this verdict (commit {} → {}) — re-verify",
            stored.provenance.subject_commit.as_deref().unwrap_or(""),
            anchor.subject_commit.as_deref().unwrap_or(""),
        )),
        CodeDrift::CommitUnreadable => stale_reasons.push(
            "the subject's commit can no longer be read to confirm this verdict — re-verify"
                .to_string(),
        ),
        CodeDrift::NowGitRepo => stale_reasons.push(
            "the subject is now a git repo; this verdict predates its history — re-verify"
                .to_string(),
        ),
    }

    // Environment drift (REQ049): a verdict is only about the environment that produced it. A
    // verdict from before this axis existed (`environment: None`) carries no environment to
    // compare, so it is left alone — the same rule as formalization above: never flag on a basis
    // we cannot establish.
    if let Some(was) = &stored.provenance.environment
        && let Some(reason) = was.drift_from(&anchor.environment)
    {
        stale_reasons.push(reason);
    }

    // Spec drift (#120): a model that lives outside the subject moves without the subject's commit
    // moving, so this is the only axis that can see it. Same rule as formalization and environment
    // above — a verdict carrying no fingerprint (from before this axis, or from a subject whose
    // specs are all in-tree) is left alone rather than flagged on a basis we cannot establish.
    if let Some(was) = &stored.provenance.spec_fingerprint {
        match &anchor.spec_fingerprint {
            Some(now) if now != was => stale_reasons.push(
                "the TLA+ specs outside the subject moved since this verdict — the subject's \
                 commit does not cover them, so re-verify against the current model"
                    .to_string(),
            ),
            None => stale_reasons.push(
                "this verdict was proved against TLA+ specs outside the subject, and none are \
                 configured now (`tla.spec_paths` in provreq.yml) — re-verify"
                    .to_string(),
            ),
            _ => {}
        }
    }

    // Trace drift (#230): a monitor's evidence is the log it read, and a log grows. The subject's
    // commit does not cover a file the subject produced, so a `not-falsified` verdict would
    // otherwise read `fresh` against a trace that has since recorded the very violation it says it
    // did not see. Same rule as every axis above — a verdict carrying no trace fingerprint is left
    // alone rather than flagged on a basis we cannot establish.
    if let Some(was) = &stored.provenance.trace_fingerprint {
        match &anchor.trace_fingerprint {
            Some(now) if now != was => stale_reasons.push(
                "the monitored trace moved since this verdict — a monitor only ever saw the log as \
                 it was then, so re-verify against the current trace"
                    .to_string(),
            ),
            None => stale_reasons.push(
                "this verdict was reached against a monitored trace that can no longer be read \
                 (`monitor.trace` in provreq.yml) — re-verify"
                    .to_string(),
            ),
            _ => {}
        }
    }

    if stored.provenance.tool_version != anchor.tool_version {
        stale_reasons.push(format!(
            "the tool changed since this verdict (provreq {} → {}) — re-verify",
            stored.provenance.tool_version, anchor.tool_version
        ));
    }

    VerdictView {
        status: stored.status.clone(),
        basis: stored.basis.clone(),
        reason: stored.reason.clone(),
        detail: stored.detail.clone(),
        witness: stored.witness.clone(),
        evidence: stored.evidence.clone(),
        correspondence: stored.correspondence.clone(),
        fresh: stale_reasons.is_empty(),
        stale_reasons,
        environment: stored.provenance.environment.as_ref().map(|e| e.describe()),
    }
}

/// Whether a stored verdict can be served without re-running the ensemble (REQ091): true when a
/// fresh run would reach the identical verdict, because every axis that *determines* the verdict is
/// unchanged. Pure over the two provenances — no filesystem — so the incremental `verify` path is
/// testable without a subject.
///
/// This is deliberately a *stricter and narrower* comparison than [`view`]'s display-staleness:
///
/// - The **code axis is `scope_fingerprint`** (a working-tree hash), not the HEAD `source_fingerprint`
///   `view` uses. A cache that skips engine runs must see an uncommitted edit; display staleness is
///   allowed its documented HEAD blind spot, a served verdict is not. Both sides must carry a scope
///   fingerprint and they must match — a verdict with none (persisted before this axis, or code that
///   could not be read) is never a hit, so the ensemble runs.
/// - The **subject commit and HEAD `source_fingerprint` are ignored**: both move when the verdict
///   record itself is committed, and the working-tree scope hash already decides the code axis.
/// - The **environment is compared by its engine set only**, not its declared name: *where* a verdict
///   was proved does not change *what* was proved, but a different engine or version can.
///
/// Every other determining axis (requirement revision, formalization, out-of-subject spec/trace/UI,
/// tool version) must match exactly. `tagged_source_fingerprint` is compared too (#508): under
/// `verify.crate` narrowing a tagged test can live outside the scoped crate, so `scope_fingerprint`
/// does not always cover it — a plain `==` catches an edited or removed tagged test while leaving a
/// formal-only verdict (no tagged source on either side, both `None`) cacheable.
pub fn reusable(stored: &VerdictReport, current: &crate::verdict::ProvenanceReport) -> bool {
    let s = &stored.provenance;
    // The code axis: both sides must carry a working-tree scope hash and agree. A missing hash on
    // either side (pre-axis record, or code that could not be read now) forces a re-run — never a
    // hit on a basis that cannot be established, the same rule every drift axis in `view` follows.
    let scope_match = matches!(
        (&s.scope_fingerprint, &current.scope_fingerprint),
        (Some(was), Some(now)) if was == now
    );
    let engines =
        |p: &crate::verdict::ProvenanceReport| p.environment.as_ref().map(|e| e.engines.clone());

    scope_match
        && s.requirement_revision == current.requirement_revision
        && s.formalization == current.formalization
        && s.spec_fingerprint == current.spec_fingerprint
        && s.trace_fingerprint == current.trace_fingerprint
        && s.ui_fingerprint == current.ui_fingerprint
        && s.tagged_source_fingerprint == current.tagged_source_fingerprint
        && s.tool_version == current.tool_version
        && engines(s) == engines(current)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::verdict::{ProvenanceReport, VerdictReport};

    fn stored(revision: &str, commit: Option<&str>, tool: &str) -> VerdictReport {
        VerdictReport {
            id: "REQ001".into(),
            status: "holds".into(),
            basis: Some("proven".into()),
            reason: None,
            witness: None,
            detail: vec![],
            evidence: vec![],
            correspondence: "mechanical".into(),
            provenance: ProvenanceReport {
                verified_scope: None,
                spec_fingerprint: None,
                trace_fingerprint: None,
                ui_fingerprint: None,
                tagged_source_fingerprint: None,
                scope_fingerprint: None,
                environment: None,
                requirement_revision: revision.into(),
                subject_commit: commit.map(str::to_string),
                tool_version: tool.into(),
                source_fingerprint: None,
                formalization: None,
            },
        }
    }

    // Verifies: #218 — a stored verdict is served with its grounds, not just its labels. The model
    // a category-2a verdict was checked under and the counterexample behind a refutation live on
    // the stored record; a view that dropped them let the browser show a conclusion whose basis was
    // visible only in the session that produced it.
    #[test]
    fn a_stored_verdict_carries_the_grounds_it_was_reached_on() {
        let mut v = stored("r1", Some("abc"), "0.0.1");
        v.detail = vec!["checked under the model — Drones = {d1, d2}, MaxAlt = 2".into()];
        v.witness = Some("state 2: alt = [d1 |-> 1]".into());
        v.evidence = vec![crate::verdict::EvidenceReport {
            engine: "TLC (TLA+)".into(),
            status: "holds".into(),
            basis: Some("model-checked (bounded)".into()),
            witness: None,
            correspondence: "mechanical".into(),
            source_location: None,
            detail: vec!["checked under the model — Drones = {d1, d2}, MaxAlt = 2".into()],
        }];
        let anchor = DriftAnchor {
            spec_fingerprint: None,
            trace_fingerprint: None,
            ui_fingerprint: None,
            environment: env(None, &[]),
            subject_commit: Some("abc".into()),
            tool_version: "0.0.1".into(),
            source_fingerprint: None,
        };

        let view = view(&v, "r1", None, &anchor);

        assert!(
            view.fresh,
            "the fixture drifted nothing: {:?}",
            view.stale_reasons
        );
        assert_eq!(view.detail, v.detail, "the verdict's own lines are dropped");
        assert_eq!(view.witness, v.witness, "the counterexample is dropped");
        assert_eq!(
            view.evidence, v.evidence,
            "the per-engine breakdown is dropped"
        );
    }

    fn env(declared: Option<&str>, engines: &[&str]) -> crate::proving_env::ProvingEnv {
        crate::proving_env::ProvingEnv {
            declared: declared.map(str::to_string),
            engines: engines.iter().map(|e| e.to_string()).collect(),
            unversioned: vec![],
            container: false,
        }
    }

    // Verifies: REQ049 — a verdict produced in one environment and read in another is stale, with
    // the environment named. Without this axis the two are indistinguishable in the store.
    #[test]
    fn a_verdict_proved_elsewhere_is_stale() {
        let mut v = stored("r1", Some("abc"), "0.0.1");
        v.provenance.environment = Some(env(Some("lab-1"), &["Kani 0.67.0"]));
        let anchor = DriftAnchor {
            spec_fingerprint: None,
            trace_fingerprint: None,
            ui_fingerprint: None,
            environment: env(Some("ci-runner"), &["Kani 0.67.0"]),
            subject_commit: Some("abc".into()),
            tool_version: "0.0.1".into(),
            source_fingerprint: None,
        };

        let view = view(&v, "r1", None, &anchor);
        assert!(!view.fresh);
        assert_eq!(view.stale_reasons.len(), 1, "{:?}", view.stale_reasons);
        assert!(view.stale_reasons[0].contains("lab-1"));
        assert!(view.stale_reasons[0].contains("ci-runner"));
    }

    // Verifies: REQ028 (#120) — a TLA+ spec outside the subject moving makes the verdict stale.
    // The subject's commit is deliberately UNCHANGED here, because that is the whole point: no
    // other axis can see an out-of-subject file move, so without this the verdict reads fresh.
    #[test]
    fn an_external_spec_moving_makes_a_verdict_stale() {
        let mut v = stored("r1", Some("abc"), "0.0.1");
        v.provenance.spec_fingerprint = Some("aaaa".into());
        let anchor = DriftAnchor {
            spec_fingerprint: Some("bbbb".into()),
            trace_fingerprint: None,
            ui_fingerprint: None,
            environment: crate::proving_env::ProvingEnv::default(),
            subject_commit: Some("abc".into()),
            tool_version: "0.0.1".into(),
            source_fingerprint: None,
        };

        let view = view(&v, "r1", None, &anchor);
        assert!(!view.fresh, "the model moved under this verdict");
        assert_eq!(view.stale_reasons.len(), 1, "{:?}", view.stale_reasons);
        assert!(
            view.stale_reasons[0].contains("outside the subject"),
            "{:?}",
            view.stale_reasons
        );
    }

    // Verifies: REQ028 (#120) — a verdict carrying no spec fingerprint is left alone, whether it
    // predates the axis or came from an in-tree subject. Same rule as every other axis: never flag
    // on a basis we cannot establish.
    #[test]
    fn a_verdict_without_a_spec_fingerprint_is_not_flagged() {
        let v = stored("r1", Some("abc"), "0.0.1");
        assert_eq!(v.provenance.spec_fingerprint, None);
        let anchor = DriftAnchor {
            spec_fingerprint: Some("bbbb".into()),
            trace_fingerprint: None,
            ui_fingerprint: None,
            environment: crate::proving_env::ProvingEnv::default(),
            subject_commit: Some("abc".into()),
            tool_version: "0.0.1".into(),
            source_fingerprint: None,
        };
        assert!(view(&v, "r1", None, &anchor).fresh);
    }

    // Verifies: REQ028 (#120) — a verdict proved against external specs, read back where none are
    // configured, is stale. The model it was checked against is not merely different, it is out of
    // reach, so the verdict cannot be confirmed.
    #[test]
    fn losing_the_configured_specs_makes_a_verdict_stale() {
        let mut v = stored("r1", Some("abc"), "0.0.1");
        v.provenance.spec_fingerprint = Some("aaaa".into());
        let anchor = DriftAnchor {
            spec_fingerprint: None,
            trace_fingerprint: None,
            ui_fingerprint: None,
            environment: crate::proving_env::ProvingEnv::default(),
            subject_commit: Some("abc".into()),
            tool_version: "0.0.1".into(),
            source_fingerprint: None,
        };
        let view = view(&v, "r1", None, &anchor);
        assert!(!view.fresh);
        assert!(
            view.stale_reasons[0].contains("spec_paths"),
            "{:?}",
            view.stale_reasons
        );
    }

    // Verifies: #230 — the monitored trace is a drift axis of its own, and the one with the fastest
    // clock. A log the subject keeps writing is not covered by the subject's commit, so without
    // this a `not-falsified` verdict would read `fresh` against a trace that has since recorded the
    // very violation it says it did not see.
    #[test]
    fn a_trace_that_moved_makes_the_verdict_stale() {
        let mut v = stored("r1", Some("abc"), "0.0.1");
        v.provenance.trace_fingerprint = Some("aaaa".into());
        let fresh_anchor = |trace: Option<&str>| DriftAnchor {
            spec_fingerprint: None,
            trace_fingerprint: trace.map(str::to_string),
            ui_fingerprint: None,
            environment: crate::proving_env::ProvingEnv::default(),
            subject_commit: Some("abc".into()),
            tool_version: "0.0.1".into(),
            source_fingerprint: None,
        };

        assert!(view(&v, "r1", None, &fresh_anchor(Some("aaaa"))).fresh);

        let moved = view(&v, "r1", None, &fresh_anchor(Some("bbbb")));
        assert!(!moved.fresh, "the log grew under this verdict");
        assert!(
            moved.stale_reasons[0].contains("monitored trace moved"),
            "{:?}",
            moved.stale_reasons
        );

        // The trace becoming unreadable is drift too: the evidence is not merely different, it is
        // out of reach, so the verdict cannot be confirmed.
        let gone = view(&v, "r1", None, &fresh_anchor(None));
        assert!(!gone.fresh);
        assert!(
            gone.stale_reasons[0].contains("monitor.trace"),
            "{:?}",
            gone.stale_reasons
        );
    }

    // Verifies: #239 — the UI axis. A category-3 verdict is `not-falsified` over one run driven by
    // exactly the declared steps, so editing a selector or repointing `base_url` leaves a verdict
    // about a check that no longer exists. This is the axis that catches it; the subject's commit
    // cannot, because the check lives in the companion manifest and the deployment lives nowhere in
    // the repo at all.
    #[test]
    fn a_ui_check_that_moved_makes_the_verdict_stale() {
        let mut v = stored("r1", Some("abc"), "0.0.1");
        v.provenance.ui_fingerprint = Some("aaaa".into());
        let fresh_anchor = |ui: Option<&str>| DriftAnchor {
            spec_fingerprint: None,
            trace_fingerprint: None,
            ui_fingerprint: ui.map(str::to_string),
            environment: crate::proving_env::ProvingEnv::default(),
            subject_commit: Some("abc".into()),
            tool_version: "0.0.1".into(),
            source_fingerprint: None,
        };

        assert!(view(&v, "r1", None, &fresh_anchor(Some("aaaa"))).fresh);

        let moved = view(&v, "r1", None, &fresh_anchor(Some("bbbb")));
        assert!(!moved.fresh, "the steps changed under this verdict");
        assert!(
            moved.stale_reasons[0].contains("declared UI check moved"),
            "{:?}",
            moved.stale_reasons
        );

        let gone = view(&v, "r1", None, &fresh_anchor(None));
        assert!(!gone.fresh, "the check being removed is drift too");
        assert!(
            gone.stale_reasons[0].contains("no longer declared"),
            "{:?}",
            gone.stale_reasons
        );

        // And a verdict from before this axis existed is left alone rather than flagged on a basis
        // that cannot be established — the same rule every other axis follows.
        let older = stored("r1", Some("abc"), "0.0.1");
        assert!(view(&older, "r1", None, &fresh_anchor(Some("bbbb"))).fresh);
    }

    // Verifies: #230 — a verdict carrying no trace fingerprint is left alone, whether it predates
    // the axis or came from a subject with no monitor. Same rule as every other axis: never flag on
    // a basis we cannot establish. Every category-1 and 2a verdict in existence is this case.
    #[test]
    fn a_verdict_without_a_trace_fingerprint_is_not_flagged() {
        let v = stored("r1", Some("abc"), "0.0.1");
        assert_eq!(v.provenance.trace_fingerprint, None);
        let anchor = DriftAnchor {
            spec_fingerprint: None,
            trace_fingerprint: Some("bbbb".into()),
            ui_fingerprint: None,
            environment: crate::proving_env::ProvingEnv::default(),
            subject_commit: Some("abc".into()),
            tool_version: "0.0.1".into(),
            source_fingerprint: None,
        };
        assert!(view(&v, "r1", None, &anchor).fresh);
    }

    // Verifies: REQ050 — the view distinguishes "proved here, unchanged" from "never recorded".
    // Both are `fresh`, so without a separate field the surface would render them identically and
    // an operator would read a guarantee the record does not carry.
    #[test]
    fn a_recorded_environment_is_distinguishable_from_an_unrecorded_one() {
        let anchor = DriftAnchor {
            spec_fingerprint: None,
            trace_fingerprint: None,
            ui_fingerprint: None,
            environment: env(Some("lab-1"), &["Kani 0.67.0"]),
            subject_commit: Some("abc".into()),
            tool_version: "0.0.1".into(),
            source_fingerprint: None,
        };

        let mut recorded = stored("r1", Some("abc"), "0.0.1");
        recorded.provenance.environment = Some(env(Some("lab-1"), &["Kani 0.67.0"]));
        let recorded = view(&recorded, "r1", None, &anchor);

        let never = view(&stored("r1", Some("abc"), "0.0.1"), "r1", None, &anchor);

        assert!(recorded.fresh && never.fresh, "both are fresh");
        assert!(
            recorded.environment.expect("recorded").contains("lab-1"),
            "a recorded environment names where the verdict was proved"
        );
        assert_eq!(
            never.environment, None,
            "a verdict that never recorded one must not look like one that did"
        );
    }

    // Verifies: REQ049 — a verdict persisted before this axis existed carries no environment, so
    // it is left alone rather than flagged on a basis that cannot be established. Same rule as the
    // formalization axis; upgrading provreq must not mark every historical verdict stale.
    #[test]
    fn a_verdict_predating_the_environment_axis_is_not_flagged() {
        let v = stored("r1", Some("abc"), "0.0.1");
        assert!(v.provenance.environment.is_none(), "the pre-axis shape");
        let anchor = DriftAnchor {
            spec_fingerprint: None,
            trace_fingerprint: None,
            ui_fingerprint: None,
            environment: env(Some("lab-1"), &["Kani 0.67.0"]),
            subject_commit: Some("abc".into()),
            tool_version: "0.0.1".into(),
            source_fingerprint: None,
        };

        let view = view(&v, "r1", None, &anchor);
        assert!(view.fresh, "{:?}", view.stale_reasons);
    }

    // Verifies: REQ071 / #271 — when both the stored verdict and the anchor carry a source
    // fingerprint, it decides the code-drift axis: a commit that changes none of the fingerprinted
    // source — most immediately, the commit that lands `verdicts.yml` itself — is not code drift,
    // however far HEAD has moved. Under the bare commit rule a subject that keeps its verdict
    // record in-tree could never hold a fresh verdict, because storing the answer moved the clock.
    #[test]
    fn a_companion_only_commit_is_not_code_drift() {
        let mut v = stored("r1", Some("abc"), "0.0.1");
        v.provenance.source_fingerprint = Some("src-1".into());
        let anchor = DriftAnchor {
            spec_fingerprint: None,
            trace_fingerprint: None,
            ui_fingerprint: None,
            environment: crate::proving_env::ProvingEnv::default(),
            subject_commit: Some("def".into()),
            tool_version: "0.0.1".into(),
            source_fingerprint: Some("src-1".into()),
        };
        let view = view(&v, "r1", None, &anchor);
        assert!(view.fresh, "{:?}", view.stale_reasons);
    }

    // Verifies: REQ071 / #271 — the fingerprint deciding the axis cuts both ways: when the source
    // itself moved, the verdict is stale and the reason names the source, not merely the commit.
    #[test]
    fn source_movement_is_code_drift() {
        let mut v = stored("r1", Some("abc"), "0.0.1");
        v.provenance.source_fingerprint = Some("src-1".into());
        let anchor = DriftAnchor {
            spec_fingerprint: None,
            trace_fingerprint: None,
            ui_fingerprint: None,
            environment: crate::proving_env::ProvingEnv::default(),
            subject_commit: Some("def".into()),
            tool_version: "0.0.1".into(),
            source_fingerprint: Some("src-2".into()),
        };
        let view = view(&v, "r1", None, &anchor);
        assert!(!view.fresh);
        assert!(
            view.stale_reasons[0].contains("source moved"),
            "{:?}",
            view.stale_reasons
        );
    }

    // Verifies: REQ071 / #271 — freshness is never widened on a basis the stored verdict does not
    // carry: a verdict from before this axis keeps the commit comparison, so upgrading provreq
    // does not quietly mark historical verdicts fresher than their own record can establish.
    #[test]
    fn a_verdict_without_a_source_fingerprint_keeps_the_commit_rule() {
        let v = stored("r1", Some("abc"), "0.0.1");
        assert_eq!(v.provenance.source_fingerprint, None, "the pre-axis shape");
        let anchor = DriftAnchor {
            spec_fingerprint: None,
            trace_fingerprint: None,
            ui_fingerprint: None,
            environment: crate::proving_env::ProvingEnv::default(),
            subject_commit: Some("def".into()),
            tool_version: "0.0.1".into(),
            source_fingerprint: Some("src-1".into()),
        };
        let view = view(&v, "r1", None, &anchor);
        assert!(!view.fresh);
        assert!(
            view.stale_reasons[0].contains("commit"),
            "{:?}",
            view.stale_reasons
        );
    }

    // Verifies: REQ039 — a verdict produced against the current world is fresh, with no reasons.
    #[test]
    fn unmoved_verdict_is_fresh() {
        let v = stored("r1", Some("abc"), "0.0.1");
        let anchor = DriftAnchor {
            spec_fingerprint: None,
            trace_fingerprint: None,
            ui_fingerprint: None,
            environment: crate::proving_env::ProvingEnv::default(),
            subject_commit: Some("abc".into()),
            tool_version: "0.0.1".into(),
            source_fingerprint: None,
        };
        let view = view(&v, "r1", None, &anchor);
        assert!(view.fresh);
        assert!(view.stale_reasons.is_empty());
    }

    // Verifies: REQ039 — each provenance axis that moves is an independent, named staleness reason;
    // several can drift at once.
    #[test]
    fn each_moved_axis_is_a_named_reason() {
        let v = stored("r1", Some("abc"), "0.0.1");
        let anchor = DriftAnchor {
            spec_fingerprint: None,
            trace_fingerprint: None,
            ui_fingerprint: None,
            environment: crate::proving_env::ProvingEnv::default(),
            subject_commit: Some("def".into()),
            tool_version: "0.0.2".into(),
            source_fingerprint: None,
        };
        let view = view(&v, "r2", None, &anchor);
        assert!(!view.fresh);
        assert_eq!(view.stale_reasons.len(), 3, "prose + code + tool all moved");
        assert!(view.stale_reasons.iter().any(|r| r.contains("prose moved")));
        assert!(view.stale_reasons.iter().any(|r| r.contains("abc → def")));
        assert!(
            view.stale_reasons
                .iter()
                .any(|r| r.contains("0.0.1 → 0.0.2"))
        );
    }

    /// A verdict pinned to a formalization fingerprint (REQ045), everything else current.
    fn formalized(fingerprint: &str) -> VerdictReport {
        let mut v = stored("r1", Some("abc"), "0.0.1");
        v.provenance.formalization = Some(fingerprint.into());
        v
    }

    fn fresh_anchor() -> DriftAnchor {
        DriftAnchor {
            spec_fingerprint: None,
            trace_fingerprint: None,
            ui_fingerprint: None,
            environment: crate::proving_env::ProvingEnv::default(),
            subject_commit: Some("abc".into()),
            tool_version: "0.0.1".into(),
            source_fingerprint: None,
        }
    }

    // Verifies: REQ045 — a verdict stays fresh only while its formalization fingerprint still
    // matches the currently admitted one; a changed fingerprint drifts it, on that axis alone.
    #[test]
    fn formalization_change_drifts_the_verdict() {
        let v = formalized("fp-1");
        // Same fingerprint still admitted → fresh (no other axis moved).
        let same = view(&v, "r1", Some("fp-1"), &fresh_anchor());
        assert!(
            same.fresh,
            "unchanged formalization is fresh: {:?}",
            same.stale_reasons
        );

        // The candidate or its bindings moved → a single formalization-drift reason.
        let changed = view(&v, "r1", Some("fp-2"), &fresh_anchor());
        assert!(!changed.fresh);
        assert_eq!(changed.stale_reasons.len(), 1);
        assert!(changed.stale_reasons[0].contains("formalization changed"));
    }

    // Verifies: REQ045 — a verdict whose formalization is no longer admitted (edited back to
    // pending, un-admitted, or discarded) is stale, naming that as the reason.
    #[test]
    fn no_admitted_formalization_drifts_the_verdict() {
        let v = formalized("fp-1");
        let orphaned = view(&v, "r1", None, &fresh_anchor());
        assert!(!orphaned.fresh);
        assert_eq!(orphaned.stale_reasons.len(), 1);
        assert!(orphaned.stale_reasons[0].contains("no longer admitted"));
    }

    // Verifies: REQ045 — a verdict from before this axis existed (`formalization: None`) is never
    // flagged on the formalization axis: drift is only claimed on a basis we can establish.
    #[test]
    fn pre_axis_verdict_is_not_flagged_on_formalization() {
        let v = stored("r1", Some("abc"), "0.0.1"); // formalization: None
        let view = view(&v, "r1", Some("fp-current"), &fresh_anchor());
        assert!(
            view.fresh,
            "a pre-REQ045 verdict skips the formalization axis"
        );
    }

    // Verifies: REQ039 — recording a verdict then loading round-trips it, and a re-verify replaces
    // rather than duplicates the item's entry.
    #[test]
    fn record_and_load_round_trip_replacing_prior() {
        let dir = tempfile::tempdir().unwrap();
        let first = record(&VerdictStore::new(), stored("r1", Some("abc"), "0.0.1"));
        save(dir.path(), &first).unwrap();

        let mut second_report = stored("r2", Some("def"), "0.0.1");
        second_report.status = "fails".into();
        let second = record(&load(dir.path()).unwrap(), second_report);
        save(dir.path(), &second).unwrap();

        let loaded = load(dir.path()).unwrap();
        assert_eq!(
            loaded.verdicts.len(),
            1,
            "re-verify replaces, never duplicates"
        );
        assert_eq!(loaded.verdicts["REQ001"].status, "fails");
        assert_eq!(
            loaded.verdicts["REQ001"].provenance.requirement_revision,
            "r2"
        );
    }

    // Verifies: REQ039 — a missing store file is an empty store (nothing verified yet), not an error.
    #[test]
    fn missing_file_is_empty_store() {
        let dir = tempfile::tempdir().unwrap();
        assert!(load(dir.path()).unwrap().verdicts.is_empty());
    }

    /// A stored verdict and the current provenance, both pinned to the same unchanged world —
    /// the baseline a cache hit is asserted against. Carries a working-tree scope fingerprint,
    /// since that is the code axis the incremental `verify` compares (REQ091).
    fn reusable_pair() -> (VerdictReport, ProvenanceReport) {
        let mut v = stored("r1", Some("abc"), "0.0.1");
        v.provenance.scope_fingerprint = Some("scope-1".into());
        v.provenance.tagged_source_fingerprint = Some("tagged-1".into());
        v.provenance.formalization = Some("fp-1".into());
        v.provenance.environment = Some(env(Some("lab-1"), &["Kani 0.67.0"]));
        (v.clone(), v.provenance)
    }

    // Verifies: REQ091 — nothing that determines the verdict moved, so the stored verdict is served
    // without re-running the ensemble.
    #[test]
    fn an_unchanged_world_is_a_cache_hit() {
        let (stored, current) = reusable_pair();
        assert!(reusable(&stored, &current));
    }

    // Verifies: REQ091 — a working-tree edit to the code the verdict covers moves `scope_fingerprint`,
    // so the verdict is re-run rather than served. This is the axis that keeps a cached verdict from
    // being served against code the working tree already contradicts.
    #[test]
    fn a_scope_edit_is_a_cache_miss() {
        let (stored, mut current) = reusable_pair();
        current.scope_fingerprint = Some("scope-2".into());
        assert!(!reusable(&stored, &current));
    }

    // Verifies: REQ091 — a verdict carrying no scope fingerprint (persisted before this axis, or code
    // that could not be read) is never a hit: the ensemble runs rather than trust a basis that cannot
    // be established, even when every other axis matches.
    #[test]
    fn a_missing_scope_fingerprint_is_never_a_hit() {
        let (mut stored, current) = reusable_pair();
        stored.provenance.scope_fingerprint = None;
        assert!(!reusable(&stored, &current));

        let (stored, mut current) = reusable_pair();
        current.scope_fingerprint = None;
        assert!(!reusable(&stored, &current));
    }

    // Verifies: REQ091 — each determining axis that moves is a miss: prose, formalization, engines,
    // tool version, and the out-of-subject spec/trace/UI axes.
    #[test]
    fn any_determining_axis_moving_is_a_miss() {
        let axes: Vec<fn(&mut ProvenanceReport)> = vec![
            |c| c.requirement_revision = "r2".into(),
            |c| c.formalization = Some("fp-2".into()),
            |c| c.tool_version = "0.0.2".into(),
            |c| c.environment = Some(env(Some("lab-1"), &["Kani 0.68.0"])),
            |c| c.spec_fingerprint = Some("spec-x".into()),
            |c| c.trace_fingerprint = Some("trace-x".into()),
            |c| c.ui_fingerprint = Some("ui-x".into()),
        ];
        for apply in axes {
            let (stored, mut current) = reusable_pair();
            apply(&mut current);
            assert!(!reusable(&stored, &current), "a moved axis must re-run");
        }
    }

    // Verifies: REQ091 / #508 — a change to a tagged test the asserted evidence rests on is a miss,
    // even when `scope_fingerprint` is unchanged. Under `verify.crate` narrowing a tagged test can
    // live outside the scoped crate, so the scope hash does not move when it changes; the cache must
    // not serve the old (passing) verdict a fresh run of that test could contradict.
    #[test]
    fn a_tagged_source_edit_is_a_cache_miss() {
        let (stored, mut current) = reusable_pair();
        current.tagged_source_fingerprint = Some("tagged-2".into());
        assert!(!reusable(&stored, &current));

        // A tagged test removed since the verdict (Some → None) is likewise a miss.
        let (stored, mut current) = reusable_pair();
        current.tagged_source_fingerprint = None;
        assert!(!reusable(&stored, &current));
    }

    // Verifies: REQ091 — a verdict with no tagged evidence on either side (both None) is unaffected
    // by this axis and stays cacheable: a formal-only verdict has no tagged source to drift.
    #[test]
    fn no_tagged_evidence_is_still_a_hit() {
        let (mut stored, mut current) = reusable_pair();
        stored.provenance.tagged_source_fingerprint = None;
        current.tagged_source_fingerprint = None;
        assert!(reusable(&stored, &current));
    }

    // Verifies: REQ091 — the axes that move merely because the verdict record was committed do not
    // force a re-run: the subject commit and the HEAD source fingerprint are ignored, and so is the
    // environment's declared *name* (where it was proved, not what was proved). The working-tree
    // scope hash is the code axis, so none of these is needed to keep a hit honest.
    #[test]
    fn record_keeping_churn_is_still_a_hit() {
        let (stored, mut current) = reusable_pair();
        current.subject_commit = Some("def".into());
        current.source_fingerprint = Some("head-moved".into());
        current.environment = Some(env(Some("ci-runner"), &["Kani 0.67.0"]));
        assert!(
            reusable(&stored, &current),
            "committing the record or re-proving in a renamed environment is not verdict drift"
        );
    }
}
