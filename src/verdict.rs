//! Step 4 — the verdict object (D7 three-valued evidence record + D9 provenance).
//!
//! A verdict is never a judgment — `status ∈ {holds, fails, unknown}` and an `unknown`
//! always carries a reason (D10). An ungrounded requirement is `unknown / missing-grounding`
//! (R-ground-1); a grounded one with no engine for its category is `unknown / no-engine`.
//!
//! **Engine-independent by design.** REQ027 wired Kani as category-1 engine #1, but D2's
//! rule is one core meaning lowered to each engine — so this module knows about *bases* and
//! *witnesses*, never about Kani. Each engine maps its own result into an [`Evidence`]
//! ([`crate::kani::Outcome::into_evidence`]) that the core aggregates ([`aggregate`]) — which
//! is what lets D2b's ensemble add engines with differing soundness directions without
//! touching the core.
//!
//! Implements: REQ023 (verdict object + provenance; honest unknown), REQ027 (a real
//! `holds`/`fails` from a wired engine, with a D8 basis and a D9 witness), REQ030 (aggregate
//! an ensemble of engines' [`Evidence`] into one verdict, soundness-aware, never a vote).

use crate::grounding::Grounding;
use std::path::PathBuf;

/// Whether an [`Evidence`]'s claim is **mechanical** or **asserted** (Phase 4b, REQ076) — the
/// honest core of the code-tag evidence source. `Mechanical` is sound by construction: provreq
/// generated the harness from the formal claim ([`crate::lowering`]), so a passing harness means
/// the requirement holds. `Asserted` is a human's claim: a `Verifies:` tag says a test checks the
/// requirement, and provreq can confirm that test runs and passes but **never** that passing it
/// *means* the requirement holds. An asserted `holds` must never read as strong as a mechanical
/// one — every surface that shows a verdict carries this so the two can never be confused.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Correspondence {
    Mechanical,
    Asserted,
}

impl Correspondence {
    pub fn as_str(&self) -> &'static str {
        match self {
            Correspondence::Mechanical => "mechanical",
            Correspondence::Asserted => "asserted",
        }
    }
}

/// Where an asserted [`Evidence`] came from — the tagged source the operator vouched for
/// (Phase 4b). Absent on mechanical evidence, whose provenance is the generated harness, not a
/// place in the subject's own tree.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct SourceLocation {
    pub file: PathBuf,
    pub line: usize,
    /// The declaration the tag annotated, when the scan could resolve one.
    pub symbol: Option<String>,
}

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
/// Only the rung an engine can actually earn is representable, so an engine cannot overclaim
/// by accident:
/// - Kani is a **bounded** model checker — it establishes the claim over the states it
///   explored, not over all executions — so it yields [`Basis::ModelCheckedBounded`] and
///   **cannot** yield `proven`.
/// - Creusot is a **deductive** verifier (REQ031) — a discharged proof obligation holds for
///   *all* executions, spec-relative — so it yields [`Basis::Proven`], the strongest rung.
/// - MonPoly is a **runtime monitor** (#222) — it reads a trace of what actually ran, so its
///   silence is [`Basis::NotFalsified`] and nothing stronger.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Basis {
    /// Deductive proof (Creusot): the property holds for every execution, relative to the
    /// spec — the strongest rung. Unlike a bounded checker, no state-space caveat applies.
    Proven,
    ModelCheckedBounded,
    /// Empirical (#229): executions that actually happened were observed, and no counterexample
    /// appeared among them. That is neither `proven` (∀ executions) nor `model-checked` (∀ over
    /// a model) — it is a statement about **what ran**, and only about what ran. The weakest
    /// evidence provreq deals in, and the only rung whose extent must travel with it: see
    /// [`Evidence::not_falsified`], which will not let an engine claim it bare.
    ///
    /// Two engines hold it and they observe differently: MonPoly reads a trace the subject wrote
    /// (#233), and a WebDriver session performs one run and watches the page (#245). The rung is
    /// the same; the extent that travels with it is not.
    NotFalsified,
}

