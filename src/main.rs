use anyhow::{bail, Context, Result};
use clap::{Parser, Subcommand};
use provreq::adopt::resolve;
use provreq::draft::{self, Draft, GateStatus};
use provreq::engine;
use provreq::formalize::Translator;
use provreq::grounding::{self, Binding, Grounding};
use provreq::llm::{LlmClassifier, RuntimeBackend};
use provreq::rust_adapter::Resolution;
use provreq::source::{Classification, Item};
use provreq::triage::{self, ProseFloorClassifier, TriageState};
use provreq::verify::VerifyOutcome;
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
    ///
    /// Single-operator by design: one subject, bound to loopback, with no auth or tenancy. Serving
    /// several repositories or several people is out of scope, not unimplemented.
    Serve {
        /// TCP port to bind on the loopback interface.
        ///
        /// 17869 rather than something memorable: 8080 is the first port every other tool on a
        /// dev box reaches for, and a default that collides is a default that wastes the
        /// operator's afternoon. This sits in the range the repo already reserves — 17867 is
        /// qrusty's devcontainer, 17868 is doorstop-server, and this is the next one.
        #[arg(long, default_value_t = 17869)]
        port: u16,
        /// Path to the one subject repository this process serves (defaults to the current
        /// directory).
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
        /// Re-run the classifier over items that are already triaged, replacing their
        /// classifications. Without this, seeding is additive and a fully-triaged backlog is left
        /// alone — so this is the way back out of a seeding you no longer want.
        #[arg(long)]
        reclassify: bool,
        /// Skip the confirmation prompt for `--reclassify` (for scripting).
        #[arg(long)]
        yes: bool,
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
    /// Traceability report: per requirement, whether it is formalized, what code implements and
    /// verifies it, and the last stored verdict (proven / not-determined / disproven) with its
    /// mechanical-or-asserted correspondence.
    Report {
        /// Path to the subject repository (defaults to the current directory).
        #[arg(default_value = ".")]
        path: PathBuf,
        /// Output format.
        #[arg(long, default_value = "text")]
        format: String,
    },
    /// Report which verification engines are installed and which formalized
    /// requirements are therefore checkable (R-eng-2/3). Never installs anything.
    Engines {
        /// Path to the subject repository (defaults to the current directory).
        #[arg(default_value = ".")]
        path: PathBuf,
    },
    /// Provision a verification engine natively into your dev env (R-eng-2 install half, REQ046).
    /// Consent-gated: without `--yes` it prints the plan and stops. Light tier only — `tlc` and
    /// `kani` (Linux/macOS). An engine provreq will not install natively is explained in terms of
    /// `--path`'s own build environment (REQ048); an unwired one says so plainly.
    Install {
        /// Which engine to install (`tlc` or `kani`).
        engine: String,
        /// Consent to the install actions the plan describes (download + write).
        #[arg(long)]
        yes: bool,
        /// Subject repository, read only to explain an engine provreq will not install natively
        /// in terms of that subject's own build environment (REQ048).
        #[arg(long, default_value = ".")]
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
        /// Ask the configured LLM to draft `#[requires]`/`#[ensures]` clauses onto the resolved
        /// predicate fns and stage them as an uncommitted working-tree edit (A6, REQ040). Opt-in
        /// and separate from `--draft-contracts`; the two compose. Requires an `llm:` block in
        /// provreq.yml. The clauses are an untrusted proposal — the verifier re-checks them.
        #[arg(long)]
        draft_semantic: bool,
        /// With `--draft-semantic`, verify each drafted contract against the real prover and repair
        /// it on the prover's feedback, up to a bounded number of rounds (REQ041). Runs the engine
        /// (slower, needs it installed); degrades to the one-shot draft when no engine is ready.
        #[arg(long, requires = "draft_semantic")]
        repair: bool,
    },
    /// Convert a Doorstop requirements tree into a ReqForge project (#317). Reads `source` only and
    /// writes a `reqforge.json` + an `artifacts/` collection under `target`, which
    /// `provreq` then reads through the ReqForge adapter. Ids are preserved verbatim, so a subject's
    /// verdicts, drafts, and code references keep pointing at the same items.
    MigrateDoorstop {
        /// The Doorstop tree to read (the directory holding the `.doorstop.yml` documents).
        source: PathBuf,
        /// Where to write the ReqForge project (created if absent). Must not already hold the
        /// collection prefixes being imported.
        #[arg(long)]
        target: PathBuf,
        /// Project slug for `reqforge.json`.
        #[arg(long)]
        slug: String,
        /// Human-readable project name for `reqforge.json`.
        #[arg(long)]
        name: String,
    },
    /// Validate the subject's ReqForge requirements project (#323) — the analogue of `doorstop -e`.
    /// Every artifact must load, collection configs must be present and valid, and no two artifacts
    /// may share a uuid. Exits non-zero, reporting each problem, if the project does not validate.
    Check {
        /// Path to the subject repository (defaults to the current directory).
        #[arg(default_value = ".")]
        path: PathBuf,
    },
    /// Author a new requirement into the subject's ReqForge collection (#325). The artifact arrives
    /// unreviewed — authored prose passes through the review workflow like any other.
    New {
        /// Requirement id — the artifact's filename stem and the id provreq reads (e.g. REQ074).
        id: String,
        /// One-line title.
        #[arg(long)]
        title: String,
        /// The requirement prose (the artifact body).
        #[arg(long)]
        text: String,
        /// Path to the subject repository (defaults to the current directory).
        #[arg(long, default_value = ".")]
        path: PathBuf,
    },
}

/// Whether an argument reads as a subject path rather than a mistyped flag or a bad id (REQ056).
/// Deliberately narrow: `.`/`..`, anything with a separator, or a directory that actually exists.
/// A wrong guess here would attach a confident, irrelevant hint to an unrelated error.
fn looks_like_a_path(arg: &str) -> bool {
    if arg.starts_with('-') {
        return false;
    }
    arg == "." || arg == ".." || arg.contains(std::path::MAIN_SEPARATOR) || Path::new(arg).is_dir()
}

/// The path-shaped argument behind an "unexpected argument" error, if that is what happened.
fn stray_path_argument(err: &clap::Error) -> Option<String> {
    if err.kind() != clap::error::ErrorKind::UnknownArgument {
        return None;
    }
    let arg = err
        .get(clap::error::ContextKind::InvalidArg)?
        .to_string()
        .trim_matches('\'')
        .to_string();
    looks_like_a_path(&arg).then_some(arg)
}

