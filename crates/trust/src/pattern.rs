//! Pattern learning and storage
//!
//! The trust system learns from user decisions. When the same pattern is approved
//! multiple times, the system suggests auto-allowing it in the future.
//!
//! Storage: ~/.config/daedalos/trust-patterns.yaml

use anyhow::{Context, Result};
use chrono::{DateTime, Duration, Utc};
use daedalos_core::Paths;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;

/// A learned permission pattern
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Pattern {
    /// The pattern string (e.g., "rm *.pyc")
    pub pattern: String,

    /// Scope where this pattern applies (e.g., "~/projects/*")
    pub scope: String,

    /// The learned decision
    pub decision: PatternDecision,

    /// Number of times this pattern was encountered
    pub count: u32,

    /// When this pattern was first seen
    pub first_seen: DateTime<Utc>,

    /// When this pattern was last used
    pub last_used: DateTime<Utc>,

    /// Whether user confirmed this as permanent
    pub confirmed: bool,
}

/// Decision for a pattern
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PatternDecision {
    /// Always allow this pattern
    Allow,
    /// Always deny this pattern
    Deny,
    /// Always ask (never suggest auto-allow)
    Ask,
}

impl Pattern {
    /// Create a new pattern
    pub fn new(pattern: String, scope: String, decision: PatternDecision) -> Self {
        let now = Utc::now();
        Self {
            pattern,
            scope,
            decision,
            count: 1,
            first_seen: now,
            last_used: now,
            confirmed: false,
        }
    }

    /// Increment usage count
    pub fn record_use(&mut self) {
        self.count += 1;
        self.last_used = Utc::now();
    }

    /// Check if pattern is stale (unused for too long)
    pub fn is_stale(&self, days: i64) -> bool {
        let threshold = Utc::now() - Duration::days(days);
        self.last_used < threshold
    }

    /// Check if pattern should be suggested for auto-allow
    pub fn should_suggest_allow(&self, threshold: u32) -> bool {
        !self.confirmed && self.count >= threshold && self.decision == PatternDecision::Allow
    }
}

/// Storage for learned patterns
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PatternStore {
    /// All learned patterns, keyed by "pattern@scope"
    patterns: HashMap<String, Pattern>,
}

impl PatternStore {
    /// Load patterns from default location
    pub fn load() -> Result<Self> {
        let paths = Paths::new();
        let store_path = paths.config.join("trust-patterns.yaml");
        Self::load_from(&store_path)
    }

    /// Load patterns from a specific path
    pub fn load_from(path: &Path) -> Result<Self> {
        if path.exists() {
            let content = std::fs::read_to_string(path)
                .with_context(|| format!("Failed to read patterns from {:?}", path))?;
            let store: Self = serde_yaml::from_str(&content)
                .with_context(|| format!("Failed to parse patterns from {:?}", path))?;
            Ok(store)
        } else {
            Ok(Self::default())
        }
    }

    /// Save patterns to default location
    pub fn save(&self) -> Result<()> {
        let paths = Paths::new();
        let store_path = paths.config.join("trust-patterns.yaml");
        self.save_to(&store_path)
    }

    /// Save patterns to a specific path
    pub fn save_to(&self, path: &Path) -> Result<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let content = serde_yaml::to_string(self)?;
        std::fs::write(path, content)?;
        Ok(())
    }

    /// Generate key for a pattern
    fn key(pattern: &str, scope: &str) -> String {
        format!("{}@{}", pattern, scope)
    }

    /// Look up a pattern
    pub fn get(&self, pattern: &str, scope: &str) -> Option<&Pattern> {
        // First try exact match
        if let Some(p) = self.patterns.get(&Self::key(pattern, scope)) {
            return Some(p);
        }

        // Try wildcard scope
        self.patterns.get(&Self::key(pattern, "*"))
    }

    /// Look up a pattern (mutable)
    pub fn get_mut(&mut self, pattern: &str, scope: &str) -> Option<&mut Pattern> {
        let key = Self::key(pattern, scope);
        if self.patterns.contains_key(&key) {
            return self.patterns.get_mut(&key);
        }

        // Try wildcard scope
        let wildcard_key = Self::key(pattern, "*");
        self.patterns.get_mut(&wildcard_key)
    }

    /// Add or update a pattern
    pub fn record(&mut self, pattern: &str, scope: &str, decision: PatternDecision) {
        let key = Self::key(pattern, scope);

        if let Some(existing) = self.patterns.get_mut(&key) {
            existing.record_use();
            // Update decision if changed
            if existing.decision != decision && !existing.confirmed {
                existing.decision = decision;
            }
        } else {
            let p = Pattern::new(pattern.to_string(), scope.to_string(), decision);
            self.patterns.insert(key, p);
        }
    }

    /// Confirm a pattern (make it permanent)
    pub fn confirm(&mut self, pattern: &str, scope: &str) -> bool {
        if let Some(p) = self.get_mut(pattern, scope) {
            p.confirmed = true;
            true
        } else {
            false
        }
    }

    /// Remove a pattern
    pub fn remove(&mut self, pattern: &str, scope: &str) -> bool {
        let key = Self::key(pattern, scope);
        self.patterns.remove(&key).is_some()
    }

    /// List all patterns
    pub fn list(&self) -> impl Iterator<Item = &Pattern> {
        self.patterns.values()
    }

    /// List patterns matching a scope
    pub fn list_for_scope(&self, scope: &str) -> Vec<&Pattern> {
        self.patterns
            .values()
            .filter(|p| p.scope == scope || p.scope == "*" || scope_matches(&p.scope, scope))
            .collect()
    }

    /// Remove stale patterns
    pub fn cleanup(&mut self, stale_days: i64) -> usize {
        let before = self.patterns.len();
        self.patterns.retain(|_, p| !p.is_stale(stale_days) || p.confirmed);
        before - self.patterns.len()
    }

    /// Get patterns that should be suggested for auto-allow
    pub fn suggestions(&self, threshold: u32) -> Vec<&Pattern> {
        self.patterns
            .values()
            .filter(|p| p.should_suggest_allow(threshold))
            .collect()
    }
}

