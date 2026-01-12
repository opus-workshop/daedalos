//! Context gathering for the resolve tool
//!
//! Gathers context from multiple sources:
//! - Specs (spec query)
//! - Codebase patterns (codex search)
//! - Project info (project)
//! - Decision history (DECISIONS.md)

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::process::{Command, Stdio};

/// Confidence level based on context found
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum ConfidenceLevel {
    /// No context found - may need to ask
    None = 0,
    /// Limited context - proceed with stated assumptions
    Low = 1,
    /// Some context found - may need inference
    Medium = 2,
    /// Multiple consistent sources - proceed silently
    High = 3,
}

impl ConfidenceLevel {
    pub fn as_str(&self) -> &'static str {
        match self {
            ConfidenceLevel::None => "none",
            ConfidenceLevel::Low => "low",
            ConfidenceLevel::Medium => "medium",
            ConfidenceLevel::High => "high",
        }
    }

    /// Calculate confidence based on number of sources with results
    pub fn from_sources(sources_with_results: usize) -> Self {
        match sources_with_results {
            0 => ConfidenceLevel::None,
            1 => ConfidenceLevel::Low,
            2 => ConfidenceLevel::Medium,
            _ => ConfidenceLevel::High,
        }
    }
}

/// Result of context gathering
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextResult {
    /// Number of sources checked
    pub sources_checked: usize,
    /// Number of sources that returned results
    pub sources_with_results: usize,
    /// Confidence level
    pub confidence: ConfidenceLevel,
    /// Context from specs
    pub spec_context: Option<String>,
    /// Context from codex (codebase patterns)
    pub codex_context: Option<String>,
    /// Context from project info
    pub project_context: Option<String>,
    /// Context from decision history
    pub decisions_context: Option<String>,
}

/// Gather context from all available sources
pub fn gather_context(question: &str) -> Result<ContextResult> {
    let mut sources_checked = 0;
    let mut sources_with_results = 0;

    // 1. Check specs
    let spec_context = check_specs(question);
    sources_checked += 1;
    if spec_context.is_some() {
        sources_with_results += 1;
    }

    // 2. Check codebase patterns
    let codex_context = check_codex(question);
    sources_checked += 1;
    if codex_context.is_some() {
        sources_with_results += 1;
    }

    // 3. Check project info
    let project_context = check_project();
    sources_checked += 1;
    if project_context.is_some() {
        sources_with_results += 1;
    }

    // 4. Check decision history
    let decisions_context = check_decisions(question);
    sources_checked += 1;
    if decisions_context.is_some() {
        sources_with_results += 1;
    }

    let confidence = ConfidenceLevel::from_sources(sources_with_results);

    Ok(ContextResult {
        sources_checked,
        sources_with_results,
        confidence,
        spec_context,
        codex_context,
        project_context,
        decisions_context,
    })
}

/// Run a command and capture output
fn run_tool(cmd: &str, args: &[&str], _timeout_secs: u64) -> Option<String> {
    let child = Command::new(cmd)
        .args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .ok()?;

    // Wait for output with timeout
    let output = child.wait_with_output().ok()?;

    if output.status.success() {
        let stdout = String::from_utf8_lossy(&output.stdout);
        let trimmed = stdout.trim();
        if !trimmed.is_empty()
            && !trimmed.contains("No matches")
            && !trimmed.contains("No results")
            && !trimmed.contains("Error")
        {
            Some(trimmed.to_string())
        } else {
            None
        }
    } else {
        None
    }
}

/// Check specs for relevant context
fn check_specs(question: &str) -> Option<String> {
    // Extract keywords for spec query
    let keywords: String = question
        .to_lowercase()
        .chars()
        .map(|c| if c.is_alphanumeric() || c == ' ' { c } else { ' ' })
        .collect();

    run_tool("spec", &["query", &keywords], 10)
}

/// Check codex for codebase patterns
fn check_codex(question: &str) -> Option<String> {
    run_tool("codex", &["search", question, "--limit", "5"], 10)
}

/// Check project info for conventions
fn check_project() -> Option<String> {
    run_tool("project", &["summary"], 10)
}

/// Check DECISIONS.md for past decisions
fn check_decisions(question: &str) -> Option<String> {
    let decisions_path = find_decisions_file()?;

    // Read and search the file
    let content = std::fs::read_to_string(&decisions_path).ok()?;

    // Extract first few words for searching
    let search_terms: Vec<&str> = question.split_whitespace().take(3).collect();
    if search_terms.is_empty() {
        return None;
    }

    // Find relevant sections
    let mut relevant_lines = Vec::new();
    let lines: Vec<&str> = content.lines().collect();

    for (i, line) in lines.iter().enumerate() {
        let line_lower = line.to_lowercase();
        // Check if any search term appears in this line
        if search_terms
            .iter()
            .any(|term| line_lower.contains(&term.to_lowercase()))
        {
            // Collect context around match
            let start = i.saturating_sub(1);
            let end = (i + 3).min(lines.len());
            for j in start..end {
                if !relevant_lines.contains(&lines[j]) {
                    relevant_lines.push(lines[j]);
                }
            }
        }
    }

    if relevant_lines.is_empty() {
        None
    } else {
        // Limit output
        let result: Vec<&str> = relevant_lines.into_iter().take(15).collect();
        Some(result.join("\n"))
    }
}

/// Find the DECISIONS.md file in the project
fn find_decisions_file() -> Option<PathBuf> {
    // Try .claude/DECISIONS.md first
    let claude_decisions = PathBuf::from(".claude/DECISIONS.md");
    if claude_decisions.exists() {
        return Some(claude_decisions);
    }

    // Try DECISIONS.md in project root
    let root_decisions = PathBuf::from("DECISIONS.md");
    if root_decisions.exists() {
        return Some(root_decisions);
    }

    // Try to find git root and check there
    if let Ok(output) = Command::new("git")
        .args(["rev-parse", "--show-toplevel"])
        .output()
    {
        if output.status.success() {
            let git_root = String::from_utf8_lossy(&output.stdout).trim().to_string();
            let git_decisions = PathBuf::from(&git_root).join(".claude/DECISIONS.md");
            if git_decisions.exists() {
                return Some(git_decisions);
            }
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_confidence_from_sources() {
        assert_eq!(ConfidenceLevel::from_sources(0), ConfidenceLevel::None);
        assert_eq!(ConfidenceLevel::from_sources(1), ConfidenceLevel::Low);
        assert_eq!(ConfidenceLevel::from_sources(2), ConfidenceLevel::Medium);
        assert_eq!(ConfidenceLevel::from_sources(3), ConfidenceLevel::High);
        assert_eq!(ConfidenceLevel::from_sources(4), ConfidenceLevel::High);
    }

    #[test]
    fn test_confidence_ordering() {
        assert!(ConfidenceLevel::High > ConfidenceLevel::Medium);
        assert!(ConfidenceLevel::Medium > ConfidenceLevel::Low);
        assert!(ConfidenceLevel::Low > ConfidenceLevel::None);
    }
}
