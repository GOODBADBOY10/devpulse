use crate::error::{DevpulseError, Result};
use colored::Colorize;
use std::path::Path;
use std::process::Command;
use std::time::Instant;
use tracing::{debug, info, instrument, warn};

const TOOL_TIMEOUT_SECS: u64 = 5;

#[derive(Debug)]
pub struct CheckResult {
    pub label: String,
    pub status: CheckStatus,
    pub message: String,
    pub duration_ms: u128,
}

#[derive(Debug, PartialEq)]
pub enum CheckStatus {
    Pass,
    Fail,
}

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
        println!(
            "{}",
            format!("{} issue(s) need attention.", total - passed).yellow().bold()
        );
    }

    println!();
    info!(passed, total, "Health check complete");
    Ok(())
}

pub fn collect_checks() -> Result<Vec<CheckResult>> {
    Ok(vec![
        check_tool("Rust", "rustc"),
        check_tool("Cargo", "cargo"),
        check_tool("Git", "git"),
        check_tool("Node.js", "node"),
        check_tool("Docker", "docker"),
        check_env_file()?,
    ])
}

/// Probe a binary by running `<binary> --version`.
/// Falls back to stderr if stdout is empty (fixes Docker on some systems).
#[instrument(skip_all, fields(binary))]
pub fn check_tool(label: &str, binary: &str) -> CheckResult {
    debug!(binary, "Probing tool");
    let start = Instant::now();

    let result = Command::new(binary)
        .arg("--version")
        // Capture stderr too so Docker's version output is never lost
        .stderr(std::process::Stdio::piped())
        .output();

    let duration_ms = start.elapsed().as_millis();

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
            warn!(binary, error = %e, "Unexpected error probing tool");
            CheckResult {
                label: label.to_string(),
                status: CheckStatus::Fail,
                message: format!("Could not run {}: {}", binary, e),
                duration_ms,
            }
        }
        Ok(output) => {
            // Try stdout first; fall back to stderr (Docker writes version to stderr)
            let raw = if !output.stdout.is_empty() {
                output.stdout
            } else {
                output.stderr
            };

            match String::from_utf8(raw) {
                Err(_) => {
                    warn!(binary, "Tool produced non-UTF-8 output");
                    CheckResult {
                        label: label.to_string(),
                        status: CheckStatus::Fail,
                        message: format!("{} returned unreadable output", binary),
                        duration_ms,
                    }
                }
                Ok(out) => {
                    let version = out
                        .lines()
                        .next()
                        .unwrap_or("unknown version")
                        .trim()
                        .to_string();

                    // If still empty after fallback, mark as fail with a clear message
                    if version.is_empty() {
                        warn!(binary, "Tool returned empty version string");
                        return CheckResult {
                            label: label.to_string(),
                            status: CheckStatus::Fail,
                            message: format!("{} returned an empty version — it may not be installed correctly", binary),
                            duration_ms,
                        };
                    }

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

pub fn check_env_file() -> Result<CheckResult> {
    let start = Instant::now();
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

    let duration_hint = if std::env::var("DEVPULSE_DEBUG").is_ok() {
        format!(" ({}ms)", check.duration_ms).dimmed().to_string()
    } else {
        String::new()
    };

    println!("  {}  {}  {}{}", icon, label, message, duration_hint);
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    // ── check_tool ───────────────────────────────────────────────────────────

    #[test]
    fn test_check_tool_finds_existing_binary() {
        // `git` is always available in CI and dev environments
        let result = check_tool("Git", "git");
        assert_eq!(result.status, CheckStatus::Pass);
        assert!(!result.message.is_empty());
    }

    #[test]
    fn test_check_tool_fails_for_missing_binary() {
        let result = check_tool("Fake", "this-binary-does-not-exist-devpulse");
        assert_eq!(result.status, CheckStatus::Fail);
        assert!(result.message.contains("not found"));
    }

    #[test]
    fn test_check_tool_label_is_preserved() {
        let result = check_tool("MyLabel", "git");
        assert_eq!(result.label, "MyLabel");
    }

    #[test]
    fn test_check_tool_duration_is_recorded() {
        let result = check_tool("Git", "git");
        // Duration should be >= 0 (it always will be, but this confirms the field exists)
        assert!(result.duration_ms < 10_000); // sanity: should finish in under 10s
    }

    // ── check_env_file ───────────────────────────────────────────────────────

    #[test]
    fn test_check_env_file_passes_when_env_exists() {
        let dir = TempDir::new().unwrap();
        let env_path = dir.path().join(".env");
        fs::write(&env_path, "DATABASE_URL=postgres://localhost/mydb\n").unwrap();

        // Temporarily change working directory to the temp dir
        let original = std::env::current_dir().unwrap();
        std::env::set_current_dir(dir.path()).unwrap();

        let result = check_env_file().unwrap();
        assert_eq!(result.status, CheckStatus::Pass);
        assert!(result.message.contains(".env"));

        std::env::set_current_dir(original).unwrap();
    }

    #[test]
    fn test_check_env_file_fails_when_env_missing() {
        let dir = TempDir::new().unwrap();

        let original = std::env::current_dir().unwrap();
        std::env::set_current_dir(dir.path()).unwrap();

        let result = check_env_file().unwrap();
        assert_eq!(result.status, CheckStatus::Fail);
        assert!(result.message.contains("No .env"));

        std::env::set_current_dir(original).unwrap();
    }

    // ── collect_checks ───────────────────────────────────────────────────────

    #[test]
    fn test_collect_checks_returns_six_checks() {
        let checks = collect_checks().unwrap();
        assert_eq!(checks.len(), 6);
    }

    #[test]
    fn test_collect_checks_has_expected_labels() {
        let checks = collect_checks().unwrap();
        let labels: Vec<&str> = checks.iter().map(|c| c.label.as_str()).collect();
        assert!(labels.contains(&"Rust"));
        assert!(labels.contains(&"Git"));
        assert!(labels.contains(&"Docker"));
        assert!(labels.contains(&".env file"));
    }

    // ── CheckStatus ──────────────────────────────────────────────────────────

    #[test]
    fn test_check_status_pass_is_not_fail() {
        assert_ne!(CheckStatus::Pass, CheckStatus::Fail);
    }
}