/// Check if a scope pattern matches a path
fn scope_matches(scope_pattern: &str, path: &str) -> bool {
    if scope_pattern == "*" {
        return true;
    }

    let expanded = shellexpand::tilde(scope_pattern).to_string();

    if expanded.ends_with("/*") {
        let prefix = &expanded[..expanded.len() - 2];
        path.starts_with(prefix)
    } else if expanded.ends_with("*") {
        let prefix = &expanded[..expanded.len() - 1];
        path.starts_with(prefix)
    } else {
        path == expanded || path.starts_with(&format!("{}/", expanded))
    }
}

/// Normalize a command pattern for matching
///
/// Converts specific commands to generalizable patterns:
/// - "rm src/__pycache__/foo.pyc" -> "rm *.pyc"
/// - "rm -f build/out.o" -> "rm *.o"
pub fn normalize_pattern(tool: &str, args: &[&str]) -> String {
    let mut parts = vec![tool.to_string()];

    for arg in args {
        if arg.starts_with('-') {
            // Keep flags as-is
            parts.push(arg.to_string());
        } else {
            // Generalize file paths to extension patterns
            let normalized = normalize_file_arg(arg);
            parts.push(normalized);
        }
    }

    parts.join(" ")
}

/// Normalize a file argument to a pattern
fn normalize_file_arg(arg: &str) -> String {
    // Extract extension
    if let Some(ext_pos) = arg.rfind('.') {
        let ext = &arg[ext_pos..];
        // Common generated file extensions
        if matches!(
            ext,
            ".pyc" | ".pyo" | ".o" | ".a" | ".so" | ".dylib"
                | ".class" | ".jar"
                | ".log" | ".tmp" | ".bak"
                | ".d" | ".dSYM"
        ) {
            return format!("*{}", ext);
        }
    }

    // Check for common generated directories
    if arg.contains("__pycache__")
        || arg.contains("node_modules")
        || arg.contains(".git")
        || arg.contains("target/")
        || arg.contains("build/")
        || arg.contains("dist/")
    {
        // Keep directory part but generalize file
        if let Some(last_slash) = arg.rfind('/') {
            let dir = &arg[..=last_slash];
            let file = &arg[last_slash + 1..];
            if let Some(ext_pos) = file.rfind('.') {
                let ext = &file[ext_pos..];
                return format!("{}*{}", dir, ext);
            }
        }
    }

    // Return as-is if no normalization applies
    arg.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pattern_store() {
        let mut store = PatternStore::default();

        // Record a pattern
        store.record("rm *.pyc", "~/projects/*", PatternDecision::Allow);
        assert!(store.get("rm *.pyc", "~/projects/foo").is_some());

        // Record again
        store.record("rm *.pyc", "~/projects/*", PatternDecision::Allow);
        let p = store.get("rm *.pyc", "~/projects/*").unwrap();
        assert_eq!(p.count, 2);
    }

    #[test]
    fn test_normalize_pattern() {
        assert_eq!(
            normalize_pattern("rm", &["src/__pycache__/foo.pyc"]),
            "rm src/__pycache__/*.pyc"
        );
        assert_eq!(
            normalize_pattern("rm", &["-f", "build/out.o"]),
            "rm -f build/*.o"
        );
        assert_eq!(
            normalize_pattern("git", &["add", "."]),
            "git add ."
        );
    }

    #[test]
    fn test_scope_matching() {
        assert!(scope_matches("*", "/any/path"));
        assert!(scope_matches("~/projects/*", "/home/user/projects/foo"));
    }
}
