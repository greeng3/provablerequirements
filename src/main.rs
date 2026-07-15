use anyhow::{bail, Context, Result};
use clap::{Parser, Subcommand};
use provreq::adopt::find_companion;
use provreq::doorstop::DoorstopSource;
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

fn run_status(subject: &Path) -> Result<()> {
    let (companion, items) = resolve(subject)?;
    let state = triage::load(&companion)?;
    let cov = provreq::status::coverage(&items, &state);
    println!("Coverage funnel:");
    println!("  discovered        {}", cov.discovered);
    println!("  untriaged         {}", cov.untriaged);
    println!("  formalizable-now  {}", cov.formalizable_now);
    println!("  falsifiable-only  {}", cov.falsifiable_only);
    println!("  stays-prose       {}", cov.stays_prose);
    println!(
        "  formalized        {} (Step 3 — not built yet)",
        cov.formalized
    );
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
