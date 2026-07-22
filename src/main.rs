use anyhow::{bail, Context, Result};
use clap::{Parser, Subcommand};
use provreq::adopt::resolve;
use provreq::doorstop::DoorstopSource;
use provreq::draft::{self, Draft, GateStatus};
use provreq::engine;
use provreq::formalize::Translator;
use provreq::grounding::{self, Binding, Grounding};
use provreq::llm::{HttpBackend, LlmClassifier};
use provreq::prl::Requirement;
use provreq::rust_adapter::Resolution;
use provreq::source::{Classification, Item, RequirementsSource};
use provreq::triage::{self, ProseFloorClassifier, TriageState};
use std::collections::BTreeMap;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

/// PRL native provisioner and backend server.
#[derive(Parser)]
#[command(name = "provreq", version, about)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Start the local web server and serve the embedded UI.
    Serve {
        /// TCP port to bind on the loopback interface.
        #[arg(long, default_value_t = 8080)]
        port: u16,
        /// Path to the subject repository the UI browses (defaults to the current directory).
        #[arg(long, default_value = ".")]
        path: PathBuf,
    },
    /// Discover a subject repo's Doorstop layout and scaffold the companion tree.
    Init {
        /// Path to the subject repository (defaults to the current directory).
        #[arg(default_value = ".")]
        path: PathBuf,
        /// Override the proposed companion-tree name.
        #[arg(long)]
        name: Option<String>,
        /// Scaffold without the interactive confirmation prompt.
        #[arg(long)]
        yes: bool,
    },
    /// Classify requirement items (advisory) and show the triage list.
    Triage {
        /// Path to the subject repository (defaults to the current directory).
        #[arg(default_value = ".")]
        path: PathBuf,
        /// Override one item's bucket: `--set REQ001 formalizable-now`.
        #[arg(long, num_args = 2, value_names = ["ID", "BUCKET"])]
        set: Option<Vec<String>>,
    },
    /// Open, resume, edit, gate, read back, admit, or discard a formalization draft.
    Draft {
        /// Requirement item id (e.g. REQ001). Omit to list all drafts.
        id: Option<String>,
        /// Path to the subject repository (defaults to the current directory).
        #[arg(long, default_value = ".")]
        path: PathBuf,
        /// Write the candidate PRL for this draft (re-baselines against the item).
        #[arg(long, value_name = "PRL")]
        set: Option<String>,
        /// Propose the candidate PRL with the configured LLM (D11 forward-translate).
        #[arg(long, conflicts_with_all = ["set", "discard"])]
        translate: bool,
        /// Run the mechanical gate (parse + type/name-check) over this draft's candidate.
        #[arg(long, conflicts_with_all = ["set", "translate", "discard"])]
        check: bool,
        /// Render the D12 read-back — the deterministic CNL surfacing of the candidate's
        /// formal meaning — for the operator to confirm intent (requires a gate pass).
        #[arg(long, conflicts_with_all = ["set", "translate", "check", "discard"])]
        readback: bool,
        /// Admit this draft's formalization after confirming the read-back (D12).
        #[arg(long, conflicts_with_all = ["set", "translate", "check", "readback", "discard"])]
        admit: bool,
        /// Write the admitted formalization's provenance back onto the subject item (D14).
        #[arg(long, conflicts_with_all = ["set", "translate", "check", "readback", "admit", "discard"])]
        writeback: bool,
        /// Bind a vocabulary symbol to an observable (D13 grounding), as `SYMBOL=OBSERVABLE`
        /// (for category 1, the observable is the name of a function standing for the
        /// predicate, resolved against the subject's real source — not a search term).
        #[arg(long, value_name = "SYMBOL=OBSERVABLE", conflicts_with_all = ["set", "translate", "check", "readback", "admit", "writeback", "discard"])]
        ground: Option<String>,
        /// Fidelity for a `--ground` binding (definitional | observed | probed);
        /// defaults from the requirement's category.
        #[arg(long, value_name = "FIDELITY", requires = "ground")]
        fidelity: Option<String>,
        /// Dry-run the category-1 grounding bindings against the subject's real source
        /// (D13) and report whether the requirement grounds or stays parked.
        #[arg(long, conflicts_with_all = ["set", "translate", "check", "readback", "admit", "writeback", "ground", "discard"])]
        dry_run: bool,
        /// Reviewer name recorded on admission (defaults to $USER).
        #[arg(long, value_name = "NAME")]
        reviewer: Option<String>,
        /// Skip the confirmation prompt for a mandatory-review admit (for scripting).
        #[arg(long)]
        yes: bool,
        /// Discard this draft.
        #[arg(long, conflicts_with = "set")]
        discard: bool,
    },
    /// Show the requirement coverage funnel.
    Status {
        /// Path to the subject repository (defaults to the current directory).
        #[arg(default_value = ".")]
        path: PathBuf,
    },
    /// Report which verification engines are installed and which formalized
    /// requirements are therefore checkable (R-eng-2/3). Never installs anything.
    Engines {
        /// Path to the subject repository (defaults to the current directory).
        #[arg(default_value = ".")]
        path: PathBuf,
    },
    /// Produce the verdict for an admitted requirement (Step 4). Runs no engine yet —
    /// reports the honest three-valued verdict (always `unknown`) with provenance.
    Verify {
        /// Requirement item id (e.g. REQ001).
        id: String,
        /// Path to the subject repository (defaults to the current directory).
        #[arg(long, default_value = ".")]
        path: PathBuf,
        /// Draft the missing deductive marker (`#[logic]`/`#[pure]`) onto opaque predicate fns
        /// and stage it as an uncommitted working-tree edit for review (A6, REQ033). Never
        /// commits; the tool proposes, the operator reviews the diff and the verifier re-checks.
        #[arg(long)]
        draft_contracts: bool,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    match Cli::parse().command {
        Command::Serve { port, path } => {
            provreq::server::serve(port, path).await.map_err(Into::into)
        }
        Command::Init { path, name, yes } => run_init(&path, name.as_deref(), yes),
        Command::Triage { path, set } => run_triage(&path, set).await,
        Command::Draft {
            id,
            path,
            set,
            translate,
            check,
            readback,
            admit,
            writeback,
            ground,
            fidelity,
            dry_run,
            reviewer,
            yes,
            discard,
        } => {
            run_draft(
                &path,
                id.as_deref(),
                set.as_deref(),
                DraftActions {
                    translate,
                    check,
                    readback,
                    admit,
                    writeback,
                    ground,
                    fidelity,
                    dry_run,
                    reviewer,
                    yes,
                    discard,
                },
            )
            .await
        }
        Command::Status { path } => run_status(&path),
        Command::Engines { path } => run_engines(&path),
        Command::Verify {
            id,
            path,
            draft_contracts,
        } => run_verify(&path, &id, draft_contracts),
    }
}