/// Parse the CLI, adding the one thing clap cannot say for itself (REQ056).
///
/// The positional slot is each command's own primary object, so it holds the subject path for
/// `init`/`triage`/`status`/`engines` and an id or engine name for `verify`/`draft`/`install`,
/// where the subject moves to `--path`. That rule is coherent, but the habit `status .` builds
/// carries straight into `verify REQ047 .` — and `Usage: provreq verify [OPTIONS] <ID>` names the
/// problem without naming the fix, because `--path` is hidden inside `[OPTIONS]`.
fn parse_cli() -> Cli {
    match Cli::try_parse() {
        Ok(cli) => cli,
        Err(err) => {
            let Some(arg) = stray_path_argument(&err) else {
                err.exit()
            };
            let _ = err.print();
            eprintln!(
                "hint: `{arg}` looks like a subject path. This command's positional argument is \
                 its own subject — an id, or an engine name — so the repository goes in a flag: \
                 `--path {arg}`."
            );
            std::process::exit(err.exit_code());
        }
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    match parse_cli().command {
        Command::Serve { port, path } => {
            provreq::server::serve(port, path).await.map_err(Into::into)
        }
        Command::Init { path, name, yes } => run_init(&path, name.as_deref(), yes),
        Command::Triage {
            path,
            set,
            reclassify,
            yes,
        } => run_triage(&path, set, reclassify, yes).await,
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
        Command::Report { path, format } => run_report(&path, &format),
        Command::Engines { path } => run_engines(&path),
        Command::Install { engine, yes, path } => run_install(&engine, yes, &path).await,
        Command::Verify {
            id,
            path,
            draft_contracts,
            draft_semantic,
            repair,
        } => run_verify(&path, &id, draft_contracts, draft_semantic, repair).await,
        Command::MigrateDoorstop {
            source,
            target,
            slug,
            name,
        } => run_migrate_doorstop(&source, &target, &slug, &name),
        Command::Check { path } => run_check(&path),
        Command::New {
            id,
            title,
            text,
            path,
        } => run_new(&path, &id, &title, &text),
    }
}

fn run_check(subject: &Path) -> Result<()> {
    let count = provreq::check::check(subject)?;
    println!("Requirements project OK — {count} artifact(s) validated.");
    Ok(())
}

fn run_new(subject: &Path, id: &str, title: &str, text: &str) -> Result<()> {
    let path = provreq::create::create(subject, id, title, text)?;
    println!(
        "Authored {id} at {} — unreviewed. Review the working-tree change and commit it.",
        path.display()
    );
    Ok(())
}

fn run_migrate_doorstop(source: &Path, target: &Path, slug: &str, name: &str) -> Result<()> {
    let report = provreq::migrate::migrate_doorstop(source, target, slug, name)?;
    println!(
        "Imported {} collection(s), {} artifact(s) into {}",
        report.totals.collections_created,
        report.totals.artifacts_imported,
        target.display()
    );
    Ok(())
}

async fn run_triage(
    subject: &Path,
    set: Option<Vec<String>>,
    reclassify: bool,
    yes: bool,
) -> Result<()> {
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
        None => seed_backlog(subject, &companion, &state, &items, reclassify, yes).await?,
    };

    print_triage(&items, &state);
    Ok(())
}

/// Seed the pending backlog using the operator's configured LLM classifier, or
/// the honest prose-floor default when no `llm:` block is present.
async fn seed_backlog(
    subject: &Path,
    companion: &Path,
    state: &TriageState,
    items: &[Item],
    reclassify: bool,
    yes: bool,
) -> Result<TriageState> {
    // Let the record answer before a model is asked (#265). An item with a stored verdict, or with
    // an admitted formalization, has demonstrably been lowered already — that is not a question for
    // a classifier, and asking one invites it to contradict our own store, which is exactly what
    // happened to REQ047 in #258's measurement. Applied first so `plan` below sees these entries
    // and leaves them out of the batch.
    let verdicts = provreq::verdict_store::load(companion)?;
    let drafts = provreq::draft::load(companion)?;
    let demonstrated = |item: &Item| {
        verdicts.verdicts.contains_key(&item.id)
            || provreq::draft::admitted_fingerprint(&drafts, &item.id).is_some()
    };
    let (with_record, read_off_the_record) = triage::apply_demonstrated(state, items, demonstrated);
    let state = &with_record;
    if !read_off_the_record.is_empty() {
        println!(
            "{} item(s) read off the record as formalizable-now ({}) — each carries a stored \
             verdict or an admitted formalization, so no classifier was asked about them.",
            read_off_the_record.len(),
            read_off_the_record.join(", ")
        );
        triage::save(companion, state)?;
    }

    // Decide the scope BEFORE describing it (REQ053). Announcing a model and then classifying
    // nothing is output that describes an action rather than reporting one.
    let pending = match triage::plan(state, items, reclassify) {
        triage::TriagePlan::Nothing { already } => {
            // The same emptiness means opposite advice by flag (#257): without `--reclassify` the
            // way forward is the flag; with it, everything left is the operator's own choice, and
            // offering the flag again would advertise a run that can never do anything.
            if reclassify {
                println!(
                    "all {already} item(s) are operator-set; `--reclassify` never replaces an \
                     operator's choice — change one with `provreq triage --set <ID> <bucket>`."
                );
            } else {
                println!(
                    "{already} item(s) already triaged; nothing to classify — re-run the \
                     classifier over them with `--reclassify`."
                );
            }
            return Ok(state.clone());
        }
        triage::TriagePlan::Classify {
            pending,
            operator_kept,
            demonstrated_kept,
        } => {
            // Said before the consent prompt, so what the operator consents to is what will
            // happen — a count that quietly included their own entries gated the wrong question.
            if !operator_kept.is_empty() {
                println!(
                    "keeping {} operator-set item(s) as they are ({}) — `--reclassify` never \
                     replaces an operator's choice; change one with `provreq triage --set`.",
                    operator_kept.len(),
                    operator_kept.join(", ")
                );
            }
            // Said apart from the operator's own entries (#257): these were kept because the
            // record already answers them, which is a different fact and a different remedy.
            if !demonstrated_kept.is_empty() {
                println!(
                    "keeping {} item(s) the record already answers ({}) — a stored verdict or an \
                     admitted formalization demonstrates these, so `--reclassify` does not re-ask \
                     a model about them.",
                    demonstrated_kept.len(),
                    demonstrated_kept.join(", ")
                );
            }
            pending
        }
    };
    let count = pending.len();

    // Re-classifying replaces the classifier's own judgements, so it is consent-gated like every
    // other action that overwrites recorded state. Operator-set entries are already out of
    // `pending` (#257), so the count here is exactly what the run will touch.
    if reclassify
        && !yes
        && !confirm(&format!(
            "Re-classify all {count} item(s), replacing the existing classifications?"
        ))?
    {
        println!("Aborted; nothing written.");
        return Ok(state.clone());
    }

    // Persisting each batch as it lands is what makes a failure cost one batch instead of the
    // whole run, and what makes the next run a resume (REQ054).
    let persist = |state: &TriageState, done: usize, total: usize| -> Result<()> {
        triage::save(companion, state)?;
        println!("  classified {done} of {total} …");
        Ok(())
    };

    let outcome = match provreq::llm::load_config(companion)? {
        Some(config) => {
            let batch_size = config.batch_size;
            println!(
                "Classifying {count} of {} item(s) with {} via {}{}, {batch_size} at a time …",
                items.len(),
                config.model,
                config.base_url,
                config.override_note()
            );
            // What the subject declares, so the classifier judges bindability rather than
            // guessing it from prose (REQ072, #259).
            let parsed = provreq::rust_adapter::ParsedSubject::load(subject, companion);
            let inv = provreq::rust_adapter::inventory(&parsed);
            let context = provreq::llm::SubjectContext {
                predicates: inv.predicates,
                sorts: inv.sorts,
            };
            let classifier = LlmClassifier::new(RuntimeBackend::from_config(config)?, context);
            triage::seed_in_batches(state, &pending, &classifier, batch_size, persist).await?
        }
        None => {
            println!(
                "No `llm:` config in provreq.yml — seeding {count} of {} item(s) with the \
                 prose-floor default. A seed is recorded as a seed, not as a classification: \
                 configure a provider and re-run `provreq triage` and these are re-done, with no \
                 `--reclassify` and nothing else of yours touched.",
                items.len()
            );
            triage::seed_in_batches(state, &pending, &ProseFloorClassifier, count, persist).await?
        }
    };

    // A run that stopped early already persisted what it managed; say what did and did not get
    // classified rather than letting the failure imply nothing happened.
    if let Some(stopped) = outcome.stopped {
        return Err(stopped).with_context(|| {
            format!(
                "classified {} of {count} item(s); {} not classified and left as they were — \
                 re-run `provreq triage` to resume from here",
                outcome.classified, outcome.unclassified
            )
        });
    }
    Ok(outcome.state)
}