impl Basis {
    pub fn as_str(&self) -> &'static str {
        match self {
            Basis::Proven => "proven",
            Basis::ModelCheckedBounded => "model-checked (bounded)",
            Basis::NotFalsified => "not-falsified",
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
/// adding the variant would be dead code. `assumption-unmet` arrives with D8 contingencies.
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
    /// Two engines in the ensemble reached **opposite** answers — one `holds`, another
    /// `fails` (D2b). This is never resolved by majority vote: a `fails` carries a
    /// re-checkable witness and a `holds` a soundness basis, so a conflict is a real
    /// modelling discrepancy for a human to adjudicate, not noise to average away.
    Divergence,
}

impl UnknownReason {
    pub fn as_str(&self) -> &'static str {
        match self {
            UnknownReason::MissingGrounding => "missing-grounding",
            UnknownReason::NoEngine => "no-engine",
            UnknownReason::Inconclusive => "inconclusive",
            UnknownReason::Divergence => "divergence-needs-review",
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

/// One engine's contribution to a requirement's verdict (D2b). The core aggregates a
/// `Vec<Evidence>` — one per engine that actually ran — into a single [`Verdict`]. Unlike a
/// verdict it carries no `id`/provenance (those belong to the requirement, not the engine);
/// it is purely *what this tool said*.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Evidence {
    /// The engine that produced this, e.g. `"Kani"` — named so a divergence or a
    /// corroboration can say *who*.
    pub engine: String,
    pub status: Status,
    /// The D8 rung, present exactly when `status == Holds`.
    pub basis: Option<Basis>,
    /// The re-checkable witness, present for a `fails` when the engine produced one.
    pub witness: Option<String>,
    /// The engine's own message (the inconclusive reason, the violated check).
    pub detail: Vec<String>,
    /// Mechanical (a provreq-generated harness) or asserted (a human-tagged artifact). Every
    /// engine's evidence is mechanical; only a tagged artifact is asserted (Phase 4b, REQ076).
    pub correspondence: Correspondence,
    /// The tagged source this came from — present only for asserted evidence.
    pub source_location: Option<SourceLocation>,
}

impl Evidence {
    pub fn holds(engine: &str, basis: Basis) -> Evidence {
        Evidence {
            engine: engine.to_string(),
            status: Status::Holds,
            basis: Some(basis),
            witness: None,
            detail: Vec::new(),
            correspondence: Correspondence::Mechanical,
            source_location: None,
        }
    }

    pub fn fails(engine: &str, witness: Option<String>, detail: Vec<String>) -> Evidence {
        Evidence {
            engine: engine.to_string(),
            status: Status::Fails,
            basis: None,
            witness,
            detail,
            correspondence: Correspondence::Mechanical,
            source_location: None,
        }
    }

    /// The empirical rung (#229) — a monitor saw no violation in the trace it read.
    ///
    /// `over` is a parameter, not an optional detail line, and that is the whole point: a bare
    /// `not-falsified` overclaims by omission the way a bare `holds` did before #121. What the
    /// monitor actually read — which trace, how long, over what span — is what makes the claim
    /// checkable at all, so the only constructor for this rung demands it. It lands in `detail`
    /// beside the engine that earned it, exactly as TLC's model line does for category 2a.
    pub fn not_falsified(engine: &str, over: impl Into<String>) -> Evidence {
        Evidence {
            detail: vec![format!("not falsified over — {}", over.into())],
            ..Evidence::holds(engine, Basis::NotFalsified)
        }
    }

    pub fn inconclusive(engine: &str, detail: Vec<String>) -> Evidence {
        Evidence {
            engine: engine.to_string(),
            status: Status::Unknown,
            basis: None,
            witness: None,
            detail,
            correspondence: Correspondence::Mechanical,
            source_location: None,
        }
    }