async fn run_triage(subject: &Path, set: Option<Vec<String>>) -> Result<()> {
    let (companion, items) = resolve(subject)?;
    let state = triage::load(&companion)?;

    let state = match set {
        Some(args) => {
            // clap guarantees exactly two values for `--set`.
            let (id, bucket) = (&args[0], &args[1]);
            let classification = Classification::parse(bucket).with_context(|| {
                format!(
                    "unknown bucket '{bucket}' (formalizable-now | falsifiable-only | stays-prose)"
                )
            })?;
            let item = items
                .iter()
                .find(|i| &i.id == id)
                .with_context(|| format!("no requirement item '{id}' in the subject"))?;
            let next = triage::set(&state, item, classification);
            triage::save(&companion, &next)?;
            println!("Set {id} = {}", classification.as_str());
            next
        }
        None => seed_backlog(&companion, &state, &items).await?,
    };

    print_triage(&items, &state);
    Ok(())
}

/// Seed the pending backlog using the operator's configured LLM classifier, or
/// the honest prose-floor default when no `llm:` block is present.
async fn seed_backlog(
    companion: &Path,
    state: &TriageState,
    items: &[Item],
) -> Result<TriageState> {
    let next = match provreq::llm::load_config(companion)? {
        Some(config) => {
            println!(
                "Classifying backlog with {} via {} …",
                config.model, config.base_url
            );
            let classifier = LlmClassifier::new(HttpBackend::from_config(config)?);
            triage::seed(state, items, &classifier).await?
        }
        None => {
            println!("No `llm:` config in provreq.yml — seeding with the prose-floor default.");
            triage::seed(state, items, &ProseFloorClassifier).await?
        }
    };
    triage::save(companion, &next)?;
    Ok(next)
}

fn print_triage(items: &[Item], state: &TriageState) {
    println!("Triage ({} item(s)):", items.len());
    for item in items {
        let bucket = state
            .items
            .get(&item.id)
            .map(|e| e.classification.as_str())
            .unwrap_or("untriaged");
        println!("  {:<12} {bucket}", item.id);
    }
}

/// The one-shot actions `provreq draft` can take on a draft (mutually exclusive at the
/// CLI). Bundled so `run_draft` stays a small signature.
struct DraftActions {
    translate: bool,
    check: bool,
    readback: bool,
    admit: bool,
    writeback: bool,
    ground: Option<String>,
    fidelity: Option<String>,
    dry_run: bool,
    reviewer: Option<String>,
    yes: bool,
    discard: bool,
}

/// Open/resume, edit, translate, check, read back, admit, or discard the draft for one
/// item — or list all drafts when no id is given.
async fn run_draft(
    subject: &Path,
    id: Option<&str>,
    set: Option<&str>,
    actions: DraftActions,
) -> Result<()> {
    let (companion, items) = resolve(subject)?;
    let state = draft::load(&companion)?;

    let Some(id) = id else {
        return list_drafts(&state, &items);
    };
    let item = items
        .iter()
        .find(|i| i.id == id)
        .with_context(|| format!("no requirement item '{id}' in the subject"))?;

    let DraftActions {
        translate,
        check,
        readback,
        admit,
        writeback,
        ground,
        fidelity,
        dry_run,
        reviewer,
        yes,
        discard,
    } = actions;

    if check {
        return check_candidate(&companion, &state, id);
    }
    if let Some(spec) = ground.as_deref() {
        return ground_candidate(&companion, &state, id, spec, fidelity.as_deref());
    }
    if dry_run {
        return dry_run_candidate(subject, &companion, &state, id);
    }
    if readback {
        return readback_candidate(&state, id);
    }
    if admit {
        return admit_candidate(&companion, &state, id, reviewer.as_deref(), yes);
    }
    if writeback {
        return writeback_candidate(subject, &state, item);
    }
    if discard {
        let next = draft::discard(&state, id);
        draft::save(&companion, &next)?;
        println!("Discarded draft for {id}.");
        return Ok(());
    }
    if translate {
        // Forward-translate then run the gate, repairing on rejection (the loop
        // returns the final candidate with its verdict either way).
        let outcome = translate_gated_candidate(&companion, item).await?;
        let status = gate_to_status(&outcome.gate);
        let next = draft::set_candidate(&state, item, &outcome.candidate, status.clone());
        draft::save(&companion, &next)?;
        println!(
            "Translated {id} in {} attempt(s), baselined against {}.",
            outcome.attempts, item.revision
        );
        println!("Candidate PRL:\n{}", outcome.candidate);
        print_gate(&status);
        return Ok(());
    }
    if let Some(candidate) = set {
        // A hand-authored candidate is gated once (no repair — the operator owns it).
        let status = gate_to_status(&provreq::prl::gate(candidate));
        let next = draft::set_candidate(&state, item, candidate, status.clone());
        draft::save(&companion, &next)?;
        println!(
            "Saved draft candidate for {id} (baselined against {}).",
            item.revision
        );
        print_gate(&status);
        return Ok(());
    }

    // Open (if new) then resume: report the draft's state and any drift.
    let next = draft::open(&state, item);
    if next != state {
        draft::save(&companion, &next)?;
        println!("Opened draft for {id}.");
    }
    print_draft(&next.drafts[id], item);
    Ok(())
}

