//! Codex - Semantic code search for Daedalos
//!
//! Find code by meaning, not just keywords. Codex provides semantic search
//! capabilities over codebases, helping AI agents and developers find
//! relevant code patterns, implementations, and examples.
//!
//! ## Key Features
//!
//! - **Semantic chunking**: Code is split by functions, classes, and structs
//!   rather than fixed-size blocks
//! - **Keyword search**: SQLite FTS5 for fast full-text search
//! - **Project-aware**: Respects .gitignore and auto-detects project roots
//! - **Incremental indexing**: Only re-indexes changed files

pub mod chunker;
pub mod indexer;
pub mod searcher;

pub use chunker::{CodeChunk, CodeChunker};
pub use indexer::{CodeIndex, IndexStats, IndexStatistics};
pub use searcher::{CodeSearcher, SearchResult, format_results, format_results_json};

use std::path::{Path, PathBuf};

/// Project root markers
const PROJECT_MARKERS: &[&str] = &[
    ".git",
    "package.json",
    "Cargo.toml",
    "pyproject.toml",
    "go.mod",
    "Package.swift",
    "build.gradle",
    "pom.xml",
    "Makefile",
    "CMakeLists.txt",
];

/// Find the project root by looking for common markers
pub fn find_project_root(start_path: &Path) -> PathBuf {
    let start = start_path
        .canonicalize()
        .unwrap_or_else(|_| start_path.to_path_buf());

    let mut current = start.as_path();

    loop {
        for marker in PROJECT_MARKERS {
            if current.join(marker).exists() {
                return current.to_path_buf();
            }
        }

        match current.parent() {
            Some(parent) => current = parent,
            None => break,
        }
    }

    // No marker found, use the starting directory
    start
}
