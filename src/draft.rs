//! Step 3 draft lifecycle: a formalization-in-progress persists as a resumable
//! **draft** — a third artifact category beside A3's committed source-of-truth and
//! regenerated-derived, because it holds human keystrokes that are neither
//! admitted nor regenerable (R-draft-1). Resuming a draft re-checks the source
//! revision token so an item that moved underneath it is flagged **stale** before
//! work continues (R-draft-2).
//!
//! No LLM forward-translate, mechanical gate, or read-back yet — those are later
//! Step 3 slices. The candidate PRL is hand-authored for now; the D11 translate
//! slice will fill it automatically.
//!
//! Implements: REQ013 (persist resumable draft state), REQ014 (resume drift-check)

use crate::grounding::Binding;
use crate::source::{Annotation, Item};
use anyhow::{Context, Result, bail};
use std::collections::BTreeMap;
use std::path::Path;

/// Mutable companion state file at the companion root (A6 write-freely channel,
/// keyed by source id) — the draft peer of `triage.yml`.
pub const DRAFT_FILE: &str = "drafts.yml";

/// Warn when the draft store is gitignored. Drafts hold candidate PRLs, groundings, and
/// admissions — human keystrokes and LLM proposals that are **not regenerable** (R-draft-1):
/// `verify` consumes drafts, it never produces them. So a gitignored `drafts.yml` is lost, and
/// lost silently, on the next clean checkout or container rebuild — the failure that cost a
/// pilot ~530 formalizations while its (also-ignored, but separate) verdicts survived and hid it.
///
/// Returns the warning to surface, or `None` when the drafts are safe — or when provreq cannot
/// tell (a non-git subject, or git absent). It never cries wolf: `git check-ignore` is asked, and
/// only a definite "ignored" answer warns.
pub fn drafts_at_loss_risk(subject: &Path, companion_root: &Path) -> Option<String> {
    let drafts = companion_root.join(DRAFT_FILE);
    // Silence git's own stderr ("fatal: not a git repository") — a non-git subject is an expected,
    // silent "cannot tell", not console noise.
    let ignored = std::process::Command::new("git")
        .arg("-C")
        .arg(subject)
        .args(["check-ignore", "-q"])
        .arg(&drafts)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .ok()?
        .success();
    if !ignored {
        return None;
    }
    Some(format!(
        "WARNING: {} is gitignored — candidate PRLs, groundings, and admissions live only here \
         and are NOT regenerable (verify consumes drafts, it does not create them). A clean \
         checkout or container rebuild will lose them. Remove it from .gitignore and commit it.",
        drafts.display()
    ))
}

/// The mechanical-gate outcome recorded on a draft (R-draft-1). Rendered to strings
/// because a draft is a snapshot for the human, not something re-processed — the
/// structured [`crate::prl::GateError`]/`GateWarning` don't need to round-trip YAML.
#[derive(Debug, Clone, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum GateStatus {
    /// A candidate exists but the gate has not been run over it.
    #[default]
    Ungated,
    /// The candidate cleared the gate; `warnings` are vacuity/triviality flags for
    /// the human (empty = clean).
    Passed {
        #[serde(default)]
        warnings: Vec<String>,
    },
    /// The gate rejected the candidate; `errors` are the rendered reasons.
    Failed { errors: Vec<String> },
}

/// The D12 risk tier of a human confirmation. Vacuity-flagged (and later
/// grounding-heavy / high-stakes) candidates are `Mandatory`; a clean candidate is
/// `Optional`. Recorded so "review not required" is never confused with "reviewed".
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReviewTier {
    Mandatory,
    Optional,
}

impl ReviewTier {
    pub fn as_str(&self) -> &'static str {
        match self {
            ReviewTier::Mandatory => "mandatory",
            ReviewTier::Optional => "optional",
        }
    }

    /// Parse a stored tier string (round-trips with [`as_str`](Self::as_str)); `None` on anything
    /// else. The read-back counterpart for a tier persisted as a plain string in an annotation.
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "mandatory" => Some(ReviewTier::Mandatory),
            "optional" => Some(ReviewTier::Optional),
            _ => None,
        }
    }
}

