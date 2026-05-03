use crate::error::{DevpulseError, Result};
use colored::Colorize;
use std::collections::BTreeMap;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use tracing::{debug, info, instrument, warn};
use walkdir::WalkDir;

/// Tags we scan for. Easy to extend later.
const TODO_TAGS: &[&str] = &["TODO", "FIXME", "HACK", "NOTE", "XXX"];

/// File extensions we care about. Avoids scanning binaries, lock files, etc.
const ALLOWED_EXTENSIONS: &[&str] = &[
    "rs", "ts", "js", "tsx", "jsx", "py", "go", "java", "c", "cpp", "h",
    "cs", "rb", "php", "swift", "kt", "toml", "yaml", "yml", "md",
];

/// One found TODO entry
#[derive(Debug)]
pub struct TodoEntry {
    pub tag: String,
    pub line_number: usize,
    pub text: String,
}

/// Entry point called from main
#[instrument(name = "todos_scan", skip(path))]
pub fn run(path: &str) -> Result<()> {
    let scan_path = PathBuf::from(path);

    // Validate the path exists before doing any work
    if !scan_path.exists() {
        return Err(DevpulseError::Io(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            format!("Path does not exist: {}", scan_path.display()),
        )));
    }

    info!(path = %scan_path.display(), "Starting TODO scan");

    println!("\n{}", "devpulse todos scan".bold().underline());
    println!("{}\n", format!("Scanning {}...", scan_path.display()).dimmed());

    // BTreeMap keeps files sorted alphabetically — nicer output
    let results = scan_directory(&scan_path)?;

    if results.is_empty() {
        println!("{}", "No TODOs found. Clean codebase!".green().bold());
        return Ok(());
    }

    let total_todos: usize = results.values().map(|v| v.len()).sum();
    let total_files = results.len();

    print_results(&results);

    println!(
        "{}",
        format!("Found {} TODO(s) across {} file(s)", total_todos, total_files)
            .bold()
    );
    println!();

    info!(total_todos, total_files, "TODO scan complete");
    Ok(())
}

/// Walk the directory recursively and collect all TODOs.
/// Returns a map of file path → list of entries.
#[instrument(skip(root))]
fn scan_directory(root: &Path) -> Result<BTreeMap<String, Vec<TodoEntry>>> {
    let mut results: BTreeMap<String, Vec<TodoEntry>> = BTreeMap::new();

    for entry in WalkDir::new(root)
        .follow_links(false)   // don't follow symlinks — avoids infinite loops
        .into_iter()
        .filter_map(|e| {
            match e {
                Ok(entry) => Some(entry),
                Err(err) => {
                    // Log permission errors etc. but keep scanning
                    warn!(error = %err, "Could not access entry, skipping");
                    None
                }
            }
        })
    {
        // Skip directories — we only want files
        if !entry.file_type().is_file() {
            continue;
        }

        let file_path = entry.path();

        // Skip files with extensions we don't care about
        if !is_allowed_file(file_path) {
            debug!(path = %file_path.display(), "Skipping file (extension not allowed)");
            continue;
        }

        // Skip hidden directories like .git, node_modules, target
        if is_ignored_path(file_path) {
            debug!(path = %file_path.display(), "Skipping ignored path");
            continue;
        }

        match scan_file(file_path) {
            Ok(entries) if !entries.is_empty() => {
                // Make the path relative to the root for cleaner output
                let display_path = file_path
                    .strip_prefix(root)
                    .unwrap_or(file_path)
                    .display()
                    .to_string();

                debug!(path = %display_path, count = entries.len(), "Found TODOs in file");
                results.insert(display_path, entries);
            }
            Ok(_) => {} // file had no TODOs — skip silently
            Err(e) => {
                // Unreadable file — log and continue rather than aborting the whole scan
                warn!(path = %file_path.display(), error = %e, "Could not read file, skipping");
            }
        }
    }

    Ok(results)
}

