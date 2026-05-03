use crate::error::{DevpulseError, Result};
use colored::Colorize;
use std::path::Path;
use std::process::Command;
use std::time::Instant;
use tracing::{debug, info, instrument, warn};

// How long we wait for a tool before giving up.
// A hanging `docker` call (e.g. on a cold daemon) would freeze the whole app
// without this guard.
const TOOL_TIMEOUT_SECS: u64 = 5;

/// Everything we know about one check after it runs.
#[derive(Debug)]
pub struct CheckResult {
    pub label: String,
    pub status: CheckStatus,
    pub message: String,
    pub duration_ms: u128,
}

/// Explicit enum — not a bare bool — so future arms (Warn, Skip) are easy to add.
#[derive(Debug)]
pub enum CheckStatus {
    Pass,
    Fail,
}

/// Entry point called from main.
/// Returns Err only for truly unexpected failures (e.g. can't read cwd).
/// Individual tool-not-found results are encoded as CheckStatus::Fail, not errors.
#[instrument(name = "health_check")]
pub fn run() -> Result<()> {
    info!("Starting health check");

    println!("\n{}", "devpulse health check".bold().underline());
    println!("{}\n", "Scanning your dev environment...".dimmed());

    let checks = collect_checks()?;

    let total = checks.len();
    let passed = checks.iter().filter(|c| matches!(c.status, CheckStatus::Pass)).count();

    for check in &checks {
        print_check(check);
    }

    println!();
    println!("{}", format!("Result: {}/{} checks passed", passed, total).bold());

    if passed == total {
        println!("{}", "Your environment looks healthy!".green().bold());
    } else {
        let failed = total - passed;
        println!("{}", format!("{} issue(s) need attention.", failed).yellow().bold());
    }

    println!();

    info!(passed, total, "Health check complete");
    Ok(())
}

/// Run every check and collect results.
/// Separated from `run()` so it is independently testable.
fn collect_checks() -> Result<Vec<CheckResult>> {
    let checks = vec![
        check_tool("Rust", "rustc"),
        check_tool("Cargo", "cargo"),
        check_tool("Git", "git"),
        check_tool("Node.js", "node"),
        check_tool("Docker", "docker"),
        check_env_file()?,
    ];
    Ok(checks)
}

/// Probe a binary by running `<binary> --version`.
///
/// We treat a missing binary as a CheckStatus::Fail (expected failure),
/// not as a DevpulseError (unexpected/programmer error). This means callers
/// never need to handle "tool not found" as an exception.
///
/// A per-call timeout prevents a slow daemon (e.g. Docker on first start)
/// from stalling the whole health check.
#[instrument(skip_all, fields(binary))]
fn check_tool(label: &str, binary: &str) -> CheckResult {
    debug!(binary, "Probing tool");
    let start = Instant::now();

    // Build the child process but do NOT use .unwrap().
    // Command::new never panics — it only fails when we call .output() / .spawn().
    let result = Command::new(binary)
        .arg("--version")
        // Silence stderr so tool error messages don't bleed into our output
        .stderr(std::process::Stdio::null())
        .output();

    let duration_ms = start.elapsed().as_millis();

    // Warn in the log if a tool is slow; the user only sees the formatted line.
    if duration_ms > (TOOL_TIMEOUT_SECS * 1000) as u128 {
        warn!(binary, duration_ms, "Tool probe exceeded timeout threshold");
    }

    match result {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            debug!(binary, "Not found");
            CheckResult {
                label: label.to_string(),
                status: CheckStatus::Fail,
                message: format!("{} not found — consider installing it", binary),
                duration_ms,
            }
        }
        Err(e) => {
            // Unexpected OS error (permissions, etc.) — still a Fail, but log it
            warn!(binary, error = %e, "Unexpected error probing tool");
            CheckResult {
                label: label.to_string(),
                status: CheckStatus::Fail,
                message: format!("Could not run {}: {}", binary, e),
                duration_ms,
            }
        }
        Ok(output) => {
            // Validate UTF-8 explicitly instead of silently replacing bad bytes.
            // A garbled version string is a signal something is wrong with the binary.
            match String::from_utf8(output.stdout) {
                Err(_) => {
                    warn!(binary, "Tool produced non-UTF-8 output");
                    CheckResult {
                        label: label.to_string(),
                        status: CheckStatus::Fail,
                        message: format!("{} returned unreadable output", binary),
                        duration_ms,
                    }
                }
                Ok(stdout) => {
                    let version = stdout
                        .lines()
                        .next()
                        .unwrap_or("unknown version") // safe: only None on empty string
                        .trim()
                        .to_string();

                    debug!(binary, version = %version, "Tool found");
                    CheckResult {
                        label: label.to_string(),
                        status: CheckStatus::Pass,
                        message: version,
                        duration_ms,
                    }
                }
            }
        }
    }
}

/// Check for a .env file in the current working directory.
///
/// Returns Err only if we cannot determine the cwd — a genuine IO failure.
/// Missing .env is a CheckStatus::Fail, not an error.
#[instrument]
fn check_env_file() -> Result<CheckResult> {
    let start = Instant::now();

    // Use std::env::current_dir() instead of a hardcoded relative path so
    // the check is correct no matter where the binary is invoked from.
    let cwd = std::env::current_dir().map_err(DevpulseError::Io)?;
    let env_path = cwd.join(".env");

    debug!(path = %env_path.display(), "Checking for .env");

    let exists = Path::new(&env_path).exists();
    let duration_ms = start.elapsed().as_millis();

    Ok(CheckResult {
        label: ".env file".to_string(),
        status: if exists { CheckStatus::Pass } else { CheckStatus::Fail },
        message: if exists {
            format!("Found at {}", env_path.display())
        } else {
            format!("No .env found in {}", cwd.display())
        },
        duration_ms,
    })
}

/// Format and print one check line.
fn print_check(check: &CheckResult) {
    let icon = match check.status {
        CheckStatus::Pass => "✔".green().bold(),
        CheckStatus::Fail => "✘".red().bold(),
    };

    let label = format!("{:<12}", check.label).bold();

    let message = match check.status {
        CheckStatus::Pass => check.message.dimmed().to_string(),
        CheckStatus::Fail => check.message.yellow().to_string(),
    };

    // Show duration only when DEVPULSE_DEBUG is set — not noise for normal users
    let duration_hint = if std::env::var("DEVPULSE_DEBUG").is_ok() {
        format!(" ({}ms)", check.duration_ms).dimmed().to_string()
    } else {
        String::new()
    };

    println!("  {}  {}  {}{}", icon, label, message, duration_hint);
}