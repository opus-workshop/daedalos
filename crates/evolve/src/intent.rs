//! Intent extraction from various sources
//!
//! Gathers intent from specs, commits, docs, tests, and code comments.

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::process::Command;

/// Intent extracted from a code path
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Intent {
    /// Spec file if found
    pub spec_file: Option<PathBuf>,

    /// Whether a spec was found (primary source)
    pub has_spec: bool,

    /// Intent statement from spec
    pub spec_intent: Option<String>,

    /// Success criteria from spec
    pub success_criteria: Vec<String>,

    /// First commit that introduced this path
    pub first_commit: Option<String>,

    /// Recent commits modifying this path
    pub recent_commits: Vec<String>,

    /// Content from README/docs
    pub doc_content: Option<String>,

    /// Test descriptions found
    pub test_descriptions: Vec<String>,

    /// Top-level code comments
    pub code_comments: Option<String>,

    /// Number of intent sources found
    pub sources_count: u32,
}

impl Intent {
    fn new() -> Self {
        Self {
            spec_file: None,
            has_spec: false,
            spec_intent: None,
            success_criteria: Vec::new(),
            first_commit: None,
            recent_commits: Vec::new(),
            doc_content: None,
            test_descriptions: Vec::new(),
            code_comments: None,
            sources_count: 0,
        }
    }
}

/// Extract intent from all available sources
pub fn extract_intent(path: &Path) -> Result<Intent> {
    let mut intent = Intent::new();

    // 1. Check for spec (primary source)
    if let Some(spec_file) = find_spec(path) {
        intent.spec_file = Some(spec_file.clone());
        intent.has_spec = true;
        intent.sources_count += 1;

        if let Ok((spec_intent, criteria)) = extract_spec_content(&spec_file) {
            intent.spec_intent = spec_intent;
            intent.success_criteria = criteria;
        }
    }

    // 2. Git history
    if let Ok((first, recent)) = extract_git_info(path) {
        if first.is_some() || !recent.is_empty() {
            intent.sources_count += 1;
        }
        intent.first_commit = first;
        intent.recent_commits = recent;
    }

    // 3. Documentation
    if let Some(doc) = extract_doc_content(path) {
        intent.doc_content = Some(doc);
        intent.sources_count += 1;
    }

    // 4. Tests
    let tests = extract_test_descriptions(path);
    if !tests.is_empty() {
        intent.test_descriptions = tests;
        intent.sources_count += 1;
    }

    // 5. Code comments
    if let Some(comments) = extract_code_comments(path) {
        intent.code_comments = Some(comments);
        intent.sources_count += 1;
    }

    Ok(intent)
}

/// Find spec file for a path
fn find_spec(path: &Path) -> Option<PathBuf> {
    let dir = if path.is_file() {
        path.parent()?
    } else {
        path
    };

    // Check various spec locations
    let patterns = [
        dir.join("*.spec.yaml"),
        dir.parent().map(|p| p.join("*.spec.yaml")).unwrap_or_default(),
        dir.join("spec.yaml"),
        dir.parent().map(|p| p.join("spec.yaml")).unwrap_or_default(),
    ];

    for pattern in &patterns {
        if let Ok(entries) = glob::glob(pattern.to_string_lossy().as_ref()) {
            for entry in entries.flatten() {
                if entry.is_file() {
                    return Some(entry);
                }
            }
        }
    }

    None
}

/// Extract intent and success criteria from spec file
fn extract_spec_content(spec_file: &Path) -> Result<(Option<String>, Vec<String>)> {
    let content = std::fs::read_to_string(spec_file)?;
    let yaml: serde_yaml::Value = serde_yaml::from_str(&content)?;

    // Extract intent section
    let spec_intent = yaml
        .get("intent")
        .and_then(|v| v.as_str())
        .map(|s| {
            // Take first paragraph or first 5 lines
            s.lines()
                .take(5)
                .collect::<Vec<_>>()
                .join("\n")
                .trim()
                .to_string()
        });

    // Extract success criteria
    let success_criteria = yaml
        .get("metrics")
        .and_then(|m| m.get("success_criteria"))
        .and_then(|c| c.as_sequence())
        .map(|seq| {
            seq.iter()
                .filter_map(|v| v.as_str())
                .map(|s| s.to_string())
                .take(5)
                .collect()
        })
        .unwrap_or_default();

    Ok((spec_intent, success_criteria))
}