/// List the backlog with each item's bucket — and, where the bucket is worth less than it looks,
/// what produced it (#172). A `stays-prose` that no classifier ever judged is the lifecycle state
/// meaning *this will not be formalized*, reached because nothing could decide; on the surface where
/// the operator reads it, that has to be visible.
fn print_triage(items: &[Item], state: &TriageState) {
    println!("Triage ({} item(s)):", items.len());
    for item in items {
        let entry = state.items.get(&item.id);
        let bucket = entry
            .map(|e| e.classification.as_str())
            .unwrap_or("untriaged");
        let note = entry.map(|e| e.origin.note()).unwrap_or_default();
        if note.is_empty() {
            println!("  {:<12} {bucket}", item.id);
        } else {
            println!("  {:<12} {bucket:<18} ({note})", item.id);
        }
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
        "Translating {} with {} via {}{} …",
        item.id,
        config.model,
        config.base_url,
        config.override_note()
    );
    let translator = Translator::new(RuntimeBackend::from_config(config)?);
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
    // Through the seam, not the Doorstop adapter directly: a ReqForge-sourced subject must get
    // that adapter's honest refusal rather than a Doorstop lookup failing for a file that was never
    // going to be there (#296).
    provreq::adopt::source_for(&provreq::adopt::requirements_root(subject))
        .annotate(&item.id, &annotation)?;
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
    // "Bound", not "Grounded": this attached a binding, and whether it *grounds* is a question only
    // the resolvers can answer. The old wording announced "Grounded checkout → `chekout`" for a
    // typo that parks the moment anything looks at it — telling the operator the opposite of what
    // had happened, in the one message they read before moving on.
    println!(
        "Bound {symbol} → `{observable}` (category {}, {} fidelity). \
         Whether it resolves is what `provreq draft {id} --dry-run` answers.",
        category.as_label(),
        fidelity.as_str()
    );
    Ok(())
}

