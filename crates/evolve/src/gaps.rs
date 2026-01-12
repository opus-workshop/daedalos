//! Gap identification
//!
//! Find what's missing to fully realize the intent.

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::Path;
use walkdir::WalkDir;

use crate::intent::Intent;

/// Gaps identified in the codebase
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Gaps {
    /// Total gap count
    pub count: u32,

    /// Missing fundamental pieces
    pub missing_fundamentals: Vec<MissingItem>,

    /// Integration points from spec that need verification
    pub integration_points: Vec<String>,

    /// Interface elements from spec to verify
    pub spec_interface: Vec<String>,

    /// Missing tests
    pub missing_tests: bool,

    /// Missing documentation
    pub missing_docs: bool,

    /// Missing spec
    pub missing_spec: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MissingItem {
    pub category: String,
    pub description: String,
    pub priority: Priority,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Priority {
    High,
    Medium,
    Low,
}

impl Gaps {
    fn new() -> Self {
        Self {
            count: 0,
            missing_fundamentals: Vec::new(),
            integration_points: Vec::new(),
            spec_interface: Vec::new(),
            missing_tests: false,
            missing_docs: false,
            missing_spec: false,
        }
    }

    fn add_missing(&mut self, category: &str, description: &str, priority: Priority) {
        self.missing_fundamentals.push(MissingItem {
            category: category.to_string(),
            description: description.to_string(),
            priority,
        });
        self.count += 1;
    }
}

/// Identify gaps based on intent and current state
pub fn identify_gaps(path: &Path, intent: &Intent) -> Result<Gaps> {
    let mut gaps = Gaps::new();

    // Check for spec
    if !intent.has_spec {
        gaps.missing_spec = true;
        gaps.add_missing(
            "documentation",
            "No .spec.yaml file - consider documenting intent",
            Priority::Medium,
        );
    } else if let Some(ref spec_file) = intent.spec_file {
        // Extract integration points and interface from spec
        extract_spec_gaps(spec_file, &mut gaps)?;
    }

    // Check for documentation
    if intent.doc_content.is_none() {
        gaps.missing_docs = true;
        gaps.add_missing(
            "documentation",
            "No README documentation",
            Priority::Low,
        );
    }

    // Check for tests
    if !has_tests(path) {
        gaps.missing_tests = true;
        gaps.add_missing(
            "testing",
            "No test files found",
            Priority::High,
        );
    }

    // If spec has success criteria, add as potential gaps to verify
    if !intent.success_criteria.is_empty() {
        gaps.add_missing(
            "verification",
            "Verify spec success criteria are met",
            Priority::Medium,
        );
    }

    Ok(gaps)
}

fn extract_spec_gaps(spec_file: &Path, gaps: &mut Gaps) -> Result<()> {
    let content = std::fs::read_to_string(spec_file)?;
    let yaml: serde_yaml::Value = serde_yaml::from_str(&content)?;

    // Extract interface commands to verify
    if let Some(interface) = yaml.get("interface") {
        if let Some(commands) = interface.get("commands") {
            if let Some(map) = commands.as_mapping() {
                for (key, _) in map {
                    if let Some(cmd) = key.as_str() {
                        gaps.spec_interface.push(cmd.to_string());
                    }
                }
            }
        }
    }

    // Extract connects_to for integration points
    if let Some(connects) = yaml.get("connects_to") {
        if let Some(seq) = connects.as_sequence() {
            for item in seq {
                if let Some(component) = item.get("component").and_then(|c| c.as_str()) {
                    gaps.integration_points.push(component.to_string());
                    gaps.count += 1;
                }
            }
        }
    }

    // Check anti_patterns section exists (good practice)
    if yaml.get("anti_patterns").is_none() {
        gaps.add_missing(
            "spec",
            "Spec missing anti_patterns section",
            Priority::Low,
        );
    }

    Ok(())
}

fn has_tests(path: &Path) -> bool {
    let dir = if path.is_file() {
        path.parent().unwrap_or(path)
    } else {
        path
    };

    for entry in WalkDir::new(dir)
        .max_depth(3)
        .into_iter()
        .flatten()
    {
        let name = entry.file_name().to_string_lossy().to_lowercase();
        if name.contains("test") || name.contains("spec") {
            return true;
        }
    }

    false
}
