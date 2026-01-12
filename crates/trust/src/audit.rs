//! Audit logging for trust decisions
//!
//! Every trust decision is logged for accountability.
//! Storage: ~/.local/share/daedalos/trust/audit.log (JSON lines)

use crate::evaluator::{Action, Decision, Reason};
use crate::level::TrustLevel;
use anyhow::{Context, Result};
use chrono::{DateTime, Duration, Utc};
use daedalos_core::Paths;
use serde::{Deserialize, Serialize};
use std::fs::{File, OpenOptions};
use std::io::{BufRead, BufReader, BufWriter, Write};
use std::path::{Path, PathBuf};

/// A single audit log entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditEntry {
    /// When the decision was made
    pub timestamp: DateTime<Utc>,

    /// Session ID
    pub session_id: String,

    /// The operation
    pub operation: String,

    /// Operation arguments
    pub args: Vec<String>,

    /// Working directory
    pub working_dir: String,

    /// Trust level at time of decision
    pub trust_level: TrustLevel,

    /// The decision made
    pub decision: Action,

    /// Reason for the decision
    pub reason: Reason,

    /// Additional details
    pub details: String,

    /// Pattern that matched (if any)
    pub matched_pattern: Option<String>,

    /// Whether user was prompted
    pub user_prompted: bool,
}

impl AuditEntry {
    /// Create from a decision
    pub fn from_decision(
        decision: &Decision,
        session_id: &str,
        operation: &str,
        args: &[&str],
        working_dir: &Path,
        trust_level: TrustLevel,
        user_prompted: bool,
    ) -> Self {
        Self {
            timestamp: Utc::now(),
            session_id: session_id.to_string(),
            operation: operation.to_string(),
            args: args.iter().map(|s| s.to_string()).collect(),
            working_dir: working_dir.to_string_lossy().to_string(),
            trust_level,
            decision: decision.action,
            reason: decision.reason,
            details: decision.details.clone(),
            matched_pattern: decision.matched_pattern.clone(),
            user_prompted,
        }
    }
}

/// Audit log manager
pub struct AuditLog {
    log_path: PathBuf,
}

impl AuditLog {
    /// Create a new audit log at the default location
    pub fn new() -> Self {
        let paths = Paths::new();
        let log_path = paths.data.join("trust").join("audit.log");
        Self { log_path }
    }

    /// Create with custom path
    pub fn with_path(log_path: PathBuf) -> Self {
        Self { log_path }
    }

    /// Append an entry to the log
    pub fn log(&self, entry: &AuditEntry) -> Result<()> {
        // Ensure directory exists
        if let Some(parent) = self.log_path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        // Append to file
        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.log_path)
            .with_context(|| format!("Failed to open audit log at {:?}", self.log_path))?;

        let mut writer = BufWriter::new(file);
        let json = serde_json::to_string(entry)?;
        writeln!(writer, "{}", json)?;
        writer.flush()?;

        Ok(())
    }

    /// Query recent entries
    pub fn recent(&self, limit: usize) -> Result<Vec<AuditEntry>> {
        self.query(AuditQuery::default().limit(limit))
    }

    /// Query entries for a session
    pub fn for_session(&self, session_id: &str) -> Result<Vec<AuditEntry>> {
        self.query(AuditQuery::default().session(session_id))
    }

    /// Query denied entries
    pub fn denied(&self) -> Result<Vec<AuditEntry>> {
        self.query(AuditQuery::default().action(Action::Deny))
    }

    /// Query with custom filter
    pub fn query(&self, query: AuditQuery) -> Result<Vec<AuditEntry>> {
        if !self.log_path.exists() {
            return Ok(Vec::new());
        }

        let file = File::open(&self.log_path)?;
        let reader = BufReader::new(file);

        let mut entries: Vec<AuditEntry> = reader
            .lines()
            .filter_map(|line| {
                line.ok().and_then(|l| serde_json::from_str(&l).ok())
            })
            .filter(|entry: &AuditEntry| query.matches(entry))
            .collect();

        // Sort by timestamp descending (most recent first)
        entries.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));

        // Apply limit
        if let Some(limit) = query.limit {
            entries.truncate(limit);
        }

        Ok(entries)
    }

    /// Clean up old entries
    pub fn cleanup(&self, retention_days: i64) -> Result<usize> {
        if !self.log_path.exists() {
            return Ok(0);
        }

        let threshold = Utc::now() - Duration::days(retention_days);

        let file = File::open(&self.log_path)?;
        let reader = BufReader::new(file);

        let mut kept: Vec<String> = Vec::new();
        let mut removed = 0;

        for line in reader.lines() {
            let line = line?;
            if let Ok(entry) = serde_json::from_str::<AuditEntry>(&line) {
                if entry.timestamp >= threshold {
                    kept.push(line);
                } else {
                    removed += 1;
                }
            }
        }

        // Rewrite file with kept entries
        if removed > 0 {
            let mut file = File::create(&self.log_path)?;
            for line in kept {
                writeln!(file, "{}", line)?;
            }
        }

        Ok(removed)
    }

    /// Get statistics
    pub fn stats(&self, since: Option<DateTime<Utc>>) -> Result<AuditStats> {
        let entries = if let Some(since) = since {
            self.query(AuditQuery::default().since(since))?
        } else {
            self.query(AuditQuery::default())?
        };

        let mut stats = AuditStats::default();

        for entry in entries {
            stats.total += 1;
            match entry.decision {
                Action::Allow => stats.allowed += 1,
                Action::Deny => stats.denied += 1,
                Action::Ask => stats.asked += 1,
            }
            if entry.user_prompted {
                stats.user_prompted += 1;
            }
            if entry.matched_pattern.is_some() {
                stats.pattern_matches += 1;
            }
        }

        Ok(stats)
    }
}