/// The formalization-admission state of a draft (D12). `Pending` is the in-progress
/// draft; `Admitted` is the `admitted-but-ungrounded` lifecycle state — formalization
/// is done, only the grounding anchor is missing — with its review provenance.
#[derive(Debug, Clone, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum Admission {
    #[default]
    Pending,
    Admitted {
        review: ReviewTier,
        by: String,
        /// Wall-clock admission time as Unix seconds (the caller supplies the clock,
        /// keeping this module pure and testable).
        at_unix: i64,
    },
}

/// One item's in-progress formalization draft.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Draft {
    /// The source revision this draft was last touched against (R-src-3).
    /// Staleness (R-draft-2) is `revision != item.revision`.
    pub revision: String,
    /// The candidate PRL — hand-authored (`--set`) or LLM-proposed (`--translate`);
    /// `None` until one is written.
    #[serde(default)]
    pub candidate: Option<String>,
    /// The last mechanical-gate outcome for `candidate` (R-draft-1). Defaults to
    /// `Ungated` so drafts written before this field existed load cleanly.
    #[serde(default)]
    pub gate: GateStatus,
    /// Whether the operator has admitted this formalization (D12). Defaults to
    /// `Pending` so drafts written before this field existed load cleanly.
    #[serde(default)]
    pub admission: Admission,
    /// D13 grounding bindings — each PRL vocabulary symbol bound to a concrete
    /// observable. Defaults to empty so drafts written before this field existed load
    /// cleanly. Dry-run *matches* are never stored here (they drift with the code);
    /// only the bindings persist.
    #[serde(default)]
    pub bindings: Vec<Binding>,
}

impl Draft {
    /// Whether this draft has been admitted (formalization confirmed by a human).
    pub fn is_admitted(&self) -> bool {
        matches!(self.admission, Admission::Admitted { .. })
    }
}

/// A fingerprint of a draft's **complete formal input** — the candidate PRL plus its grounding
/// bindings — so a verdict can record what formalization it was produced against and later detect
/// that the formalization moved (REQ045). `None` when there is no candidate: nothing was
/// formalized, so there is nothing to fingerprint.
///
/// Order-normalized: bindings are sorted, so re-grounding the same symbols in a different sequence
/// is not a spurious change. `DefaultHasher` has fixed keys, so the digest is stable across runs of
/// the same build; a tool upgrade that changed the algorithm would also change `tool_version`,
/// which already drifts every verdict — so the two never disagree.
/// `// ponytail: DefaultHasher digest — swap for a keyed/stable hash only if verdicts must compare
/// across tool versions, which the tool_version axis already prevents.`
pub fn formal_fingerprint(draft: &Draft) -> Option<String> {
    use std::hash::{Hash, Hasher};
    let candidate = draft.candidate.as_deref()?;
    let mut bindings: Vec<String> = draft
        .bindings
        .iter()
        .map(|b| {
            format!(
                "{}={}|{:?}|{:?}",
                b.symbol, b.observable, b.category, b.fidelity
            )
        })
        .collect();
    bindings.sort();
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    candidate.hash(&mut hasher);
    bindings.hash(&mut hasher);
    Some(format!("{:016x}", hasher.finish()))
}

/// The formal fingerprint of an item's draft **only when it is currently admitted** — what a fresh
/// verdict must still match (REQ045). `None` when the item has no draft, is not admitted, or has no
/// candidate: in every such case there is no live admitted formalization for a stored verdict to be
/// about, which the verdict-freshness check reads as drift.
pub fn admitted_fingerprint(state: &DraftState, id: &str) -> Option<String> {
    let draft = state.drafts.get(id)?;
    if !draft.is_admitted() {
        return None;
    }
    formal_fingerprint(draft)
}

