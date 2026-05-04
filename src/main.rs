mod commands;
mod error;

use clap::{Parser, Subcommand};
use colored::Colorize;
use tracing::error;
use tracing_subscriber::{EnvFilter, fmt, prelude::*};

#[derive(Parser)]
#[command(
    name = "devpulse",
    about = "Your developer environment companion",
    version = "0.1.0"
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Check your dev environment health
    Health,

    /// Scan a directory for TODO, FIXME, HACK comments
    Todos {
        /// Path to scan (defaults to current directory)
        #[arg(default_value = ".")]
        path: String,
    },

    /// Show context for the current project
    Context,

    /// Run all checks: health + todos + context
    All {
        /// Path to scan for TODOs (defaults to current directory)
        #[arg(default_value = ".")]
        path: String,
    },
}

fn main() {
    tracing_subscriber::registry()
        .with(fmt::layer().with_target(true))
        .with(EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("warn")))
        .init();

    let cli = Cli::parse();

    let result = match cli.command {
        Commands::Health => commands::health::run(),
        Commands::Todos { path } => commands::todos::run(&path),
        Commands::Context => commands::context::run(),
        Commands::All { path } => run_all(&path),
    };

    if let Err(e) = result {
        error!(error = %e, "Command failed");
        eprintln!("\n{}: {}", "Error".red(), e);
        std::process::exit(1);
    }
}

/// Runs all three commands in sequence.
/// One section failing does NOT abort the rest — each runs independently.
fn run_all(path: &str) -> error::Result<()> {
    // Try to load last session to show "last seen" info
    let cwd = std::env::current_dir().map_err(error::DevpulseError::Io)?;
    let project_name = cwd
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("unknown")
        .to_string();

    println!("\n{}", "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━".dimmed());
    println!(
        "  {} {}",
        "devpulse".cyan().bold(),
        "— full project scan".dimmed()
    );

    // Show last session time if one exists — this is where load_session is used
    if let Some(last) = commands::context::load_session(&project_name) {
        println!(
            "  {}",
            format!("Last scanned at unix timestamp {}", last.last_seen).dimmed()
        );
    }

    println!("{}\n", "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━".dimmed());

    // ── 1. Health ────────────────────────────────────────────────────────────
    section_header("1 / 3", "Health check");
    if let Err(e) = commands::health::run() {
        print_section_error("health", &e);
    }

    // ── 2. TODOs ─────────────────────────────────────────────────────────────
    section_header("2 / 3", "TODO scan");
    if let Err(e) = commands::todos::run(path) {
        print_section_error("todos", &e);
    }

    // ── 3. Context ───────────────────────────────────────────────────────────
    section_header("3 / 3", "Project context");
    if let Err(e) = commands::context::run() {
        print_section_error("context", &e);
    }

    println!("{}", "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━".dimmed());
    println!(
        "  {} {}",
        "Done.".green().bold(),
        "Run `devpulse --help` for individual commands.".dimmed()
    );
    println!("{}\n", "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━".dimmed());

    Ok(())
}

fn section_header(step: &str, title: &str) {
    println!("\n  {} {}", format!("[{}]", step).dimmed(), title.bold());
    println!("  {}", "───────────────────────".dimmed());
}

fn print_section_error(section: &str, e: &dyn std::error::Error) {
    eprintln!("  {} {} failed: {}", "⚠".yellow(), section.bold(), e);
}