/// D11: ask the configured LLM to propose a candidate PRL for `item`, then run the
/// mechanical gate and repair on rejection. Requires an `llm:` block (translate has no
/// honest offline fallback the way triage does — the prose floor is not a formalization).
async fn translate_gated_candidate(
    companion: &Path,
    item: &Item,
) -> Result<provreq::formalize::RepairOutcome> {
    let config = provreq::llm::load_config(companion)?.context(
        "no `llm:` block in provreq.yml — configure a provider to use `draft --translate`",
    )?;
    println!(
        "Translating {} with {} via {} …",
        item.id, config.model, config.base_url
    );
    let translator = Translator::new(HttpBackend::from_config(config)?);
    translator.translate_gated(item).await
}

/// Re-run the mechanical gate over a draft's stored candidate and persist the fresh
/// outcome (only the gate field changes — a re-check is not an edit).
fn check_candidate(companion: &Path, state: &draft::DraftState, id: &str) -> Result<()> {
    let draft = state
        .drafts
        .get(id)
        .with_context(|| format!("no draft for {id} — open one first with `provreq draft {id}`"))?;
    let Some(candidate) = &draft.candidate else {
        println!("Draft {id} has no candidate PRL to check yet — write one with `--set` or `--translate`.");
        return Ok(());
    };
    let status = gate_to_status(&provreq::prl::gate(candidate));
    let next = draft::set_gate(state, id, status.clone());
    draft::save(companion, &next)?;
    print_gate(&status);
    Ok(())
}

/// D12: render the deterministic CNL read-back of a draft's candidate for the operator
/// to confirm intent. Read-only. Requires a gate pass — the read-back surfaces the
/// *formal meaning*, so a candidate the gate rejects has no settled meaning to render.
fn readback_candidate(state: &draft::DraftState, id: &str) -> Result<()> {
    let draft = state
        .drafts
        .get(id)
        .with_context(|| format!("no draft for {id} — open one first with `provreq draft {id}`"))?;
    let Some(candidate) = &draft.candidate else {
        println!("Draft {id} has no candidate PRL to read back yet — write one with `--set` or `--translate`.");
        return Ok(());
    };
    match provreq::prl::gate(candidate) {
        Ok(outcome) => {
            println!("Read-back for {id} — confirm this matches your intent:\n");
            println!("{}", provreq::prl::render(&outcome.requirement));
            if !outcome.warnings.is_empty() {
                println!(
                    "\nWeigh {} vacuity warning(s) while confirming:",
                    outcome.warnings.len()
                );
                for w in &outcome.warnings {
                    println!("  ! {w}");
                }
            }
        }
        Err(errors) => {
            println!(
                "Cannot read back {id} — the candidate has {} gate error(s); fix them first (run `--check`):",
                errors.len()
            );
            for e in &errors {
                println!("  - {e}");
            }
        }
    }
    Ok(())
}

/// D12: admit a draft's formalization after human confirmation. Requires a gate pass
/// (re-gated as the source of truth). Vacuity-flagged candidates are mandatory-review —
/// the read-back is shown and confirmation required (or `--yes` to script); a clean
/// candidate is optional-review and admits directly. Moves the draft to
/// `admitted-but-ungrounded`.
fn admit_candidate(
    companion: &Path,
    state: &draft::DraftState,
    id: &str,
    reviewer: Option<&str>,
    yes: bool,
) -> Result<()> {
    let draft = state
        .drafts
        .get(id)
        .with_context(|| format!("no draft for {id} — open one first with `provreq draft {id}`"))?;
    let Some(candidate) = &draft.candidate else {
        println!("Draft {id} has no candidate PRL to admit yet — write one with `--set` or `--translate`.");
        return Ok(());
    };

    let outcome = match provreq::prl::gate(candidate) {
        Ok(outcome) => outcome,
        Err(errors) => {
            println!(
                "Cannot admit {id} — the candidate has {} gate error(s); fix them first (run `--check`):",
                errors.len()
            );
            for e in &errors {
                println!("  - {e}");
            }
            return Ok(());
        }
    };

    // Vacuity warnings raise the review tier: those admissions are mandatory.
    let tier = if outcome.warnings.is_empty() {
        draft::ReviewTier::Optional
    } else {
        draft::ReviewTier::Mandatory
    };

    if tier == draft::ReviewTier::Mandatory {
        println!("Read-back for {id} — mandatory review (vacuity-flagged):\n");
        println!("{}", provreq::prl::render(&outcome.requirement));
        println!("\n{} vacuity warning(s) to weigh:", outcome.warnings.len());
        for w in &outcome.warnings {
            println!("  ! {w}");
        }
        if !yes && !confirm("\nAdmit this formalization?")? {
            println!("Not admitted.");
            return Ok(());
        }
    }

    let reviewer = reviewer
        .map(str::to_string)
        .unwrap_or_else(default_reviewer);
    let next = draft::admit(state, id, tier, &reviewer, now_unix());
    draft::save(companion, &next)?;
    println!(
        "Admitted {id} (review: {}, by {reviewer}) — admitted-but-ungrounded.",
        tier.as_str()
    );
    Ok(())
}

