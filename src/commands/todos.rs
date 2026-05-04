use crate::error::{DevpulseError, Result};
use colored::Colorize;
use std::collections::BTreeMap;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use tracing::{debug, info, instrument, warn};
use walkdir::WalkDir;

const TODO_TAGS: &[&str] = &["TODO", "FIXME", "HACK", "NOTE", "XXX"];

const ALLOWED_EXTENSIONS: &[&str] = &[
    "rs", "ts", "js", "tsx", "jsx", "py", "go", "java", "c", "cpp", "h", "cs", "rb", "php",
    "swift", "kt", "toml", "yaml", "yml", "md",
];

#[derive(Debug)]
pub struct TodoEntry {
    pub tag: String,
    pub line_number: usize,
    pub text: String,
}

#[instrument(name = "todos_scan", skip(path))]
pub fn run(path: &str) -> Result<()> {
    let scan_path = PathBuf::from(path);

    if !scan_path.exists() {
        return Err(DevpulseError::Io(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            format!("Path does not exist: {}", scan_path.display()),
        )));
    }

    info!(path = %scan_path.display(), "Starting TODO scan");

    println!("\n{}", "devpulse todos scan".bold().underline());
    println!(
        "{}\n",
        format!("Scanning {}...", scan_path.display()).dimmed()
    );

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
        format!(
            "Found {} TODO(s) across {} file(s)",
            total_todos, total_files
        )
        .bold()
    );
    println!();

    info!(total_todos, total_files, "TODO scan complete");
    Ok(())
}

#[instrument(skip(root))]
pub fn scan_directory(root: &Path) -> Result<BTreeMap<String, Vec<TodoEntry>>> {
    let mut results: BTreeMap<String, Vec<TodoEntry>> = BTreeMap::new();

    for entry in WalkDir::new(root)
        .follow_links(false)
        .into_iter()
        .filter_map(|e| match e {
            Ok(entry) => Some(entry),
            Err(err) => {
                warn!(error = %err, "Could not access entry, skipping");
                None
            }
        })
    {
        if !entry.file_type().is_file() {
            continue;
        }

        let file_path = entry.path();

        if !is_allowed_file(file_path) {
            debug!(path = %file_path.display(), "Skipping file");
            continue;
        }

        if is_ignored_path(file_path) {
            debug!(path = %file_path.display(), "Skipping ignored path");
            continue;
        }

        match scan_file(file_path) {
            Ok(entries) if !entries.is_empty() => {
                let display_path = file_path
                    .strip_prefix(root)
                    .unwrap_or(file_path)
                    .display()
                    .to_string();

                debug!(path = %display_path, count = entries.len(), "Found TODOs");
                results.insert(display_path, entries);
            }
            Ok(_) => {}
            Err(e) => {
                warn!(path = %file_path.display(), error = %e, "Could not read file");
            }
        }
    }

    Ok(results)
}

pub fn scan_file(path: &Path) -> Result<Vec<TodoEntry>> {
    let file = File::open(path).map_err(DevpulseError::Io)?;
    let reader = BufReader::new(file);
    let mut entries = Vec::new();

    for (index, line_result) in reader.lines().enumerate() {
        let line = match line_result {
            Ok(l) => l,
            Err(e) => {
                warn!(path = %path.display(), line = index + 1, error = %e, "Could not read line");
                continue;
            }
        };

        if let Some(entry) = extract_todo(&line, index + 1) {
            entries.push(entry);
        }
    }

    Ok(entries)
}

