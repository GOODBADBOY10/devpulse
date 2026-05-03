use crate::error::{DevpulseError, Result};
use colored::Colorize;
use dirs::home_dir;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use std::process::Command;
use tracing::{debug, info, instrument, warn};

// ─── Data structures ────────────────────────────────────────────────────────

/// Everything we know about the current project session.
/// Derives Serialize/Deserialize so serde_json can save/load it automatically.
#[derive(Debug, Serialize, Deserialize)]
pub struct ProjectSession {
    pub project_name: String,
    pub project_path: String,
    pub last_seen: String,       // ISO 8601 timestamp
    pub git: Option<GitContext>,
    pub todo_count: Option<usize>,
}

/// Git state captured at session save time.
#[derive(Debug, Serialize, Deserialize)]
pub struct GitContext {
    pub branch: String,
    pub uncommitted_changes: usize,
    pub last_commit_message: String,
    pub last_commit_author: String,
}

// ─── Entry point ────────────────────────────────────────────────────────────

#[instrument(name = "context_show")]
pub fn run() -> Result<()> {
    info!("Starting context show");

    let cwd = std::env::current_dir().map_err(DevpulseError::Io)?;

    println!("\n{}", "devpulse context".bold().underline());
    println!("{}\n", format!("Project: {}", cwd.display()).dimmed());

    // Gather everything about the current project
    let git = gather_git_context(&cwd);
    let todo_count = count_todos(&cwd);
    let session = build_session(&cwd, git, todo_count)?;

    // Save the session to ~/.devpulse/<project>.json
    save_session(&session)?;

    // Print it all out
    print_context(&session);

    info!(project = %session.project_name, "Context captured");
    Ok(())
}

// ─── Git ────────────────────────────────────────────────────────────────────

/// Run git commands to gather branch, changes, and last commit info.
/// Returns None gracefully if we're not in a git repo — not an error.
#[instrument(skip(cwd))]
fn gather_git_context(cwd: &PathBuf) -> Option<GitContext> {
    let branch = run_git(&["rev-parse", "--abbrev-ref", "HEAD"], cwd)?;
    debug!(branch = %branch, "Got git branch");

    // Count uncommitted changes (staged + unstaged)
    let status_output = run_git(&["status", "--porcelain"], cwd)?;
    let uncommitted_changes = if status_output.is_empty() {
        0
    } else {
        status_output.lines().count()
    };

    // Get the last commit message
    let last_commit_message = run_git(
        &["log", "-1", "--pretty=format:%s"],
        cwd,
    )
    .unwrap_or_else(|| "No commits yet".to_string());

    // Get the last commit author
    let last_commit_author = run_git(
        &["log", "-1", "--pretty=format:%an"],
        cwd,
    )
    .unwrap_or_else(|| "Unknown".to_string());

    Some(GitContext {
        branch,
        uncommitted_changes,
        last_commit_message,
        last_commit_author,
    })
}

/// Run a git subcommand and return its trimmed stdout.
/// Returns None if git isn't found, the command fails, or we're not in a repo.
fn run_git(args: &[&str], cwd: &PathBuf) -> Option<String> {
    let output = Command::new("git")
        .args(args)
        .current_dir(cwd)
        .stderr(std::process::Stdio::null()) // silence "not a git repo" noise
        .output();

    match output {
        Err(e) => {
            warn!(error = %e, "Failed to run git");
            None
        }
        Ok(out) if !out.status.success() => {
            debug!(args = ?args, "Git command returned non-zero");
            None
        }
        Ok(out) => match String::from_utf8(out.stdout) {
            Ok(s) => Some(s.trim().to_string()),
            Err(_) => {
                warn!("Git output was not valid UTF-8");
                None
            }
        },
    }
}

// ─── TODO count ─────────────────────────────────────────────────────────────

