//! gnb: Create git branches with username prefix.
//!
//! Creates branches in the format: `username/branch-name` with automatic
//! collision detection and suffix numbering (_2, _3, etc.).

use anyhow::{Context, Result};
use chrono::Local;
use clap::Parser;
use colored::Colorize;
use std::collections::HashSet;
use std::process::{Command, ExitCode};

/// Create a new git branch prefixed with your username.
///
/// Examples:
///   gnb           → username/YYMMDD (or _2, _3 if exists)
///   gnb ABC-123   → username/ABC-123 (or _2, _3 if exists)
///   gnb "my feature" → username/my-feature
///
/// Override prefix with GNB_PREFIX environment variable.
#[derive(Parser, Debug)]
#[command(name = "gnb")]
#[command(version)]
#[command(about = "Create git branches with username prefix")]
#[command(after_help = "\
Examples:
  gnb            Create username/YYMMDD branch
  gnb ABC-123    Create username/ABC-123 branch
  gnb fix login  Create username/fix-login branch

Environment:
  GNB_PREFIX    Override username prefix (e.g., GNB_PREFIX=ci-bot)

The branch is created from current HEAD. Existing branch names get
a numeric suffix (_2, _3, etc.) to avoid collisions.")]
struct Cli {
    /// Branch name (defaults to YYMMDD date if not provided)
    #[arg(trailing_var_arg = true)]
    name: Vec<String>,
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("{} {:#}", "❌".red(), e);
            ExitCode::from(1)
        }
    }
}

fn run() -> Result<()> {
    let cli = Cli::parse();

    // Ensure we're in a git repo
    ensure_git_repo()?;

    // Get the prefix (username or GNB_PREFIX override)
    let prefix = get_prefix()?;

    // Build the base branch name
    let base = build_base_name(&cli.name);

    // Sanitize the base name
    let sanitized = sanitize(&base);

    // Build candidate branch name
    let candidate = format!("{}/{}", prefix, sanitized);

    // Find an available branch name (handles collisions)
    let existing = collect_existing_branches(&candidate)?;
    let target = pick_available_name(&candidate, &existing)?;

    // Create and switch to the branch
    create_branch(&target)?;

    println!(
        "{} Created and switched to branch: {}",
        "✅".green(),
        target.cyan()
    );

    Ok(())
}

/// Ensure we're inside a git repository.
fn ensure_git_repo() -> Result<()> {
    let output = Command::new("git")
        .args(["rev-parse", "--is-inside-work-tree"])
        .output()
        .context("Failed to run git")?;

    if !output.status.success() {
        anyhow::bail!("Not inside a git repository");
    }

    Ok(())
}

/// Get the branch prefix (username or GNB_PREFIX override).
fn get_prefix() -> Result<String> {
    let mut from_env = false;

    // Check for GNB_PREFIX environment variable first
    let raw_prefix = match std::env::var("GNB_PREFIX") {
        Ok(prefix) if !prefix.trim().is_empty() => {
            from_env = true;
            prefix
        }
        _ => {
            // Fall back to system username
            let output = Command::new("id")
                .arg("-un")
                .output()
                .context("Failed to get username")?;

            if !output.status.success() {
                anyhow::bail!("Failed to determine username");
            }

            String::from_utf8(output.stdout)
                .context("Username was not valid UTF-8")?
                .trim()
                .to_string()
        }
    };

    let sanitized = sanitize_component(raw_prefix.trim());
    if sanitized.is_empty() {
        if from_env {
            anyhow::bail!("GNB_PREFIX is empty or invalid after sanitization");
        }
        anyhow::bail!("Username is empty or invalid after sanitization");
    }

    Ok(sanitized)
}

/// Build the base branch name from CLI arguments.
fn build_base_name(args: &[String]) -> String {
    if args.is_empty() {
        // Default to YYMMDD format
        today_stamp()
    } else {
        // Join all arguments with spaces
        args.join(" ")
    }
}

fn today_stamp() -> String {
    Local::now().format("%y%m%d").to_string()
}

