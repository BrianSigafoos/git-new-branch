//! gnb: Create git branches with username prefix.
//!
//! Creates branches in the format: `username/branch-name` with automatic
//! collision detection and suffix numbering (_2, _3, etc.).

use anyhow::{Context, Result};
use chrono::Local;
use clap::Parser;
use colored::Colorize;
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
    let target = pick_available_name(&candidate)?;

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
    // Check for GNB_PREFIX environment variable first
    if let Ok(prefix) = std::env::var("GNB_PREFIX") {
        if !prefix.is_empty() {
            return Ok(prefix);
        }
    }

    // Fall back to system username
    let output = Command::new("id")
        .arg("-un")
        .output()
        .context("Failed to get username")?;

    if !output.status.success() {
        anyhow::bail!("Failed to determine username");
    }

    let username = String::from_utf8(output.stdout)
        .context("Username was not valid UTF-8")?
        .trim()
        .to_string();

    if username.is_empty() {
        anyhow::bail!("Username is empty");
    }

    Ok(username)
}

/// Build the base branch name from CLI arguments.
fn build_base_name(args: &[String]) -> String {
    if args.is_empty() {
        // Default to YYMMDD format
        Local::now().format("%y%m%d").to_string()
    } else {
        // Join all arguments with dashes
        args.join(" ")
    }
}

/// Sanitize a branch name to be git-compatible.
fn sanitize(name: &str) -> String {
    let mut result = String::with_capacity(name.len());

    // Replace slashes with dashes
    let name = name.replace('/', "-");

    // Replace whitespace sequences with single dash
    let mut prev_was_separator = false;
    for c in name.chars() {
        if c.is_whitespace() {
            if !prev_was_separator {
                result.push('-');
                prev_was_separator = true;
            }
        } else if c.is_alphanumeric() || c == '.' || c == '_' || c == '+' || c == '-' {
            result.push(c);
            prev_was_separator = c == '-';
        } else {
            // Skip invalid characters
            if !prev_was_separator {
                result.push('-');
                prev_was_separator = true;
            }
        }
    }

    // Trim leading/trailing dashes
    let result = result.trim_matches('-').to_string();

    // If empty after sanitization, use date fallback
    if result.is_empty() {
        Local::now().format("%y%m%d").to_string()
    } else {
        result
    }
}

/// Check if a branch exists locally or on remote origin.
fn branch_exists(name: &str) -> Result<bool> {
    // Check local branches
    let local = Command::new("git")
        .args([
            "show-ref",
            "--verify",
            "--quiet",
            &format!("refs/heads/{}", name),
        ])
        .status()
        .context("Failed to check local branch")?;

    if local.success() {
        return Ok(true);
    }

    // Check if origin remote exists
    let has_origin = Command::new("git")
        .args(["remote", "get-url", "origin"])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);

    if has_origin {
        // Check remote branches
        let remote = Command::new("git")
            .args(["ls-remote", "--exit-code", "--heads", "origin", name])
            .output()
            .context("Failed to check remote branch")?;

        if remote.status.success() {
            return Ok(true);
        }
    }

    Ok(false)
}

/// Find an available branch name, adding _2, _3, etc. if needed.
fn pick_available_name(candidate: &str) -> Result<String> {
    if !branch_exists(candidate)? {
        return Ok(candidate.to_string());
    }

    // Try with numeric suffix
    for i in 2..=100 {
        let with_suffix = format!("{}_{}", candidate, i);
        if !branch_exists(&with_suffix)? {
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
}
