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

    /// Scan a directory for TODO, FIXME, HACK comments
    Todos {
        /// Path to scan (defaults to current directory)
        #[arg(default_value = ".")]
        path: String,
    },

    /// Show context for the current project
    Context,
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
    };

    if let Err(e) = result {
        error!(error = %e, "Command failed");
        eprintln!("\n{}: {}", "Error".red(), e);
        std::process::exit(1);
    }
}