/// Extract git commit info
fn extract_git_info(path: &Path) -> Result<(Option<String>, Vec<String>)> {
    let path_str = path.to_string_lossy();

    // Get first commit
    let first_output = Command::new("git")
        .args(["log", "--diff-filter=A", "--format=%h %s", "--", &path_str])
        .output();

    let first_commit = first_output
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .and_then(|s| s.lines().last().map(|l| l.to_string()));

    // Get recent commits
    let recent_output = Command::new("git")
        .args(["log", "--oneline", "-5", "--", &path_str])
        .output();

    let recent_commits = recent_output
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| {
            s.lines()
                .take(3)
                .map(|l| l.to_string())
                .collect()
        })
        .unwrap_or_default();

    Ok((first_commit, recent_commits))
}

/// Extract content from README/docs
fn extract_doc_content(path: &Path) -> Option<String> {
    let dir = if path.is_file() {
        path.parent()?
    } else {
        path
    };

    let readme_patterns = [
        dir.join("README.md"),
        dir.join("README.txt"),
        dir.join("README"),
        dir.parent().map(|p| p.join("README.md")).unwrap_or_default(),
    ];

    for readme in &readme_patterns {
        if readme.is_file() {
            if let Ok(content) = std::fs::read_to_string(readme) {
                // Take first meaningful lines (skip title/headers)
                let lines: Vec<_> = content
                    .lines()
                    .filter(|l| !l.starts_with('#') && !l.is_empty())
                    .take(5)
                    .collect();
                if !lines.is_empty() {
                    return Some(lines.join("\n"));
                }
            }
        }
    }

    None
}

/// Extract test descriptions from test files
fn extract_test_descriptions(path: &Path) -> Vec<String> {
    let dir = if path.is_file() {
        path.parent().unwrap_or(path)
    } else {
        path
    };

    let mut descriptions = Vec::new();

    // Look for test files
    let test_files: Vec<_> = walkdir::WalkDir::new(dir)
        .max_depth(2)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| {
            let name = e.file_name().to_string_lossy();
            e.file_type().is_file() && (name.contains("test") || name.contains("spec"))
        })
        .take(3)
        .collect();

    if let Some(entry) = test_files.first() {
        if let Ok(content) = std::fs::read_to_string(entry.path()) {
            // Look for test/it/describe blocks
            for line in content.lines() {
                let trimmed = line.trim();
                if trimmed.starts_with("it(")
                    || trimmed.starts_with("test(")
                    || trimmed.starts_with("describe(")
                    || trimmed.contains("#[test]")
                    || trimmed.starts_with("def test_")
                    || trimmed.starts_with("func Test")
                {
                    descriptions.push(trimmed.to_string());
                    if descriptions.len() >= 5 {
                        break;
                    }
                }
            }
        }
    }

    descriptions
}

/// Extract top-level code comments
fn extract_code_comments(path: &Path) -> Option<String> {
    let file_path = if path.is_file() {
        path.to_path_buf()
    } else {
        // For directories, find main file
        let extensions = ["ts", "py", "js", "go", "rs", "tsx", "jsx"];
        let mut main_file = None;

        for entry in walkdir::WalkDir::new(path)
            .max_depth(2)
            .into_iter()
            .filter_map(|e| e.ok())
        {
            if entry.file_type().is_file() {
                let name = entry.file_name().to_string_lossy();
                for ext in &extensions {
                    if name.ends_with(&format!(".{}", ext)) {
                        main_file = Some(entry.path().to_path_buf());
                        break;
                    }
                }
                if main_file.is_some() {
                    break;
                }
            }
        }

        main_file?
    };

    let content = std::fs::read_to_string(&file_path).ok()?;

    // Look for top-level comments
    let mut comments = Vec::new();
    for line in content.lines().take(20) {
        let trimmed = line.trim();
        if trimmed.starts_with("//")
            || trimmed.starts_with('#')
            || trimmed.starts_with('*')
            || trimmed.starts_with("/*")
            || trimmed.starts_with("\"\"\"")
            || trimmed.starts_with("'''")
        {
            comments.push(trimmed.to_string());
            if comments.len() >= 5 {
                break;
            }
        }
    }

    if comments.is_empty() {
        None
    } else {
        Some(comments.join("\n"))
    }
}