    /// Mark this evidence **asserted** and stamp the tagged source it came from (Phase 4b). The
    /// producer builds the polarity through the ordinary ladder constructor — a tagged passing
    /// test earns `not_falsified`, no stronger rung — then calls this so the honest marker and the
    /// location travel with it. There is no `asserted` polarity constructor on purpose: asserted
    /// is a property of *where* the evidence came from, not a fourth kind of answer.
    pub fn asserted_at(mut self, location: SourceLocation) -> Evidence {
        self.correspondence = Correspondence::Asserted;
        self.source_location = Some(location);
        self
    }
}

/// A verdict for one requirement (D7). Splits **polarity** (`status`) from **basis** (how a
/// `holds` was established) and from the **witness** (what makes a `fails` re-checkable).
///
/// The aggregate fields (`status`/`basis`/`witness`/`detail`) are the ensemble's combined
/// answer per D2b; `evidence` keeps every engine's own result so the read-back can name who
/// held, who refuted, and who could not decide.
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
    /// Per-engine results that were aggregated into this verdict (D2b). Empty when no
    /// engine ran (missing-grounding / no-engine): there is nothing to break down.
    pub evidence: Vec<Evidence>,
    /// Whether the verdict's decisive polarity rests on anything **mechanical** (Phase 4c).
    /// `Asserted` only when nothing mechanical backs it — a `holds` resting solely on a human's
    /// tagged test — so the top line can never read as a mechanical proof. `Mechanical` whenever
    /// an engine's own evidence decides it, and for the no-evidence unknowns (nothing was asserted).
    pub correspondence: Correspondence,
    pub provenance: Provenance,
}

/// The correspondence of the evidence that **decides** a verdict's polarity: `Asserted` only when
/// every piece of decisive evidence is asserted, else `Mechanical`. Looking at the decisive subset
/// (the holders for a `holds`, the refuters for a `fails`, the conflict for a divergence) rather
/// than all evidence is the point — a mechanical *inconclusive* alongside an asserted *holds* must
/// not let the holds read as mechanical, because the mechanical engine established nothing.
fn decisive_correspondence(
    status: Status,
    reason: Option<UnknownReason>,
    evidence: &[Evidence],
) -> Correspondence {
    let decisive = |e: &Evidence| match status {
        Status::Holds => e.status == Status::Holds,
        Status::Fails => e.status == Status::Fails,
        Status::Unknown if reason == Some(UnknownReason::Divergence) => {
            e.status == Status::Holds || e.status == Status::Fails
        }
        Status::Unknown => e.status == Status::Unknown,
    };
    let mut relevant = evidence.iter().filter(|e| decisive(e)).peekable();
    // No decisive evidence (a missing-grounding / no-engine unknown) asserted nothing → mechanical.
    if relevant.peek().is_none() {
        return Correspondence::Mechanical;
    }
    if relevant.any(|e| e.correspondence == Correspondence::Mechanical) {
        Correspondence::Mechanical
    } else {
        Correspondence::Asserted
    }
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

/// Aggregate every engine's [`Evidence`] into one requirement verdict per **D2b** —
/// soundness-aware, never a majority vote. `evidence` must be non-empty (an engine ran);
/// callers where no engine ran use [`no_engine`] / [`from_grounding`].
///
/// - A `holds` and a `fails` together is **divergence-needs-review**: one engine found a
///   re-checkable counterexample while another established the claim — a real discrepancy no
///   vote can resolve.
/// - Otherwise any `fails` refutes the requirement (a valid counterexample is definitive),
///   carrying the first witness produced.
/// - Otherwise any `holds` establishes it; agreement *corroborates* rather than out-votes,
///   and the basis is the strongest rung any holding engine earned.
/// - Otherwise every engine was inconclusive → `unknown / inconclusive`.
///
/// Implements: REQ030
pub fn aggregate(id: &str, evidence: Vec<Evidence>, provenance: Provenance) -> Verdict {
    let any_holds = evidence.iter().any(|e| e.status == Status::Holds);
    let any_fails = evidence.iter().any(|e| e.status == Status::Fails);
    let detail: Vec<String> = evidence.iter().flat_map(describe_evidence).collect();

    let (status, reason, basis, witness) = if any_holds && any_fails {
        (Status::Unknown, Some(UnknownReason::Divergence), None, None)
    } else if any_fails {
        (
            Status::Fails,
            None,
            None,
            evidence.iter().find_map(|e| e.witness.clone()),
        )
    } else if any_holds {
        let basis = evidence
            .iter()
            .filter_map(|e| e.basis)
            .max_by_key(basis_rank);
        (Status::Holds, None, basis, None)
    } else {
        (
            Status::Unknown,
            Some(UnknownReason::Inconclusive),
            None,
            None,
        )
    };

    let correspondence = decisive_correspondence(status, reason, &evidence);
    Verdict {
        id: id.to_string(),
        status,
        reason,
        basis,
        witness,
        detail,
        evidence,
        correspondence,
        provenance,
    }
}

/// D8 rung ordering — higher is stronger, so [`aggregate`]'s `max_by_key` reports the
/// strongest basis any holding engine earned: a `proven` outranks a bounded `model-checked`,
/// which is how "proven by Creusot, corroborated bounded by Kani" comes out proven.
///
/// `not-falsified` sits at the bottom for the same reason, running the other way: a monitor
/// corroborating a bounded `holds` must never *demote* it to the empirical rung — an
/// observation of what ran adds confidence, it does not subtract the model check.
fn basis_rank(basis: &Basis) -> u8 {
    match basis {
        Basis::Proven => 2,
        Basis::ModelCheckedBounded => 1,
        Basis::NotFalsified => 0,
    }
}

/// A human line per engine, folded into the verdict `detail` so the read-back names who
/// held, who refuted, and who could not decide.
fn describe_evidence(e: &Evidence) -> Vec<String> {
    let head = match e.status {
        Status::Holds => format!(
            "{}: holds{}",
            e.engine,
            e.basis
                .map(|b| format!(" ({})", b.as_str()))
                .unwrap_or_default()
        ),
        Status::Fails => format!("{}: fails", e.engine),
        Status::Unknown => format!("{}: inconclusive", e.engine),
    };
    std::iter::once(head)
        .chain(e.detail.iter().cloned())
        .collect()
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
        evidence: Vec::new(),
        // No engine ran and nothing was asserted, so there is no asserted claim to flag.
        correspondence: Correspondence::Mechanical,
        provenance,
    }
}

/// Render a verdict as a human read-back (D1 round-trip). Deterministic, no LLM.
///
/// A `holds` always renders its basis, so a bounded result can never be *read* as a proof
/// even at a glance — the overclaim D8 guards against is a reading error as much as a
/// modelling one. Symmetrically, a genuine `proven` must not wear the bounded caveat, or the
/// read-back would *under*claim a deductive proof — so the caveat branches on the basis.
pub fn render(v: &Verdict) -> String {
    let mut out = format!("{}: {}", v.id, v.status.as_str());
    if let Some(basis) = v.basis {
        let gloss = match basis {
            Basis::Proven => {
                "established deductively for every execution (spec-relative), not just the \
                 states a bounded checker explored"
            }
            Basis::ModelCheckedBounded => {
                "verified over the states the engine explored, NOT proven for all executions"
            }
            // Deliberately says nothing about *how* it was observed. #229 wrote "the trace a
            // monitor actually read" when MonPoly was the only engine on this rung; #245 put a
            // browser on it too, and a UI driver reads no trace — the CLI was printing a sentence
            // about monitoring under a verdict produced by clicking a button. What each engine
            // actually saw travels with the evidence (`not falsified over — …`), which is where a
            // per-engine account belongs; this line is the rung, and the rung is the same for both.
            Basis::NotFalsified => {
                "no counterexample appeared in what was actually observed — a statement about what \
                 ran, NOT about what can run"
            }
        };
        out.push_str(&format!(" — {}: {gloss}", basis.as_str()));
    }
    // The masquerade guard (Phase 4c): a verdict resting only on a human-tagged test is marked so
    // it can never be *read* as a mechanical result, exactly as a bounded basis is marked above.
    if v.correspondence == Correspondence::Asserted {
        out.push_str(
            " [asserted — established by a human-tagged test provreq ran and passed, not by a \
             harness provreq generated from the claim]",
        );
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
            UnknownReason::Divergence => out.push_str(
                " — engines disagreed (one holds, another fails); a human must reconcile the \
                 witness against the basis, never a majority vote",
            ),
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

/// A JSON-serializable view of one engine's [`Evidence`] for the `POST /:id/verify` surface.
/// The polarity/basis carry their human labels (`"holds"`, `"proven"`) — the same strings the
/// CLI [`render`] prints — so the web surface and the command line never diverge on wording.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct EvidenceReport {
    pub engine: String,
    pub status: String,
    pub basis: Option<String>,
    pub witness: Option<String>,
    pub detail: Vec<String>,
    /// `"mechanical"` or `"asserted"` (Phase 4c). `#[serde(default)]` so a verdict stored before
    /// this field existed loads as mechanical — an old engine record was never asserted, so this
    /// never invents an asserted flag on legacy data. `default_correspondence` supplies the string.
    #[serde(default = "default_correspondence")]
    pub correspondence: String,
    /// The tagged source an asserted evidence came from; absent on mechanical evidence.
    #[serde(default)]
    pub source_location: Option<SourceLocation>,
}

/// The `correspondence` a record carries when it predates the field — mechanical, because every
/// verdict stored before Phase 4c came from an engine's own (mechanical) evidence.
fn default_correspondence() -> String {
    Correspondence::Mechanical.as_str().to_string()
}

/// A JSON-serializable view of a verdict's [`Provenance`] (D9).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ProvenanceReport {
    pub requirement_revision: String,
    pub subject_commit: Option<String>,
    pub tool_version: String,
    /// The formalization fingerprint (REQ045). `#[serde(default)]` so a verdict persisted before
    /// this field existed loads as `None` and simply skips the formalization drift axis.
    #[serde(default)]
    pub formalization: Option<String>,
    /// The environment this verdict was produced in (REQ049) — where it was *proved*, not what the
    /// subject declares it offers. `#[serde(default)]` for the same reason as `formalization`: a
    /// verdict from before this axis existed skips it rather than being flagged on a basis that
    /// cannot be established.
    #[serde(default)]
    pub environment: Option<crate::proving_env::ProvingEnv>,
    /// A fingerprint of the specs this was proved against that live **outside** the subject tree
    /// (#120), so the model moving makes the verdict stale. `None` — and skipped as a drift axis —
    /// both for a verdict from before this existed and for a subject whose specs are all in-tree,
    /// where `subject_commit` already covers them.
    #[serde(default)]
    pub spec_fingerprint: Option<String>,
    /// A fingerprint of the trace a monitor read (#230). The subject's commit does not cover a log
    /// the subject *produced*, and a log grows — so without this a verdict would read `fresh` while
    /// the only evidence under it moved. `None`, and skipped as a drift axis, for a verdict from
    /// before this existed or a subject with no monitor configured.
    #[serde(default)]
    pub trace_fingerprint: Option<String>,
    /// A fingerprint of the subject's tracked source at HEAD, with the companion tree and the
    /// requirement documents excluded (#271) — exactly the records whose drift other axes already
    /// own. The commit is a coarser clock than the code: committing the verdict record itself
    /// moves it, so under commit comparison a subject that keeps its record in-tree can never
    /// hold a fresh verdict. When both the stored verdict and the anchor carry this, it decides
    /// the code-drift axis and the commit stays as reproducibility metadata; a verdict from
    /// before this axis existed falls back to the commit comparison — freshness is never widened
    /// on a basis the stored verdict does not carry. Reads HEAD, so a dirty tree is as invisible
    /// to it as it always was to the commit.
    #[serde(default)]
    pub source_fingerprint: Option<String>,
    /// A fingerprint of the UI check this was driven by (#239) — the deployment URL and the steps.
    /// The only out-of-commit axis with no file behind it: a running deployment has no bytes to
    /// hash, so this covers what was *declared*, and is blind to that deployment changing at the
    /// same URL. `None`, and skipped as a drift axis, for a verdict from before this existed or a
    /// subject with no UI check configured.
    #[serde(default)]
    pub ui_fingerprint: Option<String>,
    /// A fingerprint of just the tagged source files an asserted verdict rests on (Phase 4c) — so
    /// a change to the specific test that backs *this* requirement can be pointed at precisely,
    /// rather than the whole-tree `source_fingerprint` reporting every verdict stale at once. `None`
    /// — and skipped as a drift axis — for a verdict with no asserted evidence, or one from before
    /// this field existed. It does not replace `source_fingerprint`; it refines it for the report.
    #[serde(default)]
    pub tagged_source_fingerprint: Option<String>,
}

/// A JSON-serializable view of a [`Verdict`] for the web surface (REQ038). The domain types are
/// deliberately not `Serialize` (they are the core's own vocabulary); this is the wire shape, with
/// polarity/basis/reason rendered to their labels so the client renders no enum internals.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct VerdictReport {
    pub id: String,
    pub status: String,
    pub basis: Option<String>,
    pub reason: Option<String>,
    pub witness: Option<String>,
    pub detail: Vec<String>,
    pub evidence: Vec<EvidenceReport>,
    /// The verdict-level correspondence (Phase 4c) — `"asserted"` when the verdict rests only on a
    /// human-tagged test. `#[serde(default)]` → mechanical for records predating the field.
    #[serde(default = "default_correspondence")]
    pub correspondence: String,
    pub provenance: ProvenanceReport,
}

