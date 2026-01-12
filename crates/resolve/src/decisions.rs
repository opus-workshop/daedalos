//! Decision logging for the resolve tool
//!
//! Logs decisions to .claude/DECISIONS.md for future reference.
//! Builds institutional memory in the project.

use anyhow::{Context, Result};
use chrono::Local;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::PathBuf;
use std::process::Command;

/// Get the path to DECISIONS.md
fn get_decisions_path() -> PathBuf {
    // Try to find git root
    if let Ok(output) = Command::new("git")
        .args(["rev-parse", "--show-toplevel"])
        .output()
    {
        if output.status.success() {
            let git_root = String::from_utf8_lossy(&output.stdout).trim().to_string();
            return PathBuf::from(&git_root).join(".claude/DECISIONS.md");
        }
    }

    // Fall back to current directory
    PathBuf::from(".claude/DECISIONS.md")
}

/// Create the DECISIONS.md file if it doesn't exist
fn ensure_decisions_file(path: &PathBuf) -> Result<()> {
    if path.exists() {
        return Ok(());
    }

    // Create parent directory
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).context("Failed to create .claude directory")?;
    }

    // Create file with header
    let header = r#"# Decisions

This file tracks implementation decisions for future reference.
Created and maintained by the `resolve` tool.

---

"#;

    fs::write(path, header).context("Failed to create DECISIONS.md")?;
    Ok(())
}

/// Log a decision to DECISIONS.md
pub fn log_decision(decision: &str, reasoning: Option<&str>) -> Result<PathBuf> {
    let path = get_decisions_path();
    ensure_decisions_file(&path)?;

    let now = Local::now();
    let date = now.format("%Y-%m-%d").to_string();
    let time = now.format("%H:%M").to_string();

    let reasoning_text = reasoning.unwrap_or("Not specified");

    let entry = format!(
        r#"
## {} {}

**Decision:** {}

**Reasoning:** {}

**Resolved by:** resolve tool

---
"#,
        date, time, decision, reasoning_text
    );

    let mut file = OpenOptions::new()
        .append(true)
        .open(&path)
        .context("Failed to open DECISIONS.md")?;

    file.write_all(entry.as_bytes())
        .context("Failed to write to DECISIONS.md")?;

    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;
    use tempfile::tempdir;

    #[test]
    fn test_log_decision() {
        let temp = tempdir().unwrap();
        env::set_current_dir(&temp).unwrap();

        // Create .claude directory
        fs::create_dir_all(".claude").unwrap();

        // Log a decision
        let path = log_decision("Use JWT for authentication", Some("API-first architecture"))
            .unwrap();

        assert!(path.exists());

        let content = fs::read_to_string(&path).unwrap();
        assert!(content.contains("Use JWT for authentication"));
        assert!(content.contains("API-first architecture"));
        assert!(content.contains("**Resolved by:** resolve tool"));
    }
}