/// Persisted draft state, keyed by source id.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct DraftState {
    pub schema: u32,
    pub drafts: BTreeMap<String, Draft>,
}

impl DraftState {
    pub fn new() -> Self {
        Self {
            schema: 1,
            drafts: BTreeMap::new(),
        }
    }
}

impl Default for DraftState {
    fn default() -> Self {
        Self::new()
    }
}

/// Load draft state from a companion root, or an empty state if none is written
/// yet.
pub fn load(companion_root: &Path) -> Result<DraftState> {
    let path = companion_root.join(DRAFT_FILE);
    if !path.exists() {
        return Ok(DraftState::new());
    }
    let text =
        std::fs::read_to_string(&path).with_context(|| format!("reading {}", path.display()))?;
    serde_yaml::from_str(&text).with_context(|| format!("parsing {}", path.display()))
}

/// Write draft state to the companion root.
pub fn save(companion_root: &Path, state: &DraftState) -> Result<()> {
    let path = companion_root.join(DRAFT_FILE);
    let yaml = serde_yaml::to_string(state).context("serializing draft state")?;
    std::fs::write(&path, yaml).with_context(|| format!("writing {}", path.display()))
}

/// Open a draft for an item, snapshotting its current revision. **Additive**: an
/// existing draft is returned untouched, so an in-progress candidate and its drift
/// baseline are never silently reset — resume must see the real staleness
/// (R-draft-2). Returns a new state.
pub fn open(state: &DraftState, item: &Item) -> DraftState {
    if state.drafts.contains_key(&item.id) {
        return state.clone();
    }
    let mut drafts = state.drafts.clone();
    drafts.insert(
        item.id.clone(),
        Draft {
            revision: item.revision.clone(),
            candidate: None,
            gate: GateStatus::Ungated,
            admission: Admission::Pending,
            bindings: Vec::new(),
        },
    );
    DraftState {
        schema: state.schema,
        drafts,
    }
}

/// Write a candidate PRL with its gate outcome and re-baseline the draft against the
/// item's current revision — writing the candidate is confirming it against the current
/// source, clearing any prior staleness (R-draft-2). A new candidate resets admission
/// to `Pending`: changing the formal claim invalidates any prior human confirmation.
/// A new candidate also **clears any grounding bindings**: the vocabulary may have
/// changed, so bindings to the old symbols are no longer trustworthy — re-ground after
/// editing. Returns a new state.
pub fn set_candidate(
    state: &DraftState,
    item: &Item,
    candidate: impl Into<String>,
    gate: GateStatus,
) -> DraftState {
    let mut drafts = state.drafts.clone();
    drafts.insert(
        item.id.clone(),
        Draft {
            revision: item.revision.clone(),
            candidate: Some(candidate.into()),
            gate,
            admission: Admission::Pending,
            bindings: Vec::new(),
        },
    );
    DraftState {
        schema: state.schema,
        drafts,
    }
}

/// Update only the recorded gate outcome for an existing draft, leaving its candidate
/// and revision baseline untouched (a re-check is not an edit). No-op if the draft is
/// absent. Returns a new state.
pub fn set_gate(state: &DraftState, id: &str, gate: GateStatus) -> DraftState {
    let mut drafts = state.drafts.clone();
    if let Some(existing) = drafts.get(id) {
        drafts.insert(
            id.to_string(),
            Draft {
                gate,
                ..existing.clone()
            },
        );
    }
    DraftState {
        schema: state.schema,
        drafts,
    }
}

/// Attach (or overwrite, by symbol) a D13 grounding binding on an existing draft,
/// leaving the candidate, gate, and revision baseline intact — grounding a symbol is not
/// an edit to the formal claim, so it does not revoke admission. No-op if the draft is
/// absent. Returns a new state.
pub fn set_binding(state: &DraftState, id: &str, binding: Binding) -> DraftState {
    let mut drafts = state.drafts.clone();
    if let Some(existing) = drafts.get(id) {
        let mut bindings: Vec<Binding> = existing
            .bindings
            .iter()
            .filter(|b| b.symbol != binding.symbol)
            .cloned()
            .collect();
        bindings.push(binding);
        drafts.insert(
            id.to_string(),
            Draft {
                bindings,
                ..existing.clone()
            },
        );
    }
    DraftState {
        schema: state.schema,
        drafts,
    }
}