/// D14: write an admitted formalization's provenance back onto the subject item
/// (through the source adapter). Requires an admitted draft, and refuses a drifted one
/// — an admission against since-changed prose must be re-confirmed first. Mutates the
/// subject working tree; the operator reviews and commits the change.
fn writeback_candidate(subject: &Path, state: &draft::DraftState, item: &Item) -> Result<()> {
    let draft = state
        .drafts
        .get(&item.id)
        .with_context(|| format!("no draft for {} — nothing to write back", item.id))?;
    let draft::Admission::Admitted {
        review,
        by,
        at_unix,
    } = &draft.admission
    else {
        println!(
            "Draft {} is not admitted yet — admit it first with `--admit`.",
            item.id
        );
        return Ok(());
    };
    if draft::is_stale(draft, item) {
        println!(
            "Draft {} needs reconfirmation — the requirement prose moved since admission; \
             re-admit against the current text before writing back.",
            item.id
        );
        return Ok(());
    }
    let annotation = provreq::source::Annotation {
        status: "admitted-but-ungrounded".into(),
        prl: draft.candidate.clone().unwrap_or_default(),
        review: review.as_str().into(),
        reviewer: by.clone(),
        reviewed_at_unix: *at_unix,
        source_revision: draft.revision.clone(),
    };
    DoorstopSource::new(subject).annotate(&item.id, &annotation)?;
    println!(
        "Wrote formalization provenance onto {} — review the working-tree change and commit it.",
        item.id
    );
    Ok(())
}