/// Sanitize a single branch name component to be git-compatible.
fn sanitize_component(name: &str) -> String {
    let mut result = String::with_capacity(name.len());
    let mut prev_was_separator = false;
    let mut prev_was_dot = false;

    for mut c in name.chars() {
        if c == '/' {
            c = '-';
        }

        if c.is_whitespace() {
            push_separator(&mut result, &mut prev_was_separator, &mut prev_was_dot);
            continue;
        }

        if !is_allowed_branch_char(c) {
            push_separator(&mut result, &mut prev_was_separator, &mut prev_was_dot);
            continue;
        }

        if c == '.' {
            if result.is_empty() || prev_was_separator {
                push_separator(&mut result, &mut prev_was_separator, &mut prev_was_dot);
                continue;
            }

            if prev_was_dot {
                result.pop();
                push_separator(&mut result, &mut prev_was_separator, &mut prev_was_dot);
                continue;
            }

            result.push('.');
            prev_was_separator = false;
            prev_was_dot = true;
            continue;
        }

        result.push(c);
        prev_was_separator = c == '-';
        prev_was_dot = false;
    }

    let mut cleaned = result.trim_matches(|c| c == '-' || c == '.').to_string();

    if cleaned.ends_with(".lock") {
        let dot_index = cleaned.len() - ".lock".len();
        cleaned.replace_range(dot_index..dot_index + 1, "-");
    }

    cleaned
}

fn is_allowed_branch_char(c: char) -> bool {
    c.is_alphanumeric() || matches!(c, '.' | '_' | '+' | '-')
}

fn push_separator(result: &mut String, prev_was_separator: &mut bool, prev_was_dot: &mut bool) {
    if !*prev_was_separator {
        result.push('-');
        *prev_was_separator = true;
    }
    *prev_was_dot = false;
}

/// Sanitize a branch name to be git-compatible.
fn sanitize(name: &str) -> String {
    let sanitized = sanitize_component(name);
    if sanitized.is_empty() {
        today_stamp()
    } else {
        sanitized
    }
}

/// Collect local and origin branches matching the candidate prefix.
fn collect_existing_branches(candidate: &str) -> Result<HashSet<String>> {
    let mut existing = collect_local_branches()?;

    if has_origin_remote() {
        let remote = collect_remote_branches(candidate)?;
        existing.extend(remote);
    }

    Ok(existing)
}

fn collect_local_branches() -> Result<HashSet<String>> {
    let output = Command::new("git")
        .args(["for-each-ref", "--format=%(refname:short)", "refs/heads"])
        .output()
        .context("Failed to list local branches")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("Failed to list local branches: {}", stderr.trim());
    }

    let stdout =
        String::from_utf8(output.stdout).context("Local branch list was not valid UTF-8")?;
    let mut branches = HashSet::new();
    for line in stdout.lines() {
        let name = line.trim();
        if !name.is_empty() {
            branches.insert(name.to_string());
        }
    }

    Ok(branches)
}

fn collect_remote_branches(candidate: &str) -> Result<HashSet<String>> {
    let pattern = format!("refs/heads/{}*", candidate);
    let output = Command::new("git")
        .args(["ls-remote", "--heads", "origin", &pattern])
        .output()
        .context("Failed to list remote branches")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("Failed to list remote branches: {}", stderr.trim());
    }

    let stdout =
        String::from_utf8(output.stdout).context("Remote branch list was not valid UTF-8")?;
    let mut branches = HashSet::new();
    for line in stdout.lines() {
        let mut parts = line.split_whitespace();
        let _ = parts.next();
        let Some(ref_name) = parts.next() else {
            continue;
        };
        if let Some(short) = ref_name.strip_prefix("refs/heads/") {
            branches.insert(short.to_string());
        }
    }

    Ok(branches)
}

fn has_origin_remote() -> bool {
    Command::new("git")
        .args(["remote", "get-url", "origin"])
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
}