/// Admit an existing draft's formalization (D12), recording the review tier and
/// provenance. Leaves the candidate, gate outcome, and revision baseline intact —
/// admission is a confirmation, not an edit. No-op if the draft is absent. The caller
/// supplies `at_unix` so this stays a pure function. Returns a new state.
pub fn admit(
    state: &DraftState,
    id: &str,
    review: ReviewTier,
    by: impl Into<String>,
    at_unix: i64,
) -> DraftState {
    let mut drafts = state.drafts.clone();
    if let Some(existing) = drafts.get(id) {
        drafts.insert(
            id.to_string(),
            Draft {
                admission: Admission::Admitted {
                    review,
                    by: by.into(),
                    at_unix,
                },
                ..existing.clone()
            },
        );
    }
    DraftState {
        schema: state.schema,
        drafts,
    }
}

/// The D12 admit decision plus its state transition (REQ088), factored out so `provreq draft
/// --admit` and the web surface never diverge on what may be admitted. Re-runs the mechanical gate
/// as the source of truth and derives the review tier from it; a clean candidate is optional-review
/// and admits directly, a vacuity-flagged one is mandatory-review and admits only when `confirmed`
/// (the command line's stdin prompt and the UI's read-back checkbox both feed it). A candidate that
/// does not gate, or a draft with no candidate, is refused rather than silently left unadmitted.
/// Returns the admitted state and the tier for the caller to report; `at_unix` is supplied for purity.
pub fn admit_gated(
    state: &DraftState,
    id: &str,
    reviewer: &str,
    confirmed: bool,
    at_unix: i64,
) -> Result<(DraftState, ReviewTier)> {
    let draft = state
        .drafts
        .get(id)
        .with_context(|| format!("no draft for {id} — open one first with `provreq draft {id}`"))?;
    let candidate = draft.candidate.as_deref().with_context(|| {
        format!(
            "draft {id} has no candidate PRL to admit yet — write one with `--set` or `--translate`"
        )
    })?;
    let outcome = crate::prl::gate(candidate).map_err(|errors| {
        anyhow::anyhow!(
            "cannot admit {id} — the candidate has {} gate error(s); fix them first (run `--check`)",
            errors.len()
        )
    })?;
    let tier = if outcome.warnings.is_empty() {
        ReviewTier::Optional
    } else {
        ReviewTier::Mandatory
    };
    if tier == ReviewTier::Mandatory && !confirmed {
        bail!(
            "admitting {id} is mandatory review (vacuity-flagged) — confirm the read-back to admit"
        );
    }
    Ok((admit(state, id, tier, reviewer, at_unix), tier))
}

/// The D14 write-back (REQ088), factored out so the command line and the web surface stamp
/// provenance identically. Requires an admitted draft and refuses a drifted one — an admission made
/// against prose that has since changed must be re-confirmed before it can be written. Writes the
/// confirmed formalization's provenance onto the subject item through the source adapter seam,
/// mutating the working tree; the caller reports it and the operator reviews and commits.
pub fn writeback(subject: &Path, state: &DraftState, item: &Item) -> Result<()> {
    let draft = state
        .drafts
        .get(&item.id)
        .with_context(|| format!("no draft for {} — nothing to write back", item.id))?;
    let Admission::Admitted {
        review,
        by,
        at_unix,
    } = &draft.admission
    else {
        bail!(
            "draft {} is not admitted yet — admit it first with `--admit`",
            item.id
        );
    };
    if is_stale(draft, item) {
        bail!(
            "draft {} needs reconfirmation — the requirement prose moved since admission; \
             re-admit against the current text before writing back",
            item.id
        );
    }
    let annotation = Annotation {
        status: "admitted-but-ungrounded".into(),
        prl: draft.candidate.clone().unwrap_or_default(),
        review: review.as_str().into(),
        reviewer: by.clone(),
        reviewed_at_unix: *at_unix,
        source_revision: draft.revision.clone(),
    };
    // Through the seam, not the Doorstop adapter directly: a Provreq-sourced subject must get that
    // adapter's honest refusal rather than a Doorstop lookup failing for a file that was never going
    // to be there (#296).
    crate::adopt::source_for(&crate::adopt::requirements_root(subject))
        .annotate(&item.id, &annotation)
}