/// Quick TODO count for the context snapshot.
/// Uses walkdir directly rather than calling the todos command
/// so we keep the two commands independently testable.
fn count_todos(cwd: &PathBuf) -> Option<usize> {
    use walkdir::WalkDir;

    let tags = ["TODO", "FIXME", "HACK"];
    let mut count = 0;

    for entry in WalkDir::new(cwd)
        .follow_links(false)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        if !entry.file_type().is_file() {
            continue;
        }

        // Skip non-source files and ignored dirs
        let path = entry.path();
        if is_ignored(path) {
            continue;
        }

        if let Ok(content) = fs::read_to_string(path) {
            for line in content.lines() {
                if tags.iter().any(|t| line.contains(t)) {
                    count += 1;
                }
            }
        }
    }

    Some(count)
}

fn is_ignored(path: &std::path::Path) -> bool {
    path.components().any(|c| {
        matches!(
            c.as_os_str().to_str().unwrap_or(""),
            ".git" | "node_modules" | "target" | ".next" | "dist" | "build"
        )
    })
}

// ─── Session ─────────────────────────────────────────────────────────────────

/// Build the session struct from gathered data.
fn build_session(
    cwd: &PathBuf,
    git: Option<GitContext>,
    todo_count: Option<usize>,
) -> Result<ProjectSession> {
    let project_name = cwd
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("unknown")
        .to_string();

    // Timestamp without pulling in chrono — good enough for display
    let last_seen = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| format!("{}", d.as_secs()))
        .unwrap_or_else(|_| "unknown".to_string());

    Ok(ProjectSession {
        project_name,
        project_path: cwd.display().to_string(),
        last_seen,
        git,
        todo_count,
    })
}

/// Save session as JSON to ~/.devpulse/<project_name>.json
/// This is where NoHomeDir finally gets used.
#[instrument(skip(session))]
fn save_session(session: &ProjectSession) -> Result<()> {
    let home = home_dir().ok_or(DevpulseError::NoHomeDir)?;
    let store_dir = home.join(".devpulse");

    // Create ~/.devpulse/ if it doesn't exist yet
    fs::create_dir_all(&store_dir).map_err(DevpulseError::Io)?;

    let file_path = store_dir.join(format!("{}.json", session.project_name));
    let json = serde_json::to_string_pretty(session)?;

    fs::write(&file_path, json).map_err(DevpulseError::Io)?;

    debug!(path = %file_path.display(), "Session saved");
    Ok(())
}

/// Load a previous session for this project if one exists.
/// Returns None silently if no session found — first run is fine.
pub fn load_session(project_name: &str) -> Option<ProjectSession> {
    let home = home_dir()?;
    let file_path = home
        .join(".devpulse")
        .join(format!("{}.json", project_name));

    let content = fs::read_to_string(&file_path).ok()?;
    match serde_json::from_str(&content) {
        Ok(session) => Some(session),
        Err(e) => {
            warn!(error = %e, path = %file_path.display(), "Could not parse saved session");
            None
        }
    }
}

// ─── Display ─────────────────────────────────────────────────────────────────

fn print_context(session: &ProjectSession) {
    // Git section
    match &session.git {
        None => {
            println!("  {}  {}", "◆".dimmed(), "Not a git repository".dimmed());
        }
        Some(git) => {
            println!("  {}  {}", "◆ Branch".bold(), git.branch.cyan().bold());

            let changes_label = if git.uncommitted_changes == 0 {
                "Clean".green().to_string()
            } else {
                format!("{} uncommitted change(s)", git.uncommitted_changes)
                    .yellow()
                    .to_string()
            };
            println!("  {}    {}", "◆ Changes".bold(), changes_label);

            println!(
                "  {}  {} {}",
                "◆ Last commit".bold(),
                git.last_commit_message.dimmed(),
                format!("({})", git.last_commit_author).dimmed()
            );
        }
    }

    // TODO count
    if let Some(count) = session.todo_count {
        let label = if count == 0 {
            "No TODOs found".green().to_string()
        } else {
            format!("{} TODO/FIXME/HACK(s) in project", count)
                .yellow()
                .to_string()
        };
        println!("  {}  {}", "◆ TODOs".bold(), label);
    }

    // Session saved hint
    println!();
    println!(
        "  {}",
        format!("Session saved to ~/.devpulse/{}.json", session.project_name).dimmed()
    );
    println!();
}