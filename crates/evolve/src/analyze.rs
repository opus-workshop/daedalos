//! Current state analysis
//!
//! Assess how well current code serves the intent.

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::Path;
use walkdir::WalkDir;

/// Analysis of current code state
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Analysis {
    /// Count of issues found
    pub issues: u32,

    /// Count of warnings found
    pub warnings: u32,

    /// TODO/FIXME items found
    pub todos: Vec<TodoItem>,

    /// Large files that might need splitting
    pub large_files: Vec<LargeFile>,

    /// Similar patterns found in codebase
    pub similar_patterns: Vec<String>,

    /// Files in scope
    pub file_count: u32,

    /// Total lines of code
    pub line_count: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TodoItem {
    pub file: String,
    pub line: u32,
    pub content: String,
    pub kind: String, // TODO, FIXME, XXX, HACK
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LargeFile {
    pub path: String,
    pub lines: u32,
}

impl Analysis {
    fn new() -> Self {
        Self {
            issues: 0,
            warnings: 0,
            todos: Vec::new(),
            large_files: Vec::new(),
            similar_patterns: Vec::new(),
            file_count: 0,
            line_count: 0,
        }
    }
}

/// Analyze the current state of code
pub fn analyze_state(path: &Path) -> Result<Analysis> {
    let mut analysis = Analysis::new();

    if path.is_file() {
        analyze_file(path, &mut analysis)?;
    } else {
        analyze_directory(path, &mut analysis)?;
    }

    Ok(analysis)
}

fn analyze_file(path: &Path, analysis: &mut Analysis) -> Result<()> {
    let content = std::fs::read_to_string(path)?;
    let lines: Vec<&str> = content.lines().collect();
    let line_count = lines.len() as u32;

    analysis.file_count += 1;
    analysis.line_count += line_count;

    // Check for TODO/FIXME
    for (i, line) in lines.iter().enumerate() {
        let trimmed = line.trim();
        for kind in &["TODO", "FIXME", "XXX", "HACK"] {
            if trimmed.contains(kind) {
                analysis.todos.push(TodoItem {
                    file: path.to_string_lossy().to_string(),
                    line: (i + 1) as u32,
                    content: trimmed.to_string(),
                    kind: kind.to_string(),
                });
                analysis.warnings += 1;
                break;
            }
        }
    }

    // Check for large file
    if line_count > 500 {
        analysis.large_files.push(LargeFile {
            path: path.to_string_lossy().to_string(),
            lines: line_count,
        });
        analysis.warnings += 1;
    }

    Ok(())
}

fn analyze_directory(path: &Path, analysis: &mut Analysis) -> Result<()> {
    let code_extensions = ["ts", "tsx", "js", "jsx", "py", "go", "rs", "rb", "java", "c", "cpp", "h"];

    for entry in WalkDir::new(path)
        .max_depth(5)
        .into_iter()
        .filter_entry(|e| {
            // Skip hidden and common non-source dirs
            let name = e.file_name().to_string_lossy();
            !name.starts_with('.')
                && name != "node_modules"
                && name != "target"
                && name != "dist"
                && name != "build"
                && name != "__pycache__"
                && name != "venv"
                && name != ".venv"
        })
        .flatten()
    {
        if entry.file_type().is_file() {
            let ext = entry.path().extension().and_then(|e| e.to_str()).unwrap_or("");
            if code_extensions.contains(&ext) {
                analyze_file(entry.path(), analysis)?;
            }
        }
    }

    Ok(())
}