/// Discard a draft, if one exists. Returns a new state.
pub fn discard(state: &DraftState, id: &str) -> DraftState {
    let mut drafts = state.drafts.clone();
    drafts.remove(id);
    DraftState {
        schema: state.schema,
        drafts,
    }
}

/// Convert a mechanical gate result into the persisted [`GateStatus`]. Shared by the command line
/// and the web surface so both record a candidate's gate outcome identically.
pub fn gate_to_status(
    gate: &std::result::Result<crate::prl::GateOutcome, Vec<crate::prl::GateError>>,
) -> GateStatus {
    match gate {
        Ok(outcome) => GateStatus::Passed {
            warnings: outcome.warnings.iter().map(|w| w.to_string()).collect(),
        },
        Err(errors) => GateStatus::Failed {
            errors: errors.iter().map(|e| e.to_string()).collect(),
        },
    }
}

/// Attach a D13 grounding binding to a draft from a `SYMBOL=OBSERVABLE` spec — the whole of
/// `provreq draft --ground`, factored out so the command line and the web surface bind identically.
/// The candidate is gated so the symbol is validated against the *declared* vocabulary; category and
/// default fidelity come from the requirement, and `fidelity` overrides. Returns the new state and
/// the binding made, for the caller to report. Every precondition failure is an error: no draft, no
/// candidate, a malformed spec, a candidate that does not gate, an unbindable symbol, or an unknown
/// fidelity — so a bad request is never silently written.
pub fn ground(
    state: &DraftState,
    id: &str,
    spec: &str,
    fidelity: Option<&str>,
) -> anyhow::Result<(DraftState, Binding)> {
    use anyhow::{Context, anyhow, bail};
    let draft = state
        .drafts
        .get(id)
        .with_context(|| format!("no draft for {id} — set a candidate first"))?;
    let candidate = draft.candidate.as_ref().with_context(|| {
        format!("draft {id} has no candidate PRL to ground yet — set one with a candidate first")
    })?;
    let (symbol, observable) = spec
        .split_once('=')
        .with_context(|| format!("expected SYMBOL=OBSERVABLE, got `{spec}`"))?;
    let (symbol, observable) = (symbol.trim(), observable.trim());
    if symbol.is_empty() || observable.is_empty() {
        bail!("expected a non-empty SYMBOL and OBSERVABLE, got `{spec}`");
    }
    let requirement = crate::prl::gate(candidate)
        .map_err(|errors| {
            anyhow!(
                "cannot ground {id} — the candidate has {} gate error(s); fix them first (check it)",
                errors.len()
            )
        })?
        .requirement;
    if !crate::grounding::is_bindable(&requirement, symbol) {
        let symbols = crate::grounding::bindable_symbols(&requirement);
        bail!(
            "'{symbol}' is not a declared vocabulary symbol of {id}; bindable symbols: {}",
            if symbols.is_empty() {
                "(none)".to_string()
            } else {
                symbols.join(", ")
            }
        );
    }
    let category = crate::grounding::default_category(&requirement);
    let fidelity = match fidelity {
        Some(f) => crate::grounding::Fidelity::parse(f).with_context(|| {
            format!("unknown fidelity '{f}' (definitional | observed | probed)")
        })?,
        None => category.default_fidelity(),
    };
    let binding = Binding {
        symbol: symbol.to_string(),
        category,
        observable: observable.to_string(),
        fidelity,
    };
    let next = set_binding(state, id, binding.clone());
    Ok((next, binding))
}

