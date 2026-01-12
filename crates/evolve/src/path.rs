//! Evolution path suggestion
//!
//! Chart a path from current state to full intent realization.

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::Path;

use crate::analyze::Analysis;
use crate::gaps::Gaps;
use crate::intent::Intent;

/// Suggested evolution path
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvolutionPath {
    /// Priority 1: Fix critical issues
    pub priority_fix: Vec<Suggestion>,

    /// Priority 2: Add missing fundamentals
    pub priority_fundamentals: Vec<Suggestion>,

    /// Priority 3: Extend toward full intent
    pub priority_extend: Vec<Suggestion>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Suggestion {
    pub action: String,
    pub reason: String,
    pub priority: u32, // 1-3
}

impl EvolutionPath {
    fn new() -> Self {
        Self {
            priority_fix: Vec::new(),
            priority_fundamentals: Vec::new(),
            priority_extend: Vec::new(),
        }
    }
}

/// Suggest evolution path based on intent, analysis, and gaps
pub fn suggest_path(
    _path: &Path,
    intent: &Intent,
    analysis: &Analysis,
    gaps: &Gaps,
) -> Result<EvolutionPath> {
    let mut path = EvolutionPath::new();

    // Priority 1: Fix before extend
    // Address TODO/FIXME items
    if !analysis.todos.is_empty() {
        let todo_files: Vec<_> = analysis
            .todos
            .iter()
            .map(|t| t.file.clone())
            .collect::<std::collections::HashSet<_>>()
            .into_iter()
            .take(3)
            .collect();

        path.priority_fix.push(Suggestion {
            action: format!("Address {} TODO/FIXME items", analysis.todos.len()),
            reason: format!("Found in: {}", todo_files.join(", ")),
            priority: 1,
        });
    }

    // Large files need splitting
    for large in &analysis.large_files {
        path.priority_fix.push(Suggestion {
            action: format!("Consider splitting large file ({} lines)", large.lines),
            reason: large.path.clone(),
            priority: 1,
        });
    }

    // Priority 2: Missing fundamentals
    if gaps.missing_spec {
        path.priority_fundamentals.push(Suggestion {
            action: "Create .spec.yaml to document intent".to_string(),
            reason: "Specs encode intent explicitly for future development".to_string(),
            priority: 2,
        });
    }

    if gaps.missing_tests {
        path.priority_fundamentals.push(Suggestion {
            action: "Add tests for core functionality".to_string(),
            reason: "Tests document expected behavior and catch regressions".to_string(),
            priority: 2,
        });
    }

    if gaps.missing_docs {
        path.priority_fundamentals.push(Suggestion {
            action: "Add README documentation".to_string(),
            reason: "Documentation helps others understand the component's purpose".to_string(),
            priority: 2,
        });
    }

    // Priority 3: Extend toward full intent
    // Verify spec examples are implemented
    if intent.has_spec {
        path.priority_extend.push(Suggestion {
            action: "Verify all spec examples are implemented".to_string(),
            reason: "Spec examples define expected behavior".to_string(),
            priority: 3,
        });

        if !intent.success_criteria.is_empty() {
            path.priority_extend.push(Suggestion {
                action: "Verify spec success_criteria are met".to_string(),
                reason: format!("Criteria: {}", intent.success_criteria.first().unwrap_or(&"".to_string())),
                priority: 3,
            });
        }
    }

    // Integration points from spec
    if !gaps.integration_points.is_empty() {
        path.priority_extend.push(Suggestion {
            action: format!("Verify integration with: {}", gaps.integration_points.join(", ")),
            reason: "Integration points from spec connects_to".to_string(),
            priority: 3,
        });
    }

    Ok(path)
}