/// Find an available branch name, adding _2, _3, etc. if needed.
fn pick_available_name(candidate: &str, existing: &HashSet<String>) -> Result<String> {
    if !existing.contains(candidate) {
        return Ok(candidate.to_string());
    }

    // Try with numeric suffix
    for i in 2..=100 {
        let with_suffix = format!("{}_{}", candidate, i);
        if !existing.contains(&with_suffix) {
            return Ok(with_suffix);
        }
    }

    anyhow::bail!("Could not find available branch name after 100 attempts");
}

/// Create and switch to a new branch.
fn create_branch(name: &str) -> Result<()> {
    // Try git switch first (modern git)
    let switch = Command::new("git")
        .args(["switch", "-c", name])
        .output()
        .context("Failed to run git switch")?;

    if switch.status.success() {
        return Ok(());
    }

    // Fall back to git checkout -b (older git)
    let checkout = Command::new("git")
        .args(["checkout", "-b", name])
        .output()
        .context("Failed to run git checkout")?;

    if !checkout.status.success() {
        let stderr = String::from_utf8_lossy(&checkout.stderr);
        anyhow::bail!("Failed to create branch: {}", stderr.trim());
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sanitize_simple() {
        assert_eq!(sanitize("ABC-123"), "ABC-123");
        assert_eq!(sanitize("feature"), "feature");
    }

    #[test]
    fn test_sanitize_spaces() {
        assert_eq!(sanitize("fix login bug"), "fix-login-bug");
        assert_eq!(sanitize("multiple   spaces"), "multiple-spaces");
    }

    #[test]
    fn test_sanitize_slashes() {
        assert_eq!(sanitize("feature/sub"), "feature-sub");
    }

    #[test]
    fn test_sanitize_special_chars() {
        assert_eq!(sanitize("fix@#$%bug"), "fix-bug");
        assert_eq!(sanitize("a!b@c#d"), "a-b-c-d");
    }

    #[test]
    fn test_sanitize_allowed_chars() {
        assert_eq!(sanitize("v1.2.3"), "v1.2.3");
        assert_eq!(sanitize("feat_name"), "feat_name");
        assert_eq!(sanitize("test+plus"), "test+plus");
    }

    #[test]
    fn test_sanitize_dot_edges() {
        assert_eq!(sanitize(".leading"), "leading");
        assert_eq!(sanitize("trailing."), "trailing");
        assert_eq!(sanitize("double..dot"), "double-dot");
    }

    #[test]
    fn test_sanitize_lock_suffix() {
        assert_eq!(sanitize("build.lock"), "build-lock");
    }

    #[test]
    fn test_sanitize_empty_fallback() {
        let result = sanitize("!@#$%");
        // Should be a date in YYMMDD format
        assert_eq!(result.len(), 6);
        assert!(result.chars().all(|c| c.is_ascii_digit()));
    }

    #[test]
    fn test_sanitize_trim_dashes() {
        assert_eq!(sanitize("-hello-"), "hello");
        assert_eq!(sanitize("--test--"), "test");
    }

    #[test]
    fn test_build_base_name_empty() {
        let result = build_base_name(&[]);
        assert_eq!(result.len(), 6);
        assert!(result.chars().all(|c| c.is_ascii_digit()));
    }

    #[test]
    fn test_build_base_name_single() {
        assert_eq!(build_base_name(&["ABC-123".to_string()]), "ABC-123");
    }

    #[test]
    fn test_build_base_name_multiple() {
        let args = vec!["fix".to_string(), "login".to_string()];
        assert_eq!(build_base_name(&args), "fix login");
    }

    #[test]
    fn test_pick_available_name_no_collision() {
        let existing = HashSet::new();
        let result = pick_available_name("user/240101", &existing).unwrap();
        assert_eq!(result, "user/240101");
    }

    #[test]
    fn test_pick_available_name_collision() {
        let mut existing = HashSet::new();
        existing.insert("user/240101".to_string());
        existing.insert("user/240101_2".to_string());
        existing.insert("user/240101_3".to_string());

        let result = pick_available_name("user/240101", &existing).unwrap();
        assert_eq!(result, "user/240101_4");
    }
}