/// Whether the source item has moved since the draft was last touched (R-draft-2).
/// A stale draft needs human re-confirmation before formalization continues; the
/// engine never runs off a draft written against a since-changed requirement.
pub fn is_stale(draft: &Draft, item: &Item) -> bool {
    draft.revision != item.revision
}

/// Whether an admitted formalization needs re-confirmation (D14): it was admitted, but
/// the source prose has since moved, so the confirmed PRL no longer matches the
/// requirement and must not be trusted or written back until re-admitted.
pub fn needs_reconfirmation(draft: &Draft, item: &Item) -> bool {
    draft.is_admitted() && is_stale(draft, item)
}

#[cfg(test)]
mod tests {
    use super::*;

    // Verifies #477 — a gitignored drafts.yml is caught (it is not regenerable), a tracked one is
    // silent, and a non-git subject never triggers a false alarm.
    #[test]
    fn drafts_at_loss_risk_flags_a_gitignored_store_only() {
        let run_git = |dir: &Path, args: &[&str]| {
            std::process::Command::new("git")
                .arg("-C")
                .arg(dir)
                .args(args)
                .output()
                .expect("git");
        };
        let base = std::env::temp_dir().join(format!("provreq-draftguard-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        let repo = base.join("repo");
        let companion = repo.join("ProvableRequirements");
        std::fs::create_dir_all(&companion).unwrap();
        run_git(&repo, &["init", "-q"]);

        // Ignored → warns.
        std::fs::write(repo.join(".gitignore"), "ProvableRequirements/drafts.yml\n").unwrap();
        assert!(drafts_at_loss_risk(&repo, &companion).is_some());

        // Tracked (not ignored) → silent.
        std::fs::write(repo.join(".gitignore"), "target/\n").unwrap();
        assert!(drafts_at_loss_risk(&repo, &companion).is_none());

        // Not a git subject → cannot tell, so never cries wolf.
        let plain = base.join("plain");
        std::fs::create_dir_all(plain.join("ProvableRequirements")).unwrap();
        assert!(drafts_at_loss_risk(&plain, &plain.join("ProvableRequirements")).is_none());

        std::fs::remove_dir_all(&base).ok();
    }

    fn item(id: &str, revision: &str) -> Item {
        Item {
            id: id.into(),
            text: format!("prose for {id}"),
            revision: revision.into(),
            title: None,
            verification_hint: None,
            expects_code_trace: None,
        }
    }

    // Verifies: REQ086 — grounding a symbol the candidate's vocabulary admits attaches a binding
    // with the requirement's default category/fidelity; a symbol it does not declare is rejected
    // rather than written. This is the shared machinery the web `--ground` endpoint drives.
    #[test]
    fn ground_binds_a_declared_symbol_and_rejects_an_undeclared_one() {
        let it = item("REQ001", "rev-1");
        const CANDIDATE: &str = "requirement r {
            category: 1
            vocabulary { state logged_in(u), has_session(u) }
            require { each u: User . always (not logged_in(u) or has_session(u)) }
        }";
        let state = set_candidate(&DraftState::new(), &it, CANDIDATE, GateStatus::Ungated);

        let (next, binding) =
            ground(&state, "REQ001", "logged_in=auth::is_logged_in", None).expect("declared binds");
        assert_eq!(binding.symbol, "logged_in");
        assert_eq!(binding.observable, "auth::is_logged_in");
        assert_eq!(next.drafts["REQ001"].bindings.len(), 1);

        // A symbol the vocabulary does not declare is rejected, not silently written.
        assert!(ground(&state, "REQ001", "nope=whatever", None).is_err());
        // No candidate at all is likewise an error, never a no-op write.
        assert!(ground(&DraftState::new(), "REQ001", "logged_in=x", None).is_err());
    }

    // Verifies: REQ013 — opening is additive; it never clobbers an in-progress
    // candidate or resets the drift baseline.
    #[test]
    fn open_is_additive_and_preserves_candidate() {
        let it = item("REQ001", "rev-1");
        let opened = open(&DraftState::new(), &it);
        assert_eq!(opened.drafts["REQ001"].revision, "rev-1");
        assert_eq!(opened.drafts["REQ001"].candidate, None);

        let edited = set_candidate(&opened, &it, "requirement foo { }", GateStatus::Ungated);
        // Re-opening the same id leaves the operator's work untouched.
        let reopened = open(&edited, &it);
        assert_eq!(
            reopened.drafts["REQ001"].candidate.as_deref(),
            Some("requirement foo { }")
        );
    }

    // Verifies: REQ014 — a draft is stale exactly when the source revision has
    // moved since the draft was last touched, and editing re-baselines it.
    #[test]
    fn stale_when_source_revision_moves() {
        let v1 = item("REQ001", "rev-1");
        let draft_state = set_candidate(
            &DraftState::new(),
            &v1,
            "requirement foo { }",
            GateStatus::Ungated,
        );
        let draft = &draft_state.drafts["REQ001"];

        // Same revision → fresh.
        assert!(!is_stale(draft, &v1));

        // Source item moved under the draft → stale.
        let v2 = item("REQ001", "rev-2");
        assert!(is_stale(draft, &v2));

        // Editing against the new revision re-baselines it back to fresh.
        let rebaselined = set_candidate(
            &draft_state,
            &v2,
            "requirement foo { edited }",
            GateStatus::Ungated,
        );
        assert!(!is_stale(&rebaselined.drafts["REQ001"], &v2));
    }

    // Verifies: REQ013/REQ017 — draft state (including the gate outcome) round-trips
    // through the companion file, and discard removes it.
    #[test]
    fn state_persists_reloads_and_discards() {
        let tmp = tempfile::tempdir().unwrap();
        let it = item("REQ001", "rev-1");
        let state = set_candidate(
            &DraftState::new(),
            &it,
            "requirement foo { }",
            GateStatus::Passed {
                warnings: vec!["line 1: something suspicious".into()],
            },
        );
        save(tmp.path(), &state).unwrap();

        let loaded = load(tmp.path()).unwrap();
        assert_eq!(loaded, state);
        assert_eq!(
            loaded.drafts["REQ001"].candidate.as_deref(),
            Some("requirement foo { }")
        );
        assert!(matches!(
            loaded.drafts["REQ001"].gate,
            GateStatus::Passed { .. }
        ));

        let after = discard(&loaded, "REQ001");
        assert!(after.drafts.is_empty());
    }

    // Verifies: REQ017 — a re-check updates only the gate outcome, leaving the
    // candidate and revision baseline intact.
    #[test]
    fn set_gate_updates_outcome_only() {
        let it = item("REQ001", "rev-1");
        let state = set_candidate(
            &DraftState::new(),
            &it,
            "requirement foo { }",
            GateStatus::Ungated,
        );
        let rechecked = set_gate(
            &state,
            "REQ001",
            GateStatus::Failed {
                errors: vec!["line 2: boom".into()],
            },
        );
        let d = &rechecked.drafts["REQ001"];
        assert_eq!(d.candidate.as_deref(), Some("requirement foo { }"));
        assert_eq!(d.revision, "rev-1");
        assert!(matches!(d.gate, GateStatus::Failed { .. }));
    }

    #[test]
    fn set_gate_is_a_noop_for_absent_draft() {
        let state = DraftState::new();
        let after = set_gate(&state, "REQ404", GateStatus::Ungated);
        assert!(after.drafts.is_empty());
    }

    #[test]
    fn load_absent_state_is_empty() {
        let tmp = tempfile::tempdir().unwrap();
        assert!(load(tmp.path()).unwrap().drafts.is_empty());
    }

    // Verifies: REQ019 — admitting records the review tier and provenance while
    // leaving the candidate and gate intact.
    #[test]
    fn admit_records_review_and_provenance() {
        let it = item("REQ001", "rev-1");
        let state = set_candidate(
            &DraftState::new(),
            &it,
            "requirement foo { }",
            GateStatus::Ungated,
        );
        assert!(!state.drafts["REQ001"].is_admitted());

        let admitted = admit(&state, "REQ001", ReviewTier::Mandatory, "gg", 1_700_000_000);
        let d = &admitted.drafts["REQ001"];
        assert!(d.is_admitted());
        assert_eq!(d.candidate.as_deref(), Some("requirement foo { }"));
        assert!(matches!(
            &d.admission,
            Admission::Admitted { review: ReviewTier::Mandatory, by, at_unix }
                if by == "gg" && *at_unix == 1_700_000_000
        ));
    }

    // Verifies: REQ019 — editing the candidate after admission resets it to Pending;
    // a changed formal claim is no longer the confirmed one.
    #[test]
    fn editing_candidate_revokes_admission() {
        let it = item("REQ001", "rev-1");
        let state = set_candidate(
            &DraftState::new(),
            &it,
            "requirement foo { }",
            GateStatus::Ungated,
        );
        let admitted = admit(&state, "REQ001", ReviewTier::Optional, "gg", 1);
        assert!(admitted.drafts["REQ001"].is_admitted());

        let edited = set_candidate(&admitted, &it, "requirement bar { }", GateStatus::Ungated);
        assert!(!edited.drafts["REQ001"].is_admitted());
    }

    #[test]
    fn admit_is_a_noop_for_absent_draft() {
        let after = admit(&DraftState::new(), "REQ404", ReviewTier::Optional, "gg", 1);
        assert!(after.drafts.is_empty());
    }

    fn binding(symbol: &str, observable: &str) -> Binding {
        Binding {
            symbol: symbol.into(),
            category: crate::grounding::BindCategory::Code,
            observable: observable.into(),
            fidelity: crate::grounding::Fidelity::Definitional,
        }
    }

    // Verifies: REQ021 — a grounding binding attaches to a draft and overwrites the
    // prior binding for the same symbol (one observable per symbol), without touching
    // the candidate or admission.
    #[test]
    fn set_binding_attaches_and_overwrites_by_symbol() {
        let it = item("REQ001", "rev-1");
        let state = set_candidate(
            &DraftState::new(),
            &it,
            "requirement foo { }",
            GateStatus::Ungated,
        );

        let one = set_binding(&state, "REQ001", binding("logged_in", "fn log_in"));
        assert_eq!(one.drafts["REQ001"].bindings.len(), 1);

        // Re-binding the same symbol replaces, does not duplicate.
        let two = set_binding(&one, "REQ001", binding("logged_in", "fn login"));
        assert_eq!(two.drafts["REQ001"].bindings.len(), 1);
        assert_eq!(two.drafts["REQ001"].bindings[0].observable, "fn login");

        // A different symbol adds.
        let three = set_binding(&two, "REQ001", binding("has_session", "struct Session"));
        assert_eq!(three.drafts["REQ001"].bindings.len(), 2);
    }

    // Verifies: REQ021 — editing the candidate clears grounding bindings (the vocabulary
    // may have changed; stale bindings must not survive an edit).
    #[test]
    fn editing_candidate_clears_bindings() {
        let it = item("REQ001", "rev-1");
        let state = set_candidate(
            &DraftState::new(),
            &it,
            "requirement foo { }",
            GateStatus::Ungated,
        );
        let bound = set_binding(&state, "REQ001", binding("logged_in", "fn log_in"));
        assert_eq!(bound.drafts["REQ001"].bindings.len(), 1);

        let edited = set_candidate(&bound, &it, "requirement bar { }", GateStatus::Ungated);
        assert!(edited.drafts["REQ001"].bindings.is_empty());
    }
}