/// Live resolution lookup for a draft's bindings, keyed by symbol. The single place the observable
/// worlds are consulted, so `--dry-run` and `verify` can never disagree about what grounds.
///
/// Every category has a real observable world now — code (REQ025), TLA+ (REQ028), the declared
/// event signature (#231), and the declared UI steps (#241) — so the "only category 1 is wired"
/// this comment used to carry is four slices out of date.
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
    let resolved = grounding::resolve_bindings(subject, companion, &requirement, &draft.bindings);
    for b in &draft.bindings {
        println!("  {}", resolved.describe(b).1);
    }

    print_monitor_claim(subject, companion, &requirement, &draft.bindings);
    print_ui_script(companion, &requirement, &draft.bindings);

    match grounding::verdict(&requirement, &draft.bindings, &resolved) {
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

fn run_report(subject: &Path, format: &str) -> Result<()> {
    let report = provreq::report::build(subject)?;
    match format {
        "json" => println!("{}", serde_json::to_string_pretty(&report)?),
        "text" => print!("{}", provreq::report::render_text(&report)),
        other => bail!("unknown --format `{other}` (expected `text` or `json`)"),
    }
    Ok(())
}

fn run_status(subject: &Path) -> Result<()> {
    let (companion, items) = resolve(subject)?;
    let triage_state = triage::load(&companion)?;
    let draft_state = draft::load(&companion)?;
    let verdicts = provreq::verdict_store::load(&companion)?;
    let anchor = provreq::verdict_store::DriftAnchor::current(
        provreq::verify::subject_head_commit(subject),
        provreq::proving_env::ProvingEnv::current(&companion),
        provreq::verify::current_fingerprints(subject, &companion),
    );
    let cov = provreq::status::coverage(&items, &triage_state, &draft_state, &verdicts, &anchor);
    println!("Coverage funnel:");
    println!("  discovered        {}", cov.discovered);
    println!("  untriaged         {}", cov.untriaged);
    println!("  formalizable-now  {}", cov.formalizable_now);
    println!("  falsifiable-only  {}", cov.falsifiable_only);
    println!("  stays-prose       {}", cov.stays_prose);
    println!("  drafting          {}", cov.drafting);
    println!("  formalized        {}", cov.formalized);
    println!(
        "  verified          {} (Step 6 — fresh `holds` verdicts; a drifted verdict drops out \
         until re-verified with `provreq verify <ID>`)",
        cov.verified
    );
    println!(
        "  stale             {} (Step 6 — stored verdicts that have drifted, any polarity; \
         re-verify with `provreq verify <ID>`)",
        cov.stale
    );
    // The count above names no item, and the line above it tells the operator to run
    // `provreq verify <ID>` — so the worklist follows it, on the same surface that asked for it
    // (#179). Printed only when there is work: a subject with nothing stale gets no empty heading.
    let stale =
        provreq::status::stale_worklist(&items, &triage_state, &draft_state, &verdicts, &anchor);
    if !stale.is_empty() {
        println!("\nRe-verify worklist ({} stale):", stale.len());
        for state in &stale {
            let Some(view) = &state.verdict else { continue };
            println!("  {:<12} last verdict: {}", state.id, view.status);
            for reason in &view.stale_reasons {
                println!("      {reason}");
            }
        }
        println!("\n  Re-verify with `provreq verify <ID>`.");
    }
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
    // A missing engine is only actionable once the operator knows which tier it is in and what
    // this subject's build environment offers (REQ048), so collect that alongside the probe.
    let mut missing_light: Vec<(String, &'static str)> = Vec::new();
    let mut missing_heavy: Vec<String> = Vec::new();
    // Engines that are installed but cannot start (REQ051). Kept out of the missing buckets on
    // purpose: they are already on disk, so an install command is the wrong advice for them.
    let mut unusable: Vec<(String, String)> = Vec::new();
    // Collected so the proving-environment record (REQ049) is built from the statuses already
    // probed here, rather than spawning every probe a second time.
    let mut probed: Vec<(&'static str, engine::EngineStatus)> = Vec::new();

    println!("Verification engines:");
    for e in engine::registry() {
        let status = engine::detect(&e, Some(&companion));
        probed.push((e.name, status.clone()));
        println!(
            "  category {:<3} {:<32} {}",
            e.category.as_label(),
            e.name,
            status.describe()
        );
        // `NotWired` is ours to fix by wiring the engine, never the operator's to install, so it
        // is not something a build environment can answer.
        match &status {
            engine::EngineStatus::Missing => match provreq::provision::native_install_arg(e.name) {
                Some(arg) => missing_light.push((e.name.to_string(), arg)),
                None => missing_heavy.push(e.name.to_string()),
            },
            engine::EngineStatus::Unusable { reason } => {
                unusable.push((e.name.to_string(), reason.clone()))
            }
            _ => {}
        }
        status_by_category
            .entry(e.category)
            .or_default()
            .push(status);
    }

    // An installed engine that cannot start gets its own advice (REQ051): it is already on disk,
    // so every install path above is the wrong door for it.
    if !unusable.is_empty() {
        println!("\nEngines that are installed but cannot start:");
        for (name, reason) in &unusable {
            println!("  {name}: {reason}");
        }
        println!(
            "  Installing these again will not help — they are already present. Repair the \
             environment they run in (a stale dev-container is the usual cause; re-pull it)."
        );
    }

    // Two different facts, deliberately reported apart: what the SUBJECT offers (REQ048) and what
    // this run would actually prove a verdict in (REQ049). Conflating them would let a verdict
    // proved on the host claim the subject's dev-container as its provenance.
    println!(
        "\nVerification environment (what a verdict produced now would record):\n  {}",
        provreq::proving_env::ProvingEnv::from_statuses(
            provreq::proving_env::declared_label(&companion),
            provreq::proving_env::in_container(),
            &probed,
        )
        .describe()
    );

    let build_env = provreq::buildenv::detect(subject);
    println!("\nBuild environment (what this subject offers):");
    println!("  {}", build_env.describe());
    for line in provreq::buildenv::advice(&build_env, &missing_light, &missing_heavy) {
        println!("  {line}");
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

/// The registry engine an `install` argument names, if any. `tlc` has to match `TLC (TLA+)`, so a
/// bare-word prefix counts; nothing else in the registry is ambiguous under that rule.
fn registry_engine_named(arg: &str) -> Option<engine::Engine> {
    let arg = arg.to_ascii_lowercase();
    engine::registry().into_iter().find(|e| {
        let name = e.name.to_ascii_lowercase();
        name == arg || name.starts_with(&format!("{arg} "))
    })
}

/// Why provreq will not install this engine — three genuinely different answers, kept distinct
/// because they call for different action (REQ048):
///
/// - **not an engine**: the operator mistyped, and needs the list.
/// - **wired but heavy tier**: a real engine provreq deliberately does not install, so the answer
///   is about *this subject's* build environment, not a generic "use a devcontainer".
/// - **not wired**: provreq has no integration, so installing it would not make it usable. That is
///   ours to fix, not the operator's, and saying so is more honest than pointing at their env.
/// - **not a binary at all**: the WebDriver grid is a *service* reached at an address (#245).
///   "Install it into your build environment" is the wrong sentence for something that is not
///   installed anywhere — the operator has to run one and say where it is.
fn unsupported_reason(arg: &str, subject: &Path) -> String {
    let Some(found) = registry_engine_named(arg) else {
        let known: Vec<&str> = engine::registry().iter().map(|e| e.name).collect();
        return format!(
            "'{arg}' is not a verification engine provreq knows. Known engines: {}. Installable \
             natively: tlc, kani.",
            known.join(", ")
        );
    };
    if found.probe.is_none() {
        return format!(
            "{} has no integration in provreq yet, so installing it would not make it usable — \
             that gap is provreq's to close, not yours.",
            found.name
        );
    }
    if matches!(found.probe, Some(engine::Probe::Grid)) {
        return format!(
            "{} is not installed at all — it is a service provreq talks to over HTTP. Run a \
             WebDriver grid (Selenium Grid, or a standalone browser container) and point provreq \
             at it with {}, or with `ui.endpoint` in provreq.yml if every operator reaches the \
             same one.",
            found.name,
            provreq::ui::ENDPOINT_VAR
        );
    }
    format!(
        "{} has no native install by decision (docs/design-c-decision.md) — {}.",
        found.name,
        provreq::buildenv::heavy_tier_advice(&provreq::buildenv::detect(subject))
    )
}

/// R-eng-2 install half (REQ046/REQ047): provision an engine natively, consent-gated. Only the
/// light tier is a native install (TLC, Kani); everything else is an honest "no native recipe —
/// use a devcontainer" per the Design-C decision. Exits non-zero only on a genuine install failure,
/// so an honest degradation or a consent prompt is a clean exit the operator can act on.
async fn run_install(engine: &str, yes: bool, subject: &Path) -> Result<()> {
    let outcome = match engine.to_ascii_lowercase().as_str() {
        "tlc" | "tla+" | "tla" => provreq::provision::install_tlc(yes).await?,
        "kani" => provreq::provision::install_kani(yes).await?,
        // Everything else is honest about *which* kind of "no" it is: an engine provreq cannot
        // run at all, one it will not install natively, or a name that is not an engine.
        other => provreq::provision::InstallOutcome::Unsupported {
            reason: unsupported_reason(other, subject),
        },
    };
    println!("{}: {}", engine, outcome.describe());
    if outcome.is_failure() {
        bail!("install of '{engine}' did not complete");
    }
    Ok(())
}

/// Step 4: produce the honest verdict for an admitted requirement. Re-gates, re-runs the
/// live category-1 grounding dry-run, pins provenance, and renders the verdict. Runs no
/// engine yet, so the verdict is always `unknown` (no-engine when grounded,
/// missing-grounding when not).
async fn run_verify(
    subject: &Path,
    id: &str,
    draft_contracts: bool,
    draft_semantic: bool,
    repair: bool,
) -> Result<()> {
    let Some(outcome) = provreq::verify::verify(subject, id)? else {
        bail!("no requirement item '{id}' in the subject");
    };
    let (verdict, stale, grounded, resolutions) = match outcome {
        VerifyOutcome::NoDraft => {
            bail!("no draft for {id} — formalize it first with `provreq draft {id}`");
        }
        VerifyOutcome::NotAdmitted => {
            println!(
                "Draft {id} is not admitted yet — admit the formalization first with `--admit`."
            );
            return Ok(());
        }
        VerifyOutcome::NoCandidate => {
            println!("Draft {id} has no candidate PRL to verify.");
            return Ok(());
        }
        VerifyOutcome::GateFailed { errors } => {
            println!(
                "Cannot verify {id} — the admitted candidate no longer gates ({} error(s)); re-check it:",
                errors.len()
            );
            for e in &errors {
                println!("  - {e}");
            }
            return Ok(());
        }
        VerifyOutcome::Verdict {
            verdict,
            stale,
            grounded,
            resolutions,
        } => (verdict, stale, grounded, resolutions),
    };

    println!("{}", provreq::verdict::render(&verdict));
    // An admitted draft whose source prose moved is worth flagging alongside the verdict.
    if stale {
        println!(
            "  ! the requirement prose moved since admission — re-admit before trusting this verdict"
        );
    }
    // A6 proof-carrier draft channel (REQ033): on request, stage the missing deductive marker onto
    // opaque predicate fns so a deductive engine can then see inside them. Only a grounded
    // requirement has resolved predicates to annotate.
    if draft_contracts {
        if grounded {
            stage_marker_drafts(subject, id, &resolutions)?;
        } else {
            println!(
                "\n--draft-contracts: nothing to draft — {id} is not grounded, so no predicate \
                 resolves to a function to annotate."
            );
        }
    }
    // A6 semantic contract-drafting channel (REQ040): on request, ask the LLM to draft
    // `#[requires]`/`#[ensures]` clauses onto the same resolved predicate fns. Opt-in and separate
    // from the marker draft; only a grounded requirement has resolved predicates to describe.
    if draft_semantic {
        if grounded {
            stage_semantic_drafts(subject, id, &resolutions, repair).await?;
        } else {
            println!(
                "\n--draft-semantic: nothing to draft — {id} is not grounded, so no predicate \
                 resolves to a function to contract."
            );
        }
    }
    Ok(())
}

/// Stage the A6 deductive-marker drafts into the subject's working tree (REQ033). Reads the target
/// marker from the subject's manifest, drafts it onto each resolved predicate fn that lacks it, and
/// writes the edits back as uncommitted changes for the operator to review. Runs no git.
fn stage_marker_drafts(
    subject: &Path,
    id: &str,
    resolutions: &BTreeMap<String, Resolution>,
) -> Result<()> {
    use provreq::contract_draft::{
        apply_to_source, ensure_logic_types, ensure_prelude, marker_for_subject, plan_markers,
    };

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

    // **The marker channel is Prusti-only** (#158). `#[pure]` makes a function transparent where it
    // stands; `#[logic]` declares a *logical* function, which removes the item from the program
    // namespace and breaks every call site — exactly the case a category-1 predicate normally
    // resolves to. Refusing is the honest answer, and Creusot already has the other one: a mirror
    // (REQ068), drafted by `--draft-semantic`, which leaves the program function untouched.
    if !marker.drafts_markers() {
        println!(
            "\n--draft-contracts: nothing staged — this subject is a Creusot subject, and \
             `{}` cannot go on a program function: it declares a *logical* function, so the item \
             leaves the program namespace and every call site stops compiling.\n  Creusot reaches \
             such a function through a `#[logic]` mirror that leaves it untouched — run `verify \
             {id} --draft-semantic` instead.",
            marker.attribute()
        );
        return Ok(());
    }

    let sources = load_predicate_sources(subject, resolutions)?;
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
        // Insert the markers first (bottom-up, so line numbers hold), then the import — which adds
        // a line near the top and would otherwise shift every target. A file that gains `#[pure]`
        // must import the dialect that defines it or the staged edit cannot compile at all
        // (measured: `cannot find attribute` from a file that names the contracts crate nowhere),
        // and a draft that cannot parse is not a reviewable proposal (#158).
        let edited = ensure_prelude(&apply_to_source(original, file_drafts), marker);
        // A spec-only type the staged text names has to be in scope too (#194).
        let edited = ensure_logic_types(&edited, marker);
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

/// Load the full text of every file a resolved predicate lives in, keyed by subject-relative path.
/// Shared by the marker and semantic draft stagers so both read the subject's source the same way.
fn load_predicate_sources(
    subject: &Path,
    resolutions: &BTreeMap<String, Resolution>,
) -> Result<BTreeMap<String, String>> {
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
    Ok(sources)
}

/// Ask the configured LLM to draft `#[requires]`/`#[ensures]` clauses onto each resolved predicate
/// fn and stage them into the subject's working tree (REQ040). Reads the target dialect from the
/// subject's manifest (Creusot vs Prusti) and the requirement's intent + formal claim from its
/// draft, then writes the edits back as uncommitted changes for review. Runs no git; the clauses
/// are an untrusted proposal the verifier re-checks.
async fn stage_semantic_drafts(
    subject: &Path,
    id: &str,
    resolutions: &BTreeMap<String, Resolution>,
    repair: bool,
) -> Result<()> {
    use provreq::contract_draft::{ensure_logic_types, ensure_prelude, marker_for_subject};
    use provreq::mirror_draft::{append_items, link_clauses, MirrorDraft, Mirrorer};
    use provreq::semantic_draft::{apply_to_source, ContractDraft, Drafter, ProofStep};

    let manifest = std::fs::read_to_string(subject.join("Cargo.toml"))
        .with_context(|| format!("reading {}", subject.join("Cargo.toml").display()))?;
    let Some(marker) = marker_for_subject(&manifest) else {
        println!(
            "\n--draft-semantic: the subject depends on neither creusot-contracts nor \
             prusti-contracts, so there is no verifier dialect to draft contracts in — add a \
             contracts crate first (this is REQ032's missing-dependency inconclusive)."
        );
        return Ok(());
    };

    // The requirement's intent (prose) and formal claim (PRL candidate) give the model context. Both
    // come from the admitted draft the verify run already read — reload them cheaply here rather than
    // widen the shared verify outcome with CLI-only fields.
    let (companion, items) = provreq::adopt::resolve(subject)?;
    let config = provreq::llm::load_config(&companion)?.context(
        "no `llm:` block in provreq.yml — configure a provider to use `verify --draft-semantic`",
    )?;
    let intent = items
        .iter()
        .find(|i| i.id == id)
        .map(|i| i.text.clone())
        .unwrap_or_default();
    let claim = provreq::draft::load(&companion)?
        .drafts
        .get(id)
        .and_then(|d| d.candidate.clone())
        .unwrap_or_default();

    let sources = load_predicate_sources(subject, resolutions)?;
    let engine = match marker {
        provreq::contract_draft::Marker::Logic => "Creusot",
        provreq::contract_draft::Marker::Pure => "Prusti",
    };
    let mode = if repair {
        "verify-and-repair"
    } else {
        "one-shot draft"
    };
    println!(
        "\n--draft-semantic ({mode}): drafting {engine} contracts for {id} with {} via {}{} …",
        config.model,
        config.base_url,
        config.override_note()
    );
    let drafter = Drafter::new(provreq::llm::RuntimeBackend::from_config(config)?);

    // A Creusot subject additionally needs LOGIC MIRRORS, and without them the contracts alone
    // cannot reach a proof: pearlite may only call `#[logic]` items, so a contract mentioning a
    // resolved predicate is rejected as *called program function `f` in logic context*. Drafted
    // once, before any repair round, because a mirror states what a function MEANS — that does not
    // change when the prover fails to discharge a claim, whereas a contract does. Prusti has no
    // such split (its `#[pure]` program functions are callable from specs), so this is Creusot-only.
    let drafted = if matches!(marker, provreq::contract_draft::Marker::Logic) {
        Mirrorer::new(provreq::llm::RuntimeBackend::from_config(
            provreq::llm::load_config(&companion)?.expect("config loaded above"),
        )?)
        .draft(&intent, &claim, resolutions, &sources)
        .await?
    } else {
        provreq::mirror_draft::MirrorDrafts::default()
    };
    let mirrors = drafted.drafts;
    report_mirror_drafts(id, &mirrors);
    // A target the channel gave up on is reported too, and BEFORE the verdict it explains: the
    // predicate keeps calling its program function in the harness, so the claim fails at the very
    // wall this channel removes. Silence there reads as a complete draft (#170).
    report_dropped_mirrors(&drafted.dropped);
    // The contract channel shares the mirror channel's wall — a spec may call no program function —
    // so it is told which mirrors exist. Without this it proposed `result <==> self.is_available()`:
    // a prohibition with no stated alternative just gets broken.
    let mirror_note = provreq::mirror_draft::mirror_note(&mirrors);
    let ctx = provreq::semantic_draft::DraftContext {
        intent: &intent,
        claim: &claim,
        marker,
        mirrors: &mirror_note,
    };

    // Stage a draft set FRESH from the original sources: every predicate file is rewritten from its
    // original text plus that round's clauses, so a repair round never stacks on a prior round's edit
    // and an undrafted file is restored to original. Runs no git.
    //
    // The mirror links stage in the SAME pass as the drafted clauses because both are keyed to line
    // numbers in the original text; the mirror items are appended afterwards, which inserts no lines
    // and so cannot disturb them.
    let stage = |drafts: &[ContractDraft]| -> Result<()> {
        let mut by_file: BTreeMap<String, Vec<ContractDraft>> = BTreeMap::new();
        for d in drafts.iter().cloned().chain(link_clauses(&mirrors)) {
            by_file.entry(d.file.clone()).or_default().push(d);
        }
        let mut items_by_file: BTreeMap<String, Vec<MirrorDraft>> = BTreeMap::new();
        for m in &mirrors {
            items_by_file
                .entry(m.file.clone())
                .or_default()
                .push(m.clone());
        }
        for (file, original) in &sources {
            let file_drafts = by_file.get(file).cloned().unwrap_or_default();
            let edited = apply_to_source(original, &file_drafts);
            let file_mirrors = items_by_file.get(file).cloned().unwrap_or_default();
            let edited = append_items(&edited, &file_mirrors);
            // A file that gains contract syntax must import the dialect that defines it, or the
            // staged edit cannot compile — measured, `cannot find attribute 'ensures' in this
            // scope` from a file that names `creusot_std` nowhere. Only touched when something
            // was actually staged into this file, so an undrafted file stays byte-identical.
            let edited = if file_drafts.is_empty() && file_mirrors.is_empty() {
                edited
            } else {
                // The dialect's own prelude, plus any spec-only type the staged body names (#194).
                ensure_logic_types(&ensure_prelude(&edited, marker), marker)
            };
            std::fs::write(subject.join(file), edited).with_context(|| {
                format!(
                    "staging contract draft into {}",
                    subject.join(file).display()
                )
            })?;
        }
        Ok(())
    };

    // **Contracts are a Prusti-only channel.** On the Creusot route a drafted clause cannot help and
    // can do real harm, both measured against the real prover (#164):
    //
    // * It cannot help. After `with_mirrors` the harness is a `proof_assert!` over the **mirrors**
    //   and calls no program function, so an `#[ensures]` on one is discharged where nothing reads
    //   it. Probe E proved REQ047 with mirrors and no contract clauses at all.
    // * It can produce a FALSE `proven`. The linking `#[ensures(result == mirror(..))]` is
    //   discharged *assuming the function's preconditions*, so a drafted `#[requires]` narrows the
    //   domain on which the mirror was ever checked — while the harness's `forall` ranges over all
    //   of it. Measured: `#[requires(!allowed)]` plus a mirror that is genuinely correct under
    //   `!allowed` (its link discharges honestly) yields `Holds` for a claim that is false of the
    //   program. See `probe_a_precondition_on_a_mirrored_function` in `crate::creusot`.
    //
    // Prusti has no such exposure: its `#[pure]` functions are spec-callable, so there are no
    // mirrors and no links for a precondition to weaken — contracts are the whole mechanism there.
    if !marker.drafts_contracts() {
        stage(&[])?;
        report_mirrors_are_the_whole_draft(id, &mirrors, repair, subject, engine)?;
        return Ok(());
    }

    if repair {
        // The prover is the gate: stage this round's drafts, re-run the ensemble on the changed
        // working tree (`verify::verify` re-reads everything), and report whether the claim now
        // proves. The loop's repair logic lives in the Drafter; this closure is the side-effecting
        // half. ponytail: each round re-persists a verdict via verify::verify — the final one wins.
        let verify_round = |drafts: &[ContractDraft]| -> Result<ProofStep> {
            stage(drafts)?;
            match provreq::verify::verify(subject, id)? {
                Some(VerifyOutcome::Verdict { verdict, .. }) => {
                    Ok(verdict_to_proof_step(&verdict, engine))
                }
                other => Ok(ProofStep::Inconclusive {
                    reason: format!("re-verify produced no verdict ({other:?})"),
                }),
            }
        };
        let out = drafter
            .draft_repaired(ctx, resolutions, &sources, verify_round)
            .await?;
        report_semantic_drafts(id, &out.drafts, Some((out.attempts, &out.step)));
    } else {
        let drafts = drafter.draft(ctx, resolutions, &sources).await?;
        stage(&drafts)?;
        report_semantic_drafts(id, &drafts, None);
    }
    Ok(())
}

/// Report a Creusot draft, where the mirrors **are** the whole proposal.
///
/// With no contract clauses there is nothing for a repair round to revise — a mirror states what a
/// function *means*, and prover failure does not change its meaning, which is why mirrors are
/// drafted once (#160). So `--repair` here means what it can honestly mean: verify the staged
/// mirrors against the real prover once, and say whether the claim proved.
fn report_mirrors_are_the_whole_draft(
    id: &str,
    mirrors: &[provreq::mirror_draft::MirrorDraft],
    repair: bool,
    subject: &std::path::Path,
    engine: &str,
) -> Result<()> {
    use provreq::semantic_draft::ProofStep;
    if mirrors.is_empty() {
        println!(
            "\n--draft-semantic: nothing staged for {id} — no predicate got a usable mirror, so \
             there is no proposal to review. {engine} cannot reach an ordinary program function \
             without one."
        );
        return Ok(());
    }
    if !repair {
        println!(
            "\n--draft-semantic: the mirrors above are the whole proposal — {engine} needs no \
             contract clauses to reach them. Re-run `verify` to check them against the prover."
        );
        return Ok(());
    }
    let step = match provreq::verify::verify(subject, id)? {
        Some(VerifyOutcome::Verdict { verdict, .. }) => verdict_to_proof_step(&verdict, engine),
        other => ProofStep::Inconclusive {
            reason: format!("re-verify produced no verdict ({other:?})"),
        },
    };
    match step {
        ProofStep::Proved => println!(
            "\n  ✓ {engine} discharged the claim from the mirrors alone — proof-carrying (still \
             read the mirror bodies above: they are where meaning entered)."
        ),
        ProofStep::Inconclusive { reason } => println!(
            "\n  ! {engine} did not discharge the claim ({reason}). The mirrors are staged for you \
             to refine; a mirror is re-drafted only by re-running the draft."
        ),
    }
    println!(
        "  Review the working-tree diff and re-run `verify` — the tool staged an uncommitted edit \
         and ran no git."
    );
    Ok(())
}

/// Print the staged logic mirrors. Reported separately from the contracts because they are a
/// different kind of proposal and carry a different risk: a contract clause the prover cannot
/// discharge merely fails, whereas a mirror asserts what a function *means*. The prover does check
/// each mirror against the real body — a wrong one fails at its own linking clause, naming the
/// function — but that check is only as good as the operator's reading of the mirror it kept.
fn report_mirror_drafts(id: &str, mirrors: &[provreq::mirror_draft::MirrorDraft]) {
    if mirrors.is_empty() {
        return;
    }
    println!(
        "\n--draft-semantic: staged {} logic mirror(s) for {id} — REVIEW THESE FIRST:",
        mirrors.len()
    );
    for m in mirrors {
        println!("  {}:{}  → {}", m.file, m.line, m.name);
        println!("    + {}", m.link);
        for line in m.item.lines() {
            println!("      {line}");
        }
    }
    println!(
        "  A mirror states what a function MEANS, in the prover's own language. The linking \
         #[ensures] makes the prover check it against the real body, so a wrong mirror fails \
         rather than proving something false — but a mirror you keep without reading is a claim \
         you have not reviewed."
    );
}

/// Print the predicates the mirror channel **gave up on**, and what stopped each one.
///
/// A dropped mirror is not a smaller draft, it is a hole in the one the operator is about to read:
/// that predicate keeps calling its program function inside `proof_assert!`, so Creusot fails with
/// *called program function `f` in logic context* — the exact wall this channel exists to remove,
/// now reported as if nothing had been attempted. Measured on a fresh subject (#170): one of two
/// mirrors staged, the other abandoned in silence, and the harness still calling the function the
/// prover had already named.
///
/// Dropping stays — a mirror provreq cannot parse or link is an unchecked meaning, and staging one
/// is the false `proven` this design exists to prevent. Only the silence is fixed.
fn report_dropped_mirrors(dropped: &[provreq::mirror_draft::DroppedMirror]) {
    if dropped.is_empty() {
        return;
    }
    println!(
        "\n--draft-semantic: NO mirror was staged for {} predicate(s) — the claim cannot reach a \
         Creusot proof until each is mirrored:",
        dropped.len()
    );
    for d in dropped {
        println!(
            "  {}:{}  {} (wanted `{}`)",
            d.file, d.line, d.function, d.name
        );
        println!("    - {}", d.wall.explain());
    }
    println!(
        "  Each of these keeps calling its program function inside the proof, which is what \
         Creusot refuses — so expect `called program function … in logic context` naming one of \
         them, and read that as this message, not as a defect in the subject."
    );
}

/// Map a re-verification's [`provreq::verdict::Verdict`] into the repair loop's [`ProofStep`], as
/// judged by **the engine whose contracts are being drafted** — and no other.
///
/// Any-engine was wrong, and measurably so. REQ047 has a standing bounded `holds` from Kani, which
/// no drafted Creusot clause affects either way; on that evidence the loop stopped after one round
/// and printed *"the prover discharged the claim — these clauses are proof-carrying"* while Creusot
/// had failed to compile the harness and never saw them. A clause is proof-carrying only if the
/// prover it was written for discharged it, so a corroborating verdict from a different engine —
/// however welcome in the verdict itself — is not evidence about these drafts.
///
/// The inconclusive reason likewise narrows to that engine: feeding a repair round Prusti's
/// toolchain ceiling as the thing to fix would send it revising contracts against a message that
/// has nothing to do with them.
fn verdict_to_proof_step(
    verdict: &provreq::verdict::Verdict,
    engine: &str,
) -> provreq::semantic_draft::ProofStep {
    use provreq::semantic_draft::ProofStep;
    use provreq::verdict::Status;
    let mine = || verdict.evidence.iter().filter(|e| e.engine == engine);
    if mine().any(|e| e.status == Status::Holds) {
        return ProofStep::Proved;
    }
    let reason = mine()
        .flat_map(|e| e.detail.iter().cloned())
        .collect::<Vec<_>>()
        .join("; ");
    ProofStep::Inconclusive {
        reason: if reason.is_empty() {
            format!("{engine} did not discharge the claim")
        } else {
            reason
        },
    }
}

/// Print the staged semantic drafts and, in repair mode, the proof step they reached. The A6/D12
/// note holds either way: the staged clauses are an untrusted proposal the operator reviews.
fn report_semantic_drafts(
    id: &str,
    drafts: &[provreq::semantic_draft::ContractDraft],
    repair: Option<(u32, &provreq::semantic_draft::ProofStep)>,
) {
    use provreq::semantic_draft::ProofStep;
    if drafts.is_empty() {
        println!(
            "--draft-semantic: the model proposed no contracts for {id}'s predicates — nothing \
             staged (a function it cannot faithfully contract is left untouched, not guessed)."
        );
        return;
    }
    let total: usize = drafts.iter().map(|d| d.clauses.len()).sum();
    println!(
        "--draft-semantic: staged {total} contract clause(s) across {} function(s) for review:",
        drafts.len()
    );
    for d in drafts {
        println!("  {}:{}", d.file, d.line);
        for c in &d.clauses {
            println!("    + {c}");
        }
    }
    if let Some((attempts, step)) = repair {
        match step {
            ProofStep::Proved => println!(
                "  ✓ the prover discharged the claim after {attempts} draft round(s) — these clauses \
                 are proof-carrying (still review the diff before keeping them)."
            ),
            ProofStep::Inconclusive { reason } => println!(
                "  ! after {attempts} round(s) the prover still could not discharge the claim \
                 ({reason}); the drafts are staged for you to refine."
            ),
        }
    }
    println!(
        "  Review the working-tree diff and re-run `verify` — the tool staged an uncommitted edit \
         and ran no git; these clauses are an untrusted proposal the verifier re-checks."
    );
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

/// Show the operator the MFOTL a category-2b claim lowers to, on dry-run (#232).
///
/// A generated temporal formula the operator never sees is a claim they cannot check — and this one
/// is the **negation** of what they wrote, which makes that worse rather than better. The read-back
/// says so in words beside the formula, so `AND NOT EVENTUALLY` cannot be misread as the
/// requirement itself.
///
/// Silent for every other category, and for a 2b subject with no `monitor:` block: the binding
/// dry-run above has already said what is wrong there, and repeating it here would be noise.
fn print_monitor_claim(
    subject: &Path,
    companion: &Path,
    requirement: &provreq::prl::Requirement,
    bindings: &[grounding::Binding],
) {
    if grounding::default_category(requirement) != grounding::BindCategory::Runtime {
        return;
    }
    let Ok(Some(monitor)) = provreq::monitor::Monitor::load(subject, companion) else {
        return;
    };
    for prop in &requirement.require {
        println!("\n  What MonPoly will be asked (the VIOLATION pattern — it matches where the");
        println!("  requirement is BROKEN, so silence over the trace is what a pass looks like):");
        match provreq::monitor::lower(requirement, prop, &monitor, bindings) {
            Ok(claim) => {
                println!("    formula   {}", claim.formula);
                for line in claim.signature.lines() {
                    println!("    signature {line}");
                }
                println!("    deadline  {}s", claim.within_seconds);
            }
            Err(e) => println!("    not lowerable — {}", e.reason),
        }
    }
}

/// The step script a category-3 claim lowers to (#243).
///
/// Shown for the same reason `print_monitor_claim` shows the formula: a script assembled from
/// bindings and an `after` scope is not obvious from the requirement text, and D12 is only faithful
/// if the operator can read what will actually be run rather than being told it exists.
fn print_ui_script(
    companion: &Path,
    requirement: &provreq::prl::Requirement,
    bindings: &[grounding::Binding],
) {
    if grounding::default_category(requirement) != grounding::BindCategory::Ui {
        return;
    }
    let Ok(Some(ui)) = provreq::ui::Ui::load(companion) else {
        return;
    };
    for prop in &requirement.require {
        println!("\n  What a driver will run (one execution of one deployment — it can show the");
        println!("  requirement BROKEN, and can never show it holds):");
        match provreq::ui::lower(requirement, prop, &ui, bindings) {
            Ok(claim) => {
                for line in claim.describe() {
                    println!("    {line}");
                }
            }
            Err(e) => println!("    not lowerable — {}", e.reason),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use provreq::semantic_draft::ProofStep;
    use provreq::verdict::{aggregate, Basis, Evidence, Provenance};

    // Verifies: REQ056 — a subject path handed to an id-taking command is recognised as such, so
    // the error can name the fix. This is the real wiring, not just the predicate: it asserts that
    // clap reports the mistake the way the hint depends on.
    #[test]
    fn a_path_passed_where_an_id_belongs_is_recognised() {
        for argv in [
            vec!["provreq", "verify", "REQ047", "."],
            vec!["provreq", "draft", "REQ047", ".."],
            vec!["provreq", "install", "kani", "src"],
        ] {
            let Err(err) = Cli::try_parse_from(&argv) else {
                panic!("{argv:?}: the stray path must not parse")
            };
            assert!(
                stray_path_argument(&err).is_some(),
                "{argv:?} should be recognised as a stray path, got {:?}: {err}",
                err.kind()
            );
        }
    }

    // Verifies: REQ056 — the hint stays off everything else. Attaching a confident `--path`
    // suggestion to an unrelated error would be worse than the bare message it replaced.
    #[test]
    fn the_path_hint_does_not_fire_on_other_mistakes() {
        for argv in [
            // A mistyped flag is not a path.
            vec!["provreq", "verify", "REQ047", "--pth", "."],
            // A second id is a real usage error, but naming `--path` would be wrong.
            vec!["provreq", "verify", "REQ047", "REQ048"],
            // A missing required argument reports something else entirely.
            vec!["provreq", "verify"],
        ] {
            let Err(err) = Cli::try_parse_from(&argv) else {
                panic!("{argv:?}: this must not parse either")
            };
            assert_eq!(
                stray_path_argument(&err),
                None,
                "{argv:?} must not get a path hint"
            );
        }

        assert!(!looks_like_a_path("REQ048"));
        assert!(!looks_like_a_path("--path"));
        assert!(looks_like_a_path("."));
        assert!(looks_like_a_path("../elsewhere"));
    }

    // Verifies: REQ048 — `install` distinguishes the three reasons it will not install, because
    // they call for different action: fix your typo, change your build env, or wait for us.
    #[test]
    fn install_says_which_kind_of_no_it_is() {
        let subject = Path::new(env!("CARGO_MANIFEST_DIR"));

        // Not an engine at all: name the mistake and list what is real.
        let unknown = unsupported_reason("creusto", subject);
        assert!(
            unknown.contains("is not a verification engine"),
            "{unknown}"
        );
        assert!(
            unknown.contains("Creusot"),
            "the list must be there: {unknown}"
        );

        // A wired heavy-tier engine: answer in terms of THIS subject's environment. Run against
        // provreq's own repo, whose dev-container names an image.
        let heavy = unsupported_reason("creusot", subject);
        assert!(
            heavy.starts_with("Creusot has no native install"),
            "{heavy}"
        );
        assert!(
            heavy.contains("provreq-devcontainer"),
            "must name this subject's own dev-container image: {heavy}"
        );

        // MonPoly joined the heavy tier in #233: it is wired now, so the answer is about the
        // environment rather than about provreq's gap.
        let monpoly = unsupported_reason("monpoly", subject);
        assert!(
            monpoly.starts_with("MonPoly has no native install"),
            "{monpoly}"
        );

        // The fourth kind of no, and the reason there is a fourth (#245): a WebDriver grid is not
        // installed anywhere. Telling the operator to put it in their build environment would send
        // them after something that does not go there — they run a service and say where it is.
        // The unwired branch above it can no longer be reached from the registry, because #245
        // wired the last category; it stays for the engine that is wired next.
        let grid = unsupported_reason("selenium", subject);
        assert!(grid.contains("is not installed at all"), "{grid}");
        assert!(grid.contains("WEBDRIVER_URL"), "{grid}");
        assert!(
            !grid.contains("dev-container"),
            "a grid is not put into a build environment: {grid}"
        );
    }

    // Verifies: REQ048 — `tlc` resolves to the registry's `TLC (TLA+)` despite the suffix, and a
    // non-engine resolves to nothing rather than to a near-match.
    #[test]
    fn engine_names_resolve_from_their_cli_spelling() {
        assert_eq!(
            registry_engine_named("tlc").map(|e| e.name),
            Some("TLC (TLA+)")
        );
        assert_eq!(registry_engine_named("KANI").map(|e| e.name), Some("Kani"));
        assert!(registry_engine_named("tl").is_none());
        assert!(registry_engine_named("").is_none());
    }

    fn prov() -> Provenance {
        Provenance {
            requirement_revision: "r".into(),
            subject_commit: None,
            tool_version: "t".into(),
        }
    }

    // Verifies: REQ041 — a re-verification where the DRAFTING engine established a `holds` maps to
    // Proved, so the repair loop stops.
    #[test]
    fn proof_step_is_proved_when_the_drafting_engine_holds() {
        let v = aggregate(
            "REQ001",
            vec![Evidence::holds("Creusot", Basis::Proven)],
            prov(),
        );
        assert_eq!(verdict_to_proof_step(&v, "Creusot"), ProofStep::Proved);
    }

    // Verifies: another engine's `holds` is NOT evidence about these drafts. Measured on REQ047:
    // Kani's standing bounded `holds` — which no Creusot clause affects — made the loop stop after
    // one round and print "the prover discharged the claim … proof-carrying" while Creusot had
    // failed to compile the harness and never saw a single drafted clause. A clause is
    // proof-carrying only if the prover it was written for discharged it.
    #[test]
    fn another_engines_holds_is_not_evidence_about_these_drafts() {
        let v = aggregate(
            "REQ001",
            vec![
                Evidence::holds("Kani", Basis::ModelCheckedBounded),
                Evidence::inconclusive("Creusot", vec!["the proof harness did not compile".into()]),
            ],
            prov(),
        );
        match verdict_to_proof_step(&v, "Creusot") {
            ProofStep::Inconclusive { reason } => assert!(
                reason.contains("did not compile"),
                "the repair round must be fed Creusot's own reason, got {reason:?}"
            ),
            other => panic!("Kani's bounded holds must not pass off as a Creusot proof: {other:?}"),
        }
    }

    // Verifies: REQ041 — an all-inconclusive verdict maps to Inconclusive carrying the engines' own
    // reasons, which is the feedback the next re-draft targets.
    #[test]
    fn proof_step_carries_reasons_when_inconclusive() {
        let v = aggregate(
            "REQ001",
            vec![Evidence::inconclusive(
                "Creusot",
                vec!["goal foo'post unproved".into()],
            )],
            prov(),
        );
        match verdict_to_proof_step(&v, "Creusot") {
            ProofStep::Inconclusive { reason } => {
                assert!(reason.contains("goal foo'post unproved"))
            }
            other => panic!("expected inconclusive, got {other:?}"),
        }
    }
}
