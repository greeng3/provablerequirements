use anyhow::{bail, Context, Result};
use clap::{Parser, Subcommand};
use provreq::adopt::find_companion;
use provreq::doorstop::DoorstopSource;
use provreq::draft::{self, Draft, GateStatus};
use provreq::formalize::Translator;
use provreq::grounding::{self, Binding, Grounding};
use provreq::llm::{HttpBackend, LlmClassifier};
use provreq::source::{Classification, Item, RequirementsSource};
use provreq::triage::{self, ProseFloorClassifier, TriageState};
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
        /// (for category 1, the observable is a code search term).
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
}

#[tokio::main]
async fn main() -> Result<()> {
    match Cli::parse().command {
        Command::Serve { port } => provreq::server::serve(port).await.map_err(Into::into),
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
    }
}

/// Resolve the adopted companion root + source items for a subject, or explain
/// that `init` must run first.
fn resolve(subject: &Path) -> Result<(PathBuf, Vec<Item>)> {
    let companion = find_companion(subject)?.with_context(|| {
        format!(
            "no companion tree found under {} — run `provreq init` first",
            subject.display()
        )
    })?;
    let items = DoorstopSource::new(subject).items()?;
    Ok((companion, items))
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

    // Live dry-run: only category 1 has a real observable world in this slice.
    let mut counts = std::collections::BTreeMap::new();
    for b in &draft.bindings {
        if b.category != grounding::BindCategory::Code {
            continue;
        }
        let matches = grounding::dry_run_code(subject, companion, &b.observable);
        counts.insert(b.symbol.clone(), matches.len());
        println!(
            "{} → `{}` (category {}): {} match(es){}",
            b.symbol,
            b.observable,
            b.category.as_label(),
            matches.len(),
            if matches.len() >= grounding::DRY_RUN_MATCH_CAP {
                " (capped)"
            } else {
                ""
            }
        );
        for m in &matches {
            println!("    {}:{}  {}", m.file, m.line, m.text);
        }
    }

    match grounding::verdict(&requirement, &draft.bindings, &counts) {
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
        "  verified          {} (Step 4 — not built yet)",
        cov.verified
    );
    Ok(())
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
