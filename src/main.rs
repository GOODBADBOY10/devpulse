mod commands;
mod error;

use clap::{Parser, Subcommand};
use colored::Colorize;
use tracing::error;
use tracing_subscriber::{fmt, prelude::*, EnvFilter};

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
}

fn main() {
    // Initialise structured logging.
    // Level is controlled by the RUST_LOG env var (e.g. RUST_LOG=debug devpulse health).
    // Defaults to "warn" so normal users see no log noise.
    // The `env-filter` feature lets operators tune this per-crate without recompiling.
    tracing_subscriber::registry()
        .with(fmt::layer().with_target(true))
        .with(EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("warn")))
        .init();

    let cli = Cli::parse();

    // Map the Result from each subcommand to a clean exit code.
    // std::process::exit(1) is the Unix convention for "something went wrong".
    // We never let panics be the exit mechanism in production.
    let result = match cli.command {
        Commands::Health => commands::health::run(),
    };

    if let Err(e) = result {
        // Log the full error chain for operators; print a clean message for users.
        error!(error = %e, "Command failed");
        eprintln!("\n{}: {}", "Error".red(), e);
        std::process::exit(1);
    }
}