impl Default for AuditLog {
    fn default() -> Self {
        Self::new()
    }
}

/// Query parameters for audit log
#[derive(Debug, Clone, Default)]
pub struct AuditQuery {
    session_id: Option<String>,
    action: Option<Action>,
    since: Option<DateTime<Utc>>,
    limit: Option<usize>,
    operation: Option<String>,
}

impl AuditQuery {
    /// Filter by session
    pub fn session(mut self, session_id: &str) -> Self {
        self.session_id = Some(session_id.to_string());
        self
    }

    /// Filter by action
    pub fn action(mut self, action: Action) -> Self {
        self.action = Some(action);
        self
    }

    /// Filter by time
    pub fn since(mut self, since: DateTime<Utc>) -> Self {
        self.since = Some(since);
        self
    }

    /// Limit results
    pub fn limit(mut self, limit: usize) -> Self {
        self.limit = Some(limit);
        self
    }

    /// Filter by operation
    pub fn operation(mut self, operation: &str) -> Self {
        self.operation = Some(operation.to_string());
        self
    }

    /// Check if an entry matches the query
    fn matches(&self, entry: &AuditEntry) -> bool {
        if let Some(ref session) = self.session_id {
            if entry.session_id != *session {
                return false;
            }
        }

        if let Some(action) = self.action {
            if entry.decision != action {
                return false;
            }
        }

        if let Some(since) = self.since {
            if entry.timestamp < since {
                return false;
            }
        }

        if let Some(ref operation) = self.operation {
            if entry.operation != *operation {
                return false;
            }
        }

        true
    }
}

/// Audit statistics
#[derive(Debug, Clone, Default)]
pub struct AuditStats {
    pub total: usize,
    pub allowed: usize,
    pub denied: usize,
    pub asked: usize,
    pub user_prompted: usize,
    pub pattern_matches: usize,
}

impl AuditStats {
    /// Calculate the auto-allow rate
    pub fn auto_allow_rate(&self) -> f64 {
        if self.total == 0 {
            return 0.0;
        }
        let auto_allowed = self.allowed - self.user_prompted.min(self.allowed);
        (auto_allowed as f64) / (self.total as f64) * 100.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_audit_log() {
        let dir = tempdir().unwrap();
        let log_path = dir.path().join("audit.log");
        let log = AuditLog::with_path(log_path);

        let entry = AuditEntry {
            timestamp: Utc::now(),
            session_id: "test".to_string(),
            operation: "rm".to_string(),
            args: vec!["*.pyc".to_string()],
            working_dir: "/tmp".to_string(),
            trust_level: TrustLevel::Developer,
            decision: Action::Allow,
            reason: Reason::PatternMatch,
            details: "Test".to_string(),
            matched_pattern: Some("rm *.pyc".to_string()),
            user_prompted: false,
        };

        log.log(&entry).unwrap();

        let entries = log.recent(10).unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].operation, "rm");
    }

    #[test]
    fn test_audit_query() {
        let dir = tempdir().unwrap();
        let log_path = dir.path().join("audit.log");
        let log = AuditLog::with_path(log_path);

        // Log some entries
        for i in 0..5 {
            let entry = AuditEntry {
                timestamp: Utc::now(),
                session_id: format!("session{}", i % 2),
                operation: "rm".to_string(),
                args: vec![],
                working_dir: "/tmp".to_string(),
                trust_level: TrustLevel::Developer,
                decision: if i % 2 == 0 { Action::Allow } else { Action::Ask },
                reason: Reason::TrustLevel,
                details: "Test".to_string(),
                matched_pattern: None,
                user_prompted: i % 2 == 1,
            };
            log.log(&entry).unwrap();
        }

        // Query by session
        let session0 = log.query(AuditQuery::default().session("session0")).unwrap();
        assert_eq!(session0.len(), 3);

        // Query denied
        let asked = log.query(AuditQuery::default().action(Action::Ask)).unwrap();
        assert_eq!(asked.len(), 2);
    }
}