/// Render a [`Verdict`] into its JSON wire shape. Pure — the aggregate polarity, the per-engine
/// breakdown (D2b), and the provenance all carry through by their labels.
pub fn report(v: &Verdict) -> VerdictReport {
    VerdictReport {
        id: v.id.clone(),
        status: v.status.as_str().to_string(),
        basis: v.basis.map(|b| b.as_str().to_string()),
        reason: v.reason.map(|r| r.as_str().to_string()),
        witness: v.witness.clone(),
        detail: v.detail.clone(),
        evidence: v
            .evidence
            .iter()
            .map(|e| EvidenceReport {
                engine: e.engine.clone(),
                status: e.status.as_str().to_string(),
                basis: e.basis.map(|b| b.as_str().to_string()),
                witness: e.witness.clone(),
                detail: e.detail.clone(),
                correspondence: e.correspondence.as_str().to_string(),
                source_location: e.source_location.clone(),
            })
            .collect(),
        correspondence: v.correspondence.as_str().to_string(),
        provenance: ProvenanceReport {
            requirement_revision: v.provenance.requirement_revision.clone(),
            subject_commit: v.provenance.subject_commit.clone(),
            tool_version: v.provenance.tool_version.clone(),
            // The formalization fingerprint, the proving environment, the out-of-subject spec
            // fingerprint, the monitor trace's, the UI check's, and the source fingerprint are
            // persistence concerns, not part of the in-memory verdict's identity — the verify flow
            // stamps all six on the stored copy (REQ045, REQ049, #120, #230, #239, #271). A live
            // (unpersisted) report carries none of them; nothing reads them off the wire.
            formalization: None,
            environment: None,
            spec_fingerprint: None,
            trace_fingerprint: None,
            ui_fingerprint: None,
            source_fingerprint: None,
            tagged_source_fingerprint: None,
        },
    }
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

    // Verifies: #220 — the invariant a surface rendering BOTH `detail` and `evidence` depends on.
    // `aggregate` builds `detail` entirely by folding `evidence`, so every line is already
    // attributable to an engine; `unknown` (the only other constructor, behind `from_grounding` and
    // `no_engine`) carries the verdict's own lines and NO evidence. The two never mix, which is why
    // the web surface can drop `detail` whenever evidence is present without a string match and
    // without losing a line. Adding a verdict-level line to `aggregate`'s `detail` would break that
    // silently — the browser would stop showing it — so it must break here first.
    #[test]
    fn detail_is_a_fold_of_evidence_or_evidence_is_empty() {
        let engines = vec![
            Evidence {
                detail: vec!["checked under the model — MaxAlt = 2".into()],
                ..Evidence::holds("TLC (TLA+)", Basis::ModelCheckedBounded)
            },
            Evidence::inconclusive("Kani", vec!["harness would not compile".into()]),
        ];
        let v = aggregate("REQ001", engines.clone(), prov());
        let folded: Vec<String> = engines.iter().flat_map(describe_evidence).collect();
        assert_eq!(
            v.detail, folded,
            "aggregate's detail is exactly the fold, nothing of its own"
        );

        for v in [
            from_grounding("REQ001", &Grounding::Grounded, prov()),
            from_grounding(
                "REQ001",
                &Grounding::Parked {
                    reasons: vec!["nope".into()],
                },
                prov(),
            ),
            no_engine(
                "REQ001",
                vec!["no engine is wired for category 3".into()],
                prov(),
            ),
        ] {
            assert!(
                v.evidence.is_empty(),
                "a verdict carrying its own detail must carry no evidence: {:?}",
                v.detail
            );
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

    // Verifies: REQ038 — the wire report carries the labels (not enum internals), the D9
    // provenance, and every engine's own breakdown so the web surface reads them verbatim.
    #[test]
    fn report_carries_labels_provenance_and_per_engine_breakdown() {
        let v = aggregate(
            "REQ001",
            vec![
                Evidence::holds("Creusot", Basis::Proven),
                Evidence::inconclusive("Kani", vec!["harness would not compile".into()]),
            ],
            prov(),
        );
        let r = report(&v);
        assert_eq!(r.id, "REQ001");
        assert_eq!(r.status, "holds");
        assert_eq!(r.basis.as_deref(), Some("proven"));
        assert_eq!(r.provenance.subject_commit.as_deref(), Some("abc123"));
        assert_eq!(r.evidence.len(), 2);
        let kani = r.evidence.iter().find(|e| e.engine == "Kani").unwrap();
        assert_eq!(kani.status, "unknown");
        assert_eq!(kani.detail, vec!["harness would not compile".to_string()]);
    }

    // Verifies: REQ030 — a single engine's evidence aggregates to exactly that engine's
    // answer (N=1 is the wired reality today; the ensemble must not change it).
    #[test]
    fn single_holds_aggregates_to_that_holds() {
        let v = aggregate(
            "SR010",
            vec![Evidence::holds("Kani", Basis::ModelCheckedBounded)],
            prov(),
        );
        assert_eq!(v.status, Status::Holds);
        assert_eq!(v.basis, Some(Basis::ModelCheckedBounded));
        assert_eq!(v.evidence.len(), 1);
        assert!(render(&v).contains("Kani: holds"));
    }

    // Verifies: REQ030 (D2b) — agreement corroborates rather than out-votes; the read-back
    // names every contributing engine.
    #[test]
    fn agreeing_holds_corroborate() {
        let v = aggregate(
            "SR011",
            vec![
                Evidence::holds("Kani", Basis::ModelCheckedBounded),
                Evidence::holds("Prusti", Basis::ModelCheckedBounded),
            ],
            prov(),
        );
        assert_eq!(v.status, Status::Holds);
        let text = render(&v);
        assert!(text.contains("Kani: holds"));
        assert!(text.contains("Prusti: holds"));
    }

    // Verifies: REQ030 (D2b) — a `holds` and a `fails` together is divergence-needs-review,
    // NEVER resolved by majority vote.
    #[test]
    fn holds_versus_fails_is_divergence() {
        let v = aggregate(
            "SR012",
            vec![
                Evidence::holds("Prusti", Basis::ModelCheckedBounded),
                Evidence::fails("Kani", Some("cex: u=0".into()), vec![]),
            ],
            prov(),
        );
        assert_eq!(v.status, Status::Unknown);
        assert_eq!(v.reason, Some(UnknownReason::Divergence));
        assert!(render(&v).contains("divergence-needs-review"));
    }

    // Verifies: REQ030 — a lone counterexample refutes and carries its witness.
    #[test]
    fn a_fails_refutes_and_keeps_the_witness() {
        let v = aggregate(
            "SR013",
            vec![Evidence::fails("Kani", Some("cex: u=0".into()), vec![])],
            prov(),
        );
        assert_eq!(v.status, Status::Fails);
        assert_eq!(v.witness.as_deref(), Some("cex: u=0"));
    }

    // Verifies: REQ030 — a `holds` corroborated by an inconclusive engine still holds;
    // an inability to decide is not a disagreement.
    #[test]
    fn inconclusive_does_not_block_a_holds() {
        let v = aggregate(
            "SR014",
            vec![
                Evidence::holds("Kani", Basis::ModelCheckedBounded),
                Evidence::inconclusive("Prusti", vec!["no contracts on `login`".into()]),
            ],
            prov(),
        );
        assert_eq!(v.status, Status::Holds);
        assert!(render(&v).contains("Prusti: inconclusive"));
    }

    // Verifies: REQ031 (D8) — a deductive `proven` renders WITHOUT the bounded caveat. A
    // real proof holds for all executions, so reading it as "only the states explored" would
    // under-claim it — the symmetric error to letting a bounded pass read as a proof.
    #[test]
    fn a_proven_holds_does_not_wear_the_bounded_caveat() {
        let v = aggregate(
            "SR020",
            vec![Evidence::holds("Creusot", Basis::Proven)],
            prov(),
        );
        assert_eq!(v.status, Status::Holds);
        assert_eq!(v.basis, Some(Basis::Proven));
        let text = render(&v);
        assert!(text.contains("proven: established deductively"), "{text}");
        assert!(
            !text.contains("NOT proven for all executions"),
            "a deductive proof must not read as bounded: {text}"
        );
    }

    // Verifies: REQ031 (D2b) — `proven` outranks bounded `model-checked`, so an ensemble
    // where Creusot proves and Kani corroborates bounded reports the STRONGER basis. This is
    // "proven by Creusot, corroborated bounded by Kani".
    #[test]
    fn proven_outranks_bounded_model_checked_in_the_ensemble() {
        let v = aggregate(
            "SR021",
            vec![
                Evidence::holds("Kani", Basis::ModelCheckedBounded),
                Evidence::holds("Creusot", Basis::Proven),
            ],
            prov(),
        );
        assert_eq!(v.status, Status::Holds);
        assert_eq!(
            v.basis,
            Some(Basis::Proven),
            "the strongest rung any holding engine earned wins"
        );
        let text = render(&v);
        assert!(text.contains("Kani: holds"), "{text}");
        assert!(text.contains("Creusot: holds (proven)"), "{text}");
    }

    // Verifies: #229 (D8) — the empirical rung renders as what it is. `not-falsified` must never
    // read like a proof or a model check: something was observed, and that is all it was.
    //
    // #245 put a second engine on this rung — a browser, which reads no trace — so the gloss says
    // what the RUNG is and leaves how it was observed to the evidence's own extent line. It was
    // previously MonPoly's sentence, and a UI verdict printed it: a `holds` produced by clicking a
    // button announced itself as a trace a monitor had read. Both engines are checked here so the
    // shared line cannot drift back toward either one's vocabulary.
    #[test]
    fn not_falsified_renders_as_a_statement_about_what_ran() {
        for evidence in [
            Evidence::not_falsified(
                "MonPoly",
                "logs/events.jsonl — 4812 events, 2026-08-01T00:00Z … 2026-08-06T23:59Z",
            ),
            Evidence::not_falsified(
                crate::ui::ENGINE,
                "one execution of the deployment at http://localhost:8080 in chrome 124.0",
            ),
        ] {
            let engine = evidence.engine.clone();
            let v = aggregate("SR030", vec![evidence], prov());
            assert_eq!(v.status, Status::Holds);
            assert_eq!(v.basis, Some(Basis::NotFalsified));
            let text = render(&v);
            assert!(
                !text.contains("trace") && !text.contains("monitor"),
                "the rung's gloss must not describe one engine's way of observing ({engine}): \
                 {text}"
            );
            assert!(
                text.contains("not-falsified: no counterexample appeared"),
                "{text}"
            );
            assert!(
                text.contains("NOT about what can run"),
                "the rung must disclaim the reach it does not have: {text}"
            );
        }

        let v = aggregate(
            "SR030",
            vec![Evidence::not_falsified(
                "MonPoly",
                "logs/events.jsonl — 4812 events, 2026-08-01T00:00Z … 2026-08-06T23:59Z",
            )],
            prov(),
        );
        let text = render(&v);
        assert!(
            !text.contains("established deductively")
                && !text.contains("states the engine explored"),
            "an empirical result must not borrow a stronger rung's words: {text}"
        );
    }

    // Verifies: #229 — the extent travels with the rung. A bare `not-falsified` overclaims by
    // omission (the defect #121 fixed for the 2a model line), so the ONLY constructor for this
    // rung takes what it was checked over and puts it beside the engine that earned it.
    #[test]
    fn not_falsified_says_what_it_was_checked_over() {
        let e = Evidence::not_falsified("MonPoly", "logs/events.jsonl — 4812 events");
        assert_eq!(e.basis, Some(Basis::NotFalsified));
        assert_eq!(
            e.detail,
            vec!["not falsified over — logs/events.jsonl — 4812 events".to_string()]
        );
        let text = render(&aggregate("SR031", vec![e], prov()));
        assert!(text.contains("MonPoly: holds (not-falsified)"), "{text}");
        assert!(text.contains("4812 events"), "{text}");
    }

    // Verifies: #229 (D2b) — the empirical rung is the WEAKEST, and ranking is what keeps that
    // true in both directions. A monitor corroborating a bounded model check must not demote the
    // verdict to `not-falsified`; a monitor alone must not be promoted past what it saw.
    #[test]
    fn an_empirical_corroboration_never_demotes_a_model_check() {
        let v = aggregate(
            "SR032",
            vec![
                Evidence::not_falsified("MonPoly", "logs/events.jsonl — 4812 events"),
                Evidence::holds("TLC (TLA+)", Basis::ModelCheckedBounded),
            ],
            prov(),
        );
        assert_eq!(
            v.basis,
            Some(Basis::ModelCheckedBounded),
            "the strongest rung any holding engine earned wins, and empirical is not it"
        );
        assert!(basis_rank(&Basis::NotFalsified) < basis_rank(&Basis::ModelCheckedBounded));
        assert!(basis_rank(&Basis::NotFalsified) < basis_rank(&Basis::Proven));
        // Both engines still get their say — the weaker rung is outranked, not erased.
        let text = render(&v);
        assert!(text.contains("MonPoly: holds (not-falsified)"), "{text}");
        assert!(
            text.contains("TLC (TLA+): holds (model-checked (bounded))"),
            "{text}"
        );
    }

    // Verifies: REQ030 — every engine inconclusive yields unknown/inconclusive, not a fake
    // decision.
    #[test]
    fn all_inconclusive_is_unknown_inconclusive() {
        let v = aggregate(
            "SR015",
            vec![Evidence::inconclusive(
                "Kani",
                vec!["harness would not compile".into()],
            )],
            prov(),
        );
        assert_eq!(v.status, Status::Unknown);
        assert_eq!(v.reason, Some(UnknownReason::Inconclusive));
    }

    fn asserted_test(status_holds: bool) -> Evidence {
        let loc = SourceLocation {
            file: std::path::PathBuf::from("src/a.rs"),
            line: 5,
            symbol: Some("the_test".into()),
        };
        if status_holds {
            Evidence::not_falsified("cargo test", "the tagged test passed").asserted_at(loc)
        } else {
            Evidence::fails("cargo test", None, vec!["failed".into()]).asserted_at(loc)
        }
    }

    // Verifies: REQ076 — a verdict resting only on a tagged passing test is ASSERTED at the top
    // level, so it can never be read as a mechanical proof (the masquerade guard).
    #[test]
    fn a_verdict_from_a_tagged_test_alone_is_asserted() {
        let v = aggregate("A", vec![asserted_test(true)], prov());
        assert_eq!(v.status, Status::Holds);
        assert_eq!(v.basis, Some(Basis::NotFalsified));
        assert_eq!(v.correspondence, Correspondence::Asserted);
    }

    // Verifies: REQ076 — a mechanical proof present makes the verdict mechanical even when an
    // asserted test corroborates it; the basis stays the strongest rung.
    #[test]
    fn a_mechanical_proof_dominates_the_verdict_correspondence() {
        let v = aggregate(
            "A",
            vec![
                Evidence::holds("Creusot", Basis::Proven),
                asserted_test(true),
            ],
            prov(),
        );
        assert_eq!(v.status, Status::Holds);
        assert_eq!(v.basis, Some(Basis::Proven));
        assert_eq!(v.correspondence, Correspondence::Mechanical);
    }

    // Verifies: REQ076 — the subtlety: a mechanical INCONCLUSIVE alongside an asserted holds must
    // not let the holds read as mechanical, because the mechanical engine established nothing. Only
    // the decisive (holding) evidence counts, and it is asserted.
    #[test]
    fn a_mechanical_inconclusive_does_not_make_an_asserted_holds_mechanical() {
        let v = aggregate(
            "A",
            vec![
                Evidence::inconclusive("Kani", vec!["would not compile".into()]),
                asserted_test(true),
            ],
            prov(),
        );
        assert_eq!(v.status, Status::Holds);
        assert_eq!(v.correspondence, Correspondence::Asserted);
    }

    // Verifies: REQ076 — a tagged test that fails a mechanically proven claim is divergence, not a
    // silent refutation: a real discrepancy for a human to adjudicate.
    #[test]
    fn a_tagged_fail_against_a_proof_is_divergence() {
        let v = aggregate(
            "A",
            vec![
                Evidence::holds("Creusot", Basis::Proven),
                asserted_test(false),
            ],
            prov(),
        );
        assert_eq!(v.status, Status::Unknown);
        assert_eq!(v.reason, Some(UnknownReason::Divergence));
    }

    // Verifies: REQ076 — the asserted marker travels into the store shape and the read-back.
    #[test]
    fn correspondence_is_surfaced_in_report_and_render() {
        let v = aggregate("A", vec![asserted_test(true)], prov());
        let r = report(&v);
        assert_eq!(r.correspondence, "asserted");
        assert_eq!(r.evidence[0].correspondence, "asserted");
        assert_eq!(r.evidence[0].source_location.as_ref().unwrap().line, 5);
        assert!(render(&v).contains("asserted"), "{}", render(&v));
    }

    // Verifies: REQ076 — an engine-only verdict stays mechanical everywhere, and its store shape
    // carries no source location.
    #[test]
    fn an_engine_verdict_stays_mechanical() {
        let v = aggregate("A", vec![Evidence::holds("Creusot", Basis::Proven)], prov());
        assert_eq!(v.correspondence, Correspondence::Mechanical);
        let r = report(&v);
        assert_eq!(r.correspondence, "mechanical");
        assert!(r.evidence[0].source_location.is_none());
        assert!(!render(&v).contains("[asserted"));
    }
}