pub fn extract_todo(line: &str, line_number: usize) -> Option<TodoEntry> {
    let trimmed = line.trim();
    let comment_start = find_comment_start(trimmed)?;
    let comment_text = &trimmed[comment_start..];

    for &tag in TODO_TAGS {
        if let Some(pos) = comment_text.find(tag) {
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

pub fn find_comment_start(line: &str) -> Option<usize> {
    if line.starts_with("///") {
        return Some(3);
    }
    for marker in &["//", "/*", "<!--", "# ", "##"] {
        if let Some(pos) = line.find(marker) {
            return Some(pos + marker.len());
        }
    }
    None
}

pub fn is_allowed_file(path: &Path) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| ALLOWED_EXTENSIONS.contains(&ext))
        .unwrap_or(false)
}

pub fn is_ignored_path(path: &Path) -> bool {
    path.components().any(|component| {
        matches!(
            component.as_os_str().to_str().unwrap_or(""),
            ".git" | "node_modules" | "target" | ".next" | "dist" | "build" | ".cache"
        )
    })
}

fn print_results(results: &BTreeMap<String, Vec<TodoEntry>>) {
    for (file_path, entries) in results {
        println!("  {}", file_path.cyan().bold());

        for entry in entries {
            let tag_colored = match entry.tag.as_str() {
                "FIXME" => entry.tag.red().bold(),
                "HACK" => entry.tag.yellow().bold(),
                "XXX" => entry.tag.red().bold(),
                _ => entry.tag.blue().bold(),
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

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    // ── extract_todo ─────────────────────────────────────────────────────────

    #[test]
    fn test_extract_todo_finds_basic_todo() {
        let result = extract_todo("    // TODO: fix this", 1);
        assert!(result.is_some());
        let entry = result.unwrap();
        assert_eq!(entry.tag, "TODO");
        assert_eq!(entry.text, "fix this");
        assert_eq!(entry.line_number, 1);
    }

    #[test]
    fn test_extract_todo_finds_fixme() {
        let result = extract_todo("    // FIXME: this panics on empty input", 5);
        assert!(result.is_some());
        let entry = result.unwrap();
        assert_eq!(entry.tag, "FIXME");
        assert_eq!(entry.text, "this panics on empty input");
    }

    #[test]
    fn test_extract_todo_finds_hack() {
        let result = extract_todo("// HACK: workaround for slow API", 10);
        assert!(result.is_some());
        let entry = result.unwrap();
        assert_eq!(entry.tag, "HACK");
        assert_eq!(entry.text, "workaround for slow API");
    }

    #[test]
    fn test_extract_todo_finds_note() {
        let result = extract_todo("// NOTE: this is intentional", 3);
        assert!(result.is_some());
        let entry = result.unwrap();
        assert_eq!(entry.tag, "NOTE");
        assert_eq!(entry.text, "this is intentional");
    }

    #[test]
    fn test_extract_todo_ignores_string_literal() {
        // "TODO" inside a string is NOT in a comment — should be ignored
        let result = extract_todo("    let label = \"TODO\";", 1);
        assert!(result.is_none());
    }

    #[test]
    fn test_extract_todo_ignores_plain_code_line() {
        let result = extract_todo("    let x = 42;", 1);
        assert!(result.is_none());
    }

    #[test]
    fn test_extract_todo_does_not_match_todos_word() {
        // "TODOS" should NOT match the "TODO" tag (word boundary check)
        let result = extract_todo("// TODOS are important", 1);
        assert!(result.is_none());
    }

    #[test]
    fn test_extract_todo_handles_no_colon() {
        // TODO without colon should still be found
        let result = extract_todo("// TODO fix this later", 1);
        assert!(result.is_some());
        let entry = result.unwrap();
        assert_eq!(entry.tag, "TODO");
        assert_eq!(entry.text, "fix this later");
    }

    #[test]
    fn test_extract_todo_handles_hash_comment() {
        // Python / YAML style comments
        let result = extract_todo("# TODO: refactor this", 1);
        assert!(result.is_some());
        let entry = result.unwrap();
        assert_eq!(entry.tag, "TODO");
        assert_eq!(entry.text, "refactor this");
    }

    #[test]
    fn test_extract_todo_handles_block_comment() {
        let result = extract_todo("/* TODO: clean up */", 1);
        assert!(result.is_some());
        let entry = result.unwrap();
        assert_eq!(entry.tag, "TODO");
        // Trailing */ should be stripped
        assert_eq!(entry.text, "clean up");
    }

    #[test]
    fn test_extract_todo_empty_text_after_tag() {
        let result = extract_todo("// TODO", 1);
        assert!(result.is_some());
        let entry = result.unwrap();
        assert_eq!(entry.tag, "TODO");
        assert_eq!(entry.text, "");
    }

    // ── find_comment_start ───────────────────────────────────────────────────

    #[test]
    fn test_find_comment_start_slash_slash() {
        let pos = find_comment_start("// some comment");
        assert!(pos.is_some());
    }

    #[test]
    fn test_find_comment_start_hash() {
        let pos = find_comment_start("# some comment");
        assert!(pos.is_some());
    }

    #[test]
    fn test_find_comment_start_no_comment() {
        let pos = find_comment_start("let x = 5;");
        assert!(pos.is_none());
    }

    #[test]
    fn test_find_comment_start_doc_comment() {
        let pos = find_comment_start("/// doc comment");
        assert!(pos.is_some());
    }

    // ── is_allowed_file ──────────────────────────────────────────────────────

    #[test]
    fn test_is_allowed_file_rust() {
        assert!(is_allowed_file(Path::new("main.rs")));
    }

    #[test]
    fn test_is_allowed_file_typescript() {
        assert!(is_allowed_file(Path::new("app.ts")));
    }

    #[test]
    fn test_is_allowed_file_rejects_lock_file() {
        assert!(!is_allowed_file(Path::new("Cargo.lock")));
    }

    #[test]
    fn test_is_allowed_file_rejects_binary() {
        assert!(!is_allowed_file(Path::new("devpulse")));
    }

    #[test]
    fn test_is_allowed_file_rejects_no_extension() {
        assert!(!is_allowed_file(Path::new("Makefile")));
    }

    // ── is_ignored_path ──────────────────────────────────────────────────────

    #[test]
    fn test_is_ignored_path_node_modules() {
        assert!(is_ignored_path(Path::new(
            "project/node_modules/lib/index.js"
        )));
    }

    #[test]
    fn test_is_ignored_path_target() {
        assert!(is_ignored_path(Path::new("project/target/debug/main")));
    }

    #[test]
    fn test_is_ignored_path_git() {
        assert!(is_ignored_path(Path::new("project/.git/config")));
    }

    #[test]
    fn test_is_ignored_path_allows_src() {
        assert!(!is_ignored_path(Path::new("project/src/main.rs")));
    }

    // ── scan_file ────────────────────────────────────────────────────────────

    #[test]
    fn test_scan_file_finds_todos_in_real_file() {
        // Write a temp file with known TODOs and verify scan_file finds them
        let dir = TempDir::new().unwrap();
        let file_path = dir.path().join("test.rs");

        fs::write(
            &file_path,
            r#"
fn main() {
    // TODO: implement this
    let x = 1;
    // FIXME: this is broken
    let y = 2; // HACK: shortcut
}
"#,
        )
        .unwrap();

        let entries = scan_file(&file_path).unwrap();
        assert_eq!(entries.len(), 3);
        assert_eq!(entries[0].tag, "TODO");
        assert_eq!(entries[1].tag, "FIXME");
        assert_eq!(entries[2].tag, "HACK");
    }

    #[test]
    fn test_scan_file_empty_file_returns_no_entries() {
        let dir = TempDir::new().unwrap();
        let file_path = dir.path().join("empty.rs");
        fs::write(&file_path, "").unwrap();

        let entries = scan_file(&file_path).unwrap();
        assert!(entries.is_empty());
    }

    #[test]
    fn test_scan_file_clean_file_returns_no_entries() {
        let dir = TempDir::new().unwrap();
        let file_path = dir.path().join("clean.rs");
        fs::write(&file_path, "fn main() {\n    println!(\"hello\");\n}\n").unwrap();

        let entries = scan_file(&file_path).unwrap();
        assert!(entries.is_empty());
    }

    // ── scan_directory ───────────────────────────────────────────────────────

    #[test]
    fn test_scan_directory_finds_todos_recursively() {
        let dir = TempDir::new().unwrap();

        // Create a nested structure
        let sub = dir.path().join("src");
        fs::create_dir(&sub).unwrap();

        fs::write(sub.join("main.rs"), "// TODO: top level\n").unwrap();
        fs::write(sub.join("lib.rs"), "fn clean() {}\n").unwrap();

        let results = scan_directory(dir.path()).unwrap();
        assert_eq!(results.len(), 1); // only main.rs has a TODO
        assert!(results.keys().any(|k| k.contains("main.rs")));
    }

    #[test]
    fn test_scan_directory_skips_ignored_dirs() {
        let dir = TempDir::new().unwrap();

        // Put a TODO inside node_modules — should be skipped
        let ignored = dir.path().join("node_modules");
        fs::create_dir(&ignored).unwrap();
        fs::write(ignored.join("lib.js"), "// TODO: inside ignored dir\n").unwrap();

        // Put a real TODO in src
        let src = dir.path().join("src");
        fs::create_dir(&src).unwrap();
        fs::write(src.join("app.rs"), "// TODO: real todo\n").unwrap();

        let results = scan_directory(dir.path()).unwrap();

        // Should find the src TODO but NOT the node_modules one
        assert_eq!(results.len(), 1);
        assert!(results.keys().any(|k| k.contains("app.rs")));
        assert!(!results.keys().any(|k| k.contains("node_modules")));
    }

    #[test]
    fn test_scan_directory_empty_dir_returns_empty() {
        let dir = TempDir::new().unwrap();
        let results = scan_directory(dir.path()).unwrap();
        assert!(results.is_empty());
    }
}
