use anyhow::{bail, Context, Result};
use clap::{Parser, Subcommand};
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
}

#[tokio::main]
async fn main() -> Result<()> {
    match Cli::parse().command {
        Command::Serve { port } => provreq::server::serve(port).await.map_err(Into::into),
        Command::Init { path, name, yes } => run_init(&path, name.as_deref(), yes),
    }
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