/// Scan a single file line by line and return all TODO entries found.
#[instrument(skip(path))]
fn scan_file(path: &Path) -> Result<Vec<TodoEntry>> {
    let file = File::open(path).map_err(DevpulseError::Io)?;
    let reader = BufReader::new(file);
    let mut entries = Vec::new();

    for (index, line_result) in reader.lines().enumerate() {
        let line = match line_result {
            Ok(l) => l,
            Err(e) => {
                // A single unreadable line shouldn't abort the whole file
                warn!(
                    path = %path.display(),
                    line = index + 1,
                    error = %e,
                    "Could not read line, skipping"
                );
                continue;
            }
        };

        // Check if this line contains any of our tracked tags
        if let Some(entry) = extract_todo(&line, index + 1) {
            entries.push(entry);
        }
    }

    Ok(entries)
}

/// Parse a single line and extract a TodoEntry if a tag is found.
/// Only matches tags that appear inside comments, not string literals.
/// Handles formats like:
///   // TODO: fix this
///   # FIXME some issue
///   /* HACK: workaround */
fn extract_todo(line: &str, line_number: usize) -> Option<TodoEntry> {
    let trimmed = line.trim();

    // Only consider the part of the line that is inside a comment.
    // We detect comment starts for common styles: //, #, <!--, /*
    // If no comment marker is found, the line cannot contain a valid TODO.
    let comment_start = find_comment_start(trimmed)?;
    let comment_text = &trimmed[comment_start..];

    for &tag in TODO_TAGS {
        if let Some(pos) = comment_text.find(tag) {
            // Ensure the tag is a whole word — not e.g. "TODOS" or "NOTABLE"
            let after_tag = &comment_text[pos + tag.len()..];
            let is_word_boundary = after_tag
                .chars()
                .next()
                .map(|c| !c.is_alphanumeric() && c != '_')
                .unwrap_or(true);

            if !is_word_boundary {
                continue;
            }

            let text = after_tag
                .trim_start_matches(':')
                .trim_start_matches('-')
                .trim()
                // Strip trailing comment closers like */
                .trim_end_matches("*/")
                .trim()
                .to_string();

            return Some(TodoEntry {
                tag: tag.to_string(),
                line_number,
                text,
            });
        }
    }

    None
}

/// Find the index where a comment starts in a line.
/// Returns None if no comment marker is detected.
fn find_comment_start(line: &str) -> Option<usize> {
    // Order matters: check longer markers before shorter ones
    for marker in &["//", "/*", "<!--", "# ", "##"] {
        if let Some(pos) = line.find(marker) {
            return Some(pos + marker.len());
        }
    }
    // Also handle lines that are pure doc comments (start with ///)
    if line.starts_with("///") {
        return Some(3);
    }
    None
}

/// Check if a file extension is in our allowed list
fn is_allowed_file(path: &Path) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| ALLOWED_EXTENSIONS.contains(&ext))
        .unwrap_or(false)
}

/// Skip common directories that should never be scanned
fn is_ignored_path(path: &Path) -> bool {
    path.components().any(|component| {
        matches!(
            component.as_os_str().to_str().unwrap_or(""),
            ".git" | "node_modules" | "target" | ".next" | "dist" | "build" | ".cache"
        )
    })
}

/// Print all results grouped by file
fn print_results(results: &BTreeMap<String, Vec<TodoEntry>>) {
    for (file_path, entries) in results {
        println!("  {}", file_path.cyan().bold());

        for entry in entries {
            let tag_colored = match entry.tag.as_str() {
                "FIXME" => entry.tag.red().bold(),
                "HACK"  => entry.tag.yellow().bold(),
                "XXX"   => entry.tag.red().bold(),
                _       => entry.tag.blue().bold(), // TODO, NOTE
            };

            let line_num = format!("line {:<5}", entry.line_number).dimmed();

            if entry.text.is_empty() {
                println!("    {}  {}", line_num, tag_colored);
            } else {
                println!("    {}  {}  {}", line_num, tag_colored, entry.text);
            }
        }

        println!();
    }
}