/// D13: attach a grounding binding (`SYMBOL=OBSERVABLE`) to a draft. The candidate is
/// gated so the symbol is validated against the *declared* vocabulary — you cannot ground
/// a symbol the requirement does not speak of. Category and default fidelity come from the
/// requirement; `--fidelity` overrides. Grounding does not revoke admission.
fn ground_candidate(
    companion: &Path,
    state: &draft::DraftState,
    id: &str,
    spec: &str,
    fidelity: Option<&str>,
) -> Result<()> {
    let draft = state
        .drafts
        .get(id)
        .with_context(|| format!("no draft for {id} — open one first with `provreq draft {id}`"))?;
    let Some(candidate) = &draft.candidate else {
        println!("Draft {id} has no candidate PRL to ground yet — write one with `--set` or `--translate`.");
        return Ok(());
    };
    let (symbol, observable) = spec
        .split_once('=')
        .with_context(|| format!("--ground expects SYMBOL=OBSERVABLE, got `{spec}`"))?;
    let (symbol, observable) = (symbol.trim(), observable.trim());
    if symbol.is_empty() || observable.is_empty() {
        bail!("--ground expects a non-empty SYMBOL and OBSERVABLE, got `{spec}`");
    }

    let requirement = match provreq::prl::gate(candidate) {
        Ok(outcome) => outcome.requirement,
        Err(errors) => {
            println!(
                "Cannot ground {id} — the candidate has {} gate error(s); fix them first (run `--check`):",
                errors.len()
            );
            for e in &errors {
                println!("  - {e}");
            }
            return Ok(());
        }
    };

    if !grounding::is_bindable(&requirement, symbol) {
        let symbols = grounding::bindable_symbols(&requirement);
        bail!(
            "'{symbol}' is not a declared vocabulary symbol of {id}; \
             bindable symbols: {}",
            if symbols.is_empty() {
                "(none)".to_string()
            } else {
                symbols.join(", ")
            }
        );
    }

    let category = grounding::default_category(&requirement);
    let fidelity = match fidelity {
        Some(f) => grounding::Fidelity::parse(f).with_context(|| {
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
    let next = draft::set_binding(state, id, binding);
    draft::save(companion, &next)?;
    println!(
        "Grounded {symbol} → `{observable}` (category {}, {} fidelity). \
         Dry-run it with `provreq draft {id} --dry-run`.",
        category.as_label(),
        fidelity.as_str()
    );
    Ok(())
}

/// Live category-1 resolution lookup for a draft's bindings, keyed by symbol. The single
/// place the observable world is consulted, so `ground --dry-run` and `verify` can never
/// disagree about what grounds. Only category 1 (code) has a real observable world in this
/// slice; other categories are absent from the map and park in [`grounding::verdict`].
///
/// The arity checked against is the one the **requirement** declares for that predicate —
/// the binding is wrong if the two disagree, and which of them is at fault is the
/// operator's call, not this tool's.
/// The live resolution maps for a draft's bindings, one per observable world: category-1
/// predicates → functions and sorts → types (REQ025/REQ026), and category-2a symbols →
/// TLA+ definitions (REQ028). The cat-1 predicate/sort split is kept because a coincidental
/// cross-hit (a `struct login` standing in for the predicate `login`) must never ground
/// anything; category 2a needs no such split — TLA+ has one kind of name. Categories 2b/3
/// have no observable world wired yet, so their bindings are absent from every map and park
/// in [`grounding::verdict`].
/// D13: dry-run a draft's category-1 bindings against the subject's real source and
/// report whether the requirement grounds or stays parked. Read-only over the subject
/// (matches are recomputed live, never stored). Requires a gate pass — the bindings are
/// checked against the current formal meaning.
fn dry_run_candidate(
    subject: &Path,
    companion: &Path,
    state: &draft::DraftState,
    id: &str,
) -> Result<()> {
    let draft = state
        .drafts
        .get(id)
        .with_context(|| format!("no draft for {id} — open one first with `provreq draft {id}`"))?;
    let Some(candidate) = &draft.candidate else {
        println!("Draft {id} has no candidate PRL to dry-run yet — write one with `--set` or `--translate`.");
        return Ok(());
    };
    let requirement = match provreq::prl::gate(candidate) {
        Ok(outcome) => outcome.requirement,
        Err(errors) => {
            println!(
                "Cannot dry-run {id} — the candidate has {} gate error(s); fix them first (run `--check`):",
                errors.len()
            );
            for e in &errors {
                println!("  - {e}");
            }
            return Ok(());
        }
    };
    if draft.bindings.is_empty() {
        println!(
            "Draft {id} has no grounding bindings yet — attach one with \
             `provreq draft {id} --ground SYMBOL=OBSERVABLE`."
        );
        return Ok(());
    }

    // Live dry-run: categories 1 (code) and 2a (model) have real observable worlds. Each
    // binding reports what it resolved to (D13's "is that what you meant?"), which the
    // operator can only answer against a named observable at a named line.
    let (by_symbol, by_sort, by_model) =
        grounding::resolve_bindings(subject, companion, &requirement, &draft.bindings);
    for b in &draft.bindings {
        if let Some(r) = by_sort.get(&b.symbol) {
            println!("  {}", r.describe(&b.symbol, &b.observable));
        } else if let Some(r) = by_symbol.get(&b.symbol) {
            println!("  {}", r.describe(&b.symbol, &b.observable));
        } else if let Some(r) = by_model.get(&b.symbol) {
            println!("  {}", r.describe(&b.symbol, &b.observable));
        } else {
            println!(
                "  {} → `{}` (category {}): dry-run deferred — engine not wired yet",
                b.symbol,
                b.observable,
                b.category.as_label()
            );
        }
    }

    match grounding::verdict(
        &requirement,
        &draft.bindings,
        &by_symbol,
        &by_sort,
        &by_model,
    ) {
        Grounding::Grounded => {
            println!("\n{id}: GROUNDED — every symbol binds to a confirmed observable.");
        }
        Grounding::Parked { reasons } => {
            println!(
                "\n{id}: admitted-but-ungrounded (parked) — {} reason(s):",
                reasons.len()
            );
            for r in &reasons {
                println!("  - {r}");
            }
        }
    }
    Ok(())
}

/// The reviewer name recorded on an admission when `--reviewer` is not given: the
/// `$USER`/`$USERNAME` environment value, or `"unknown"`.
fn default_reviewer() -> String {
    std::env::var("USER")
        .or_else(|_| std::env::var("USERNAME"))
        .unwrap_or_else(|_| "unknown".to_string())
}

/// Current wall-clock time as Unix seconds (0 if the clock is before the epoch).
fn now_unix() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

/// Render a gate result into the persisted [`GateStatus`] (errors/warnings as strings).
fn gate_to_status(
    gate: &std::result::Result<provreq::prl::GateOutcome, Vec<provreq::prl::GateError>>,
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

/// Print a gate outcome for the operator.
fn print_gate(status: &GateStatus) {
    match status {
        GateStatus::Ungated => println!("Gate: not run."),
        GateStatus::Passed { warnings } if warnings.is_empty() => println!("Gate: PASSED (clean)."),
        GateStatus::Passed { warnings } => {
            println!(
                "Gate: PASSED with {} vacuity warning(s) — review before admitting:",
                warnings.len()
            );
            for w in warnings {
                println!("  ! {w}");
            }
        }
        GateStatus::Failed { errors } => {
            println!("Gate: FAILED ({} error(s)):", errors.len());
            for e in errors {
                println!("  - {e}");
            }
        }
    }
}

fn print_draft(d: &Draft, item: &Item) {
    if draft::is_stale(d, item) {
        println!(
            "Draft {} is STALE — the requirement moved (draft @ {}, source now @ {}); \
             re-confirm before continuing.",
            item.id, d.revision, item.revision
        );
    } else {
        println!(
            "Draft {} is up to date (baselined @ {}).",
            item.id, d.revision
        );
    }
    match &d.candidate {
        Some(prl) => {
            println!("Candidate PRL:\n{prl}");
            print_gate(&d.gate);
        }
        None => println!("No candidate PRL yet — write one with `--set` or `--translate`."),
    }
    if !d.bindings.is_empty() {
        println!(
            "Grounding: {} binding(s) — dry-run with `--dry-run`.",
            d.bindings.len()
        );
    }
    if let draft::Admission::Admitted { review, by, .. } = &d.admission {
        if draft::needs_reconfirmation(d, item) {
            println!(
                "Admitted (review: {}, by {by}) but NEEDS RECONFIRMATION — prose moved since admission; re-admit before writing back.",
                review.as_str()
            );
        } else {
            println!(
                "Admitted (review: {}, by {by}) — admitted-but-ungrounded.",
                review.as_str()
            );
        }
    }
}

fn list_drafts(state: &draft::DraftState, items: &[Item]) -> Result<()> {
    if state.drafts.is_empty() {
        println!("No drafts.");
        return Ok(());
    }
    println!("Drafts ({}):", state.drafts.len());
    for (id, d) in &state.drafts {
        let stale = items
            .iter()
            .find(|i| &i.id == id)
            .map(|i| draft::is_stale(d, i))
            .unwrap_or(false);
        let flag = if stale { " [STALE]" } else { "" };
        let has = if d.candidate.is_some() {
            "candidate"
        } else {
            "empty"
        };
        let gate = match &d.gate {
            GateStatus::Ungated => "",
            GateStatus::Passed { warnings } if warnings.is_empty() => " [gate ok]",
            GateStatus::Passed { .. } => " [gate ok, warnings]",
            GateStatus::Failed { .. } => " [gate failed]",
        };
        let admitted = if d.is_admitted() {
            if stale {
                " [admitted, needs-reconfirm]"
            } else {
                " [admitted]"
            }
        } else {
            ""
        };
        println!("  {id:<12} {has}{flag}{gate}{admitted}");
    }
    Ok(())
}

fn run_status(subject: &Path) -> Result<()> {
    let (companion, items) = resolve(subject)?;
    let triage_state = triage::load(&companion)?;
    let draft_state = draft::load(&companion)?;
    let cov = provreq::status::coverage(&items, &triage_state, &draft_state);
    println!("Coverage funnel:");
    println!("  discovered        {}", cov.discovered);
    println!("  untriaged         {}", cov.untriaged);
    println!("  formalizable-now  {}", cov.formalizable_now);
    println!("  falsifiable-only  {}", cov.falsifiable_only);
    println!("  stays-prose       {}", cov.stays_prose);
    println!("  drafting          {}", cov.drafting);
    println!("  formalized        {}", cov.formalized);
    println!(
        "  verified          {} (Step 4 — no engine wired yet, so every verdict is \
         `unknown`; see `provreq verify <ID>`)",
        cov.verified
    );
    Ok(())
}

/// R-eng-2/3: probe the verification engines and report which formalized requirements are
/// checkable given what is installed. Read-only; never installs an engine.
fn run_engines(subject: &Path) -> Result<()> {
    let (companion, items) = resolve(subject)?;
    let draft_state = draft::load(&companion)?;

    // Probe each engine once (R-eng-2) and keep the per-category statuses for coverage. A
    // category can have several engines (D2b), so statuses accumulate per category rather than
    // overwriting — readiness then needs only one of them ready.
    let mut status_by_category: std::collections::BTreeMap<
        grounding::BindCategory,
        Vec<engine::EngineStatus>,
    > = std::collections::BTreeMap::new();
    println!("Verification engines:");
    for e in engine::registry() {
        let status = engine::detect(&e);
        println!(
            "  category {:<3} {:<32} {}",
            e.category.as_label(),
            e.name,
            status.describe()
        );
        status_by_category
            .entry(e.category)
            .or_default()
            .push(status);
    }

    // Coverage of formalized (admitted) requirements (R-eng-3).
    let admitted: Vec<&Item> = items
        .iter()
        .filter(|i| {
            draft_state
                .drafts
                .get(&i.id)
                .map(Draft::is_admitted)
                .unwrap_or(false)
        })
        .collect();

    if admitted.is_empty() {
        println!("\nNo formalized (admitted) requirements yet — nothing to route.");
        return Ok(());
    }

    println!(
        "\nFormalized requirement coverage ({} admitted):",
        admitted.len()
    );
    let mut ready_count = 0usize;
    for item in &admitted {
        let draft = &draft_state.drafts[&item.id];
        // An admitted draft's candidate should gate; if it no longer does, it is reported
        // unroutable rather than silently skipped.
        let categories: Vec<grounding::BindCategory> = draft
            .candidate
            .as_deref()
            .and_then(|c| provreq::prl::gate(c).ok())
            .map(|o| {
                o.requirement
                    .category
                    .iter()
                    .copied()
                    .map(grounding::BindCategory::from)
                    .collect()
            })
            .unwrap_or_default();
        let r = engine::readiness(&item.id, &categories, &status_by_category);
        if r.ready {
            ready_count += 1;
        }
        let cats = if r.categories.is_empty() {
            "(none)".to_string()
        } else {
            r.categories
                .iter()
                .map(|c| c.as_label())
                .collect::<Vec<_>>()
                .join(" + ")
        };
        if r.ready {
            println!("  {:<12} category {cats:<10} engine-ready", item.id);
        } else {
            println!(
                "  {:<12} category {cats:<10} engine-blocked ({})",
                item.id,
                r.blockers.join("; ")
            );
        }
    }
    println!(
        "\nSummary: {ready_count} engine-ready, {} blocked.",
        admitted.len() - ready_count
    );
    Ok(())
}

/// Step 4: produce the honest verdict for an admitted requirement. Re-gates, re-runs the
/// live category-1 grounding dry-run, pins provenance, and renders the verdict. Runs no
/// engine yet, so the verdict is always `unknown` (no-engine when grounded,
/// missing-grounding when not).
fn run_verify(subject: &Path, id: &str, draft_contracts: bool) -> Result<()> {
    let (companion, items) = resolve(subject)?;
    let state = draft::load(&companion)?;
    let item = items
        .iter()
        .find(|i| i.id == id)
        .with_context(|| format!("no requirement item '{id}' in the subject"))?;

    let draft = state.drafts.get(id).with_context(|| {
        format!("no draft for {id} — formalize it first with `provreq draft {id}`")
    })?;
    if !draft.is_admitted() {
        println!("Draft {id} is not admitted yet — admit the formalization first with `--admit`.");
        return Ok(());
    }
    let Some(candidate) = &draft.candidate else {
        println!("Draft {id} has no candidate PRL to verify.");
        return Ok(());
    };
    let requirement = match provreq::prl::gate(candidate) {
        Ok(outcome) => outcome.requirement,
        Err(errors) => {
            println!(
                "Cannot verify {id} — the admitted candidate no longer gates ({} error(s)); re-check it:",
                errors.len()
            );
            for e in &errors {
                println!("  - {e}");
            }
            return Ok(());
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

    let provenance = provreq::verdict::Provenance {
        requirement_revision: draft.revision.clone(),
        subject_commit: subject_head_commit(subject),
        tool_version: env!("CARGO_PKG_VERSION").to_string(),
    };
    // Only a GROUNDED requirement reaches an engine: an unresolved binding means there is
    // nothing to check the claim through, and running an engine against it would answer a
    // question nobody asked (R-ground-1).
    let verdict = match &grounding_result {
        Grounding::Grounded => engine_verdict(
            subject,
            &companion,
            id,
            &requirement,
            &draft.bindings,
            &by_symbol,
            provenance,
        ),
        Grounding::Parked { .. } => {
            provreq::verdict::from_grounding(id, &grounding_result, provenance)
        }
    };
    println!("{}", provreq::verdict::render(&verdict));
    // An admitted draft whose source prose moved is worth flagging alongside the verdict.
    if draft::is_stale(draft, item) {
        println!(
            "  ! the requirement prose moved since admission — re-admit before trusting this verdict"
        );
    }
    // A6 proof-carrier draft channel (REQ033): on request, stage the missing deductive marker onto
    // opaque predicate fns so a deductive engine can then see inside them. Only a grounded
    // requirement has resolved predicates to annotate.
    if draft_contracts {
        if matches!(grounding_result, Grounding::Grounded) {
            stage_marker_drafts(subject, &by_symbol)?;
        } else {
            println!(
                "\n--draft-contracts: nothing to draft — {id} is not grounded, so no predicate \
                 resolves to a function to annotate."
            );
        }
    }
    Ok(())
}

/// Stage the A6 deductive-marker drafts into the subject's working tree (REQ033). Reads the target
/// marker from the subject's manifest, drafts it onto each resolved predicate fn that lacks it, and
/// writes the edits back as uncommitted changes for the operator to review. Runs no git.
fn stage_marker_drafts(subject: &Path, resolutions: &BTreeMap<String, Resolution>) -> Result<()> {
    use provreq::contract_draft::{apply_to_source, marker_for_subject, plan_markers};

    let manifest = std::fs::read_to_string(subject.join("Cargo.toml"))
        .with_context(|| format!("reading {}", subject.join("Cargo.toml").display()))?;
    let Some(marker) = marker_for_subject(&manifest) else {
        println!(
            "\n--draft-contracts: the subject depends on neither creusot-contracts nor \
             prusti-contracts, so there is no deductive marker to draft — add a contracts crate \
             first (this is REQ032's missing-dependency inconclusive)."
        );
        return Ok(());
    };

    // Load every file a resolved predicate lives in, so the planner can skip already-marked fns.
    let mut sources: BTreeMap<String, String> = BTreeMap::new();
    for res in resolutions.values() {
        if let Resolution::Resolved { at, .. } = res {
            if !sources.contains_key(&at.file) {
                let text = std::fs::read_to_string(subject.join(&at.file))
                    .with_context(|| format!("reading {}", subject.join(&at.file).display()))?;
                sources.insert(at.file.clone(), text);
            }
        }
    }

    let drafts = plan_markers(resolutions, marker, &sources);
    if drafts.is_empty() {
        println!(
            "\n--draft-contracts: every resolved predicate already carries {} — nothing to draft.",
            marker.attribute()
        );
        return Ok(());
    }

    // Group by file and apply, then write the edited source back into the working tree.
    let mut by_file: BTreeMap<String, Vec<_>> = BTreeMap::new();
    for d in &drafts {
        by_file.entry(d.file.clone()).or_default().push(d.clone());
    }
    for (file, file_drafts) in &by_file {
        let original = &sources[file];
        let edited = apply_to_source(original, file_drafts);
        std::fs::write(subject.join(file), edited).with_context(|| {
            format!("staging marker draft into {}", subject.join(file).display())
        })?;
    }

    println!(
        "\n--draft-contracts: staged {} `{}` marker(s) into the working tree for review:",
        drafts.len(),
        marker.attribute()
    );
    for d in &drafts {
        println!("  + {} above {}:{}", d.attribute, d.file, d.line);
    }
    println!(
        "  Review the working-tree diff and re-run `verify` — the tool staged an uncommitted edit \
         and ran no git; the draft is a proposal the verifier must re-check."
    );
    Ok(())
}

/// Ask the requirement's engine, and turn what it says into a verdict (REQ027/REQ029).
///
/// Dispatch is by engine name, not category, because category 1 is an **ensemble** (D2b):
/// Kani AND Creusot both run and their evidence is aggregated. Category 2a routes to TLC.
/// Each engine has its own lowering, and none silently inherits another's. 2b/3 have no wired
/// engine, so they never reach a branch here — they are `no_engine` at the gate below.
fn engine_verdict(
    subject: &Path,
    companion: &Path,
    id: &str,
    requirement: &Requirement,
    bindings: &[Binding],
    resolutions: &BTreeMap<String, Resolution>,
    provenance: provreq::verdict::Provenance,
) -> provreq::verdict::Verdict {
    let category = grounding::default_category(requirement);
    let engines = engine::engines_for(category);

    // The ensemble runs every engine that is ready; the others are reported but do not block,
    // as long as one can answer (D2b). No engine ready means nothing checked the property — an
    // honest no-engine that names who must act (wiring is ours, installing is the operator's).
    let ready: Vec<&engine::Engine> = engines
        .iter()
        .filter(|e| engine::detect(e).is_ready())
        .collect();
    if ready.is_empty() {
        let detail = engines
            .iter()
            .map(|e| {
                format!(
                    "category {} routes to {} — {}",
                    category.as_label(),
                    e.name,
                    engine::detect(e).describe()
                )
            })
            .collect();
        return provreq::verdict::no_engine(id, detail, provenance);
    }

    let mut evidence = Vec::new();
    for e in ready {
        println!("  running {}...", e.name);
        // Dispatch by engine, not category: a category may route to several engines (D2b) and
        // each has its own lowering. A ready engine with no lowering wired here is a gap in
        // provreq, recorded as inconclusive rather than silently skipped.
        let ev = match e.name {
            "Kani" => kani_evidence(subject, id, requirement, bindings, resolutions),
            "Creusot" => creusot_evidence(subject, id, requirement, bindings, resolutions),
            "Prusti" => prusti_evidence(subject, id, requirement, bindings, resolutions),
            "TLC (TLA+)" => tlc_evidence(subject, companion, id, requirement, bindings),
            other => provreq::verdict::Evidence::inconclusive(
                other,
                vec![format!(
                    "{other} probed as ready but has no lowering wired in provreq"
                )],
            ),
        };
        evidence.push(ev);
    }
    provreq::verdict::aggregate(id, evidence, provenance)
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
) -> provreq::verdict::Evidence {
    let Some(crate_name) = provreq::kani::subject_crate_name(subject) else {
        return provreq::verdict::Evidence::inconclusive(
            "Kani",
            vec![
                "the subject is not a cargo crate (`cargo metadata` found no package), so a \
                 Kani harness has nothing to import"
                    .to_string(),
            ],
        );
    };
    let harness = match provreq::kani::lower(
        requirement,
        &crate_name,
        bindings,
        resolutions,
        &provreq::kani::harness_name(id),
    ) {
        Ok(h) => h,
        Err(e) => return provreq::verdict::Evidence::inconclusive("Kani", vec![e.reason]),
    };
    provreq::kani::run(subject, &harness).into_evidence()
}

/// Category 1 → Creusot (REQ031): the ensemble's deductive member. Lower to an additive
/// in-crate proof harness, run it, map to evidence. Unlike Kani it needs no crate name (the
/// harness lives inside the subject crate and reaches it via `crate::`). A claim that cannot
/// be faithfully lowered — or a subject with no crate root — is honest `inconclusive`, never
/// approximated (D2). A pass earns `proven`; an unproved goal is `inconclusive`, never a
/// witnessed `fails`.
fn creusot_evidence(
    subject: &Path,
    id: &str,
    requirement: &Requirement,
    bindings: &[Binding],
    resolutions: &BTreeMap<String, Resolution>,
) -> provreq::verdict::Evidence {
    let harness = match provreq::creusot::lower(
        requirement,
        bindings,
        resolutions,
        &provreq::creusot::harness_name(id),
    ) {
        Ok(h) => h,
        Err(e) => return provreq::verdict::Evidence::inconclusive("Creusot", vec![e.reason]),
    };
    provreq::creusot::run(subject, &harness).into_evidence()
}

/// Category 1 → Prusti (REQ032): the ensemble's second deductive member. Lower to an additive
/// in-crate proof harness, run it, map to evidence. Like Creusot the harness lives inside the
/// subject crate and reaches it via `crate::`; unlike Creusot it needs no prover config, but it
/// does need the subject to already depend on `prusti-contracts` (a subject that uses Prusti
/// has it) — a subject without it is honest `inconclusive`, never approximated (D2). A pass earns
/// `proven`; an undischarged obligation is `inconclusive`, never a witnessed `fails`.
fn prusti_evidence(
    subject: &Path,
    id: &str,
    requirement: &Requirement,
    bindings: &[Binding],
    resolutions: &BTreeMap<String, Resolution>,
) -> provreq::verdict::Evidence {
    let harness = match provreq::prusti::lower(
        requirement,
        bindings,
        resolutions,
        &provreq::prusti::harness_name(id),
    ) {
        Ok(h) => h,
        Err(e) => return provreq::verdict::Evidence::inconclusive("Prusti", vec![e.reason]),
    };
    provreq::prusti::run(subject, &harness).into_evidence()
}

/// Category 2a → TLC (REQ029): locate the subject's `Spec`, lower to an additive TLA+ module
/// with a temporal property, run TLC beside the spec, map to evidence. A missing `Spec` or an
/// un-lowerable claim is honestly `inconclusive`, never approximated (D2).
fn tlc_evidence(
    subject: &Path,
    companion: &Path,
    id: &str,
    requirement: &Requirement,
    bindings: &[Binding],
) -> provreq::verdict::Evidence {
    let site = match provreq::tlc::locate_spec(subject, companion) {
        Ok(site) => site,
        Err(reason) => return provreq::verdict::Evidence::inconclusive("TLC (TLA+)", vec![reason]),
    };
    let check = match provreq::tlc::lower(
        requirement,
        &site.module,
        bindings,
        &provreq::tlc::module_name(id),
    ) {
        Ok(c) => c,
        Err(e) => return provreq::verdict::Evidence::inconclusive("TLC (TLA+)", vec![e.reason]),
    };
    provreq::tlc::run(&site, &check).into_evidence()
}

/// Best-effort subject git HEAD for verdict provenance (D9). `None` when the subject is not
/// a git repo — never fabricated.
fn subject_head_commit(subject: &Path) -> Option<String> {
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

fn run_init(subject: &Path, name: Option<&str>, yes: bool) -> Result<()> {
    let docs = provreq::doorstop::discover(subject)?;
    if docs.is_empty() {
        bail!(
            "no Doorstop documents (.doorstop.yml) found under {}",
            subject.display()
        );
    }
    let plan = provreq::adopt::plan(&docs, name)?;

    println!("Discovered Doorstop layout under {}:", subject.display());
    for d in &plan.docs {
        println!(
            "  {} ({}) — {} item(s)",
            d.dir.display(),
            d.prefix,
            d.item_ids.len()
        );
    }
    println!("Proposed companion tree: {}", plan.companion_root.display());

    if !yes && !confirm("Create companion tree?")? {
        println!("Aborted; nothing written.");
        return Ok(());
    }

    let created = provreq::adopt::scaffold(&plan)?;
    println!("Created companion tree at {}", created.display());
    Ok(())
}

fn confirm(prompt: &str) -> Result<bool> {
    print!("{prompt} [y/N] ");
    io::stdout().flush().context("flushing stdout")?;
    let mut line = String::new();
    io::stdin()
        .read_line(&mut line)
        .context("reading confirmation")?;
    Ok(matches!(
        line.trim().to_ascii_lowercase().as_str(),
        "y" | "yes"
    ))
}
