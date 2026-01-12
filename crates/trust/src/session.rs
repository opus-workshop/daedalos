//! Session management for trust
//!
//! Sessions track trust state for a specific interaction (loop, agent, terminal).
//! Escalations and approvals are session-scoped - they don't persist.

use crate::level::TrustLevel;
use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use daedalos_core::Paths;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::path::PathBuf;

/// A trust session
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Session {
    /// Unique session ID
    pub id: String,

    /// Session type (loop, agent, terminal)
    pub session_type: SessionType,

    /// Working directory for the session
    pub working_dir: PathBuf,

    /// Base trust level (from project)
    pub base_level: TrustLevel,

    /// Escalated trust level (if any)
    pub escalated_level: Option<TrustLevel>,

    /// Approved patterns for this session
    pub approved_patterns: HashSet<String>,

    /// Approved domains for this session
    pub approved_domains: HashSet<String>,

    /// Approved categories for this session
    pub approved_categories: HashSet<String>,

    /// When the session started
    pub started_at: DateTime<Utc>,

    /// Last activity
    pub last_activity: DateTime<Utc>,

    /// Parent session (for nested agents)
    pub parent_id: Option<String>,
}

/// Type of session
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SessionType {
    /// Interactive terminal session
    Terminal,
    /// Loop iteration session
    Loop,
    /// Agent session
    Agent,
    /// Explorer (read-only agent)
    Explorer,
}

impl Session {
    /// Create a new session
    pub fn new(
        id: String,
        session_type: SessionType,
        working_dir: PathBuf,
        base_level: TrustLevel,
    ) -> Self {
        let now = Utc::now();
        Self {
            id,
            session_type,
            working_dir,
            base_level,
            escalated_level: None,
            approved_patterns: HashSet::new(),
            approved_domains: HashSet::new(),
            approved_categories: HashSet::new(),
            started_at: now,
            last_activity: now,
            parent_id: None,
        }
    }

    /// Create a child session (for subagents)
    pub fn child(&self, child_id: String, session_type: SessionType) -> Self {
        let now = Utc::now();

        // Child inherits parent's approvals but not escalation
        let mut child = Self {
            id: child_id,
            session_type,
            working_dir: self.working_dir.clone(),
            base_level: self.effective_level().min(TrustLevel::Developer), // Cap at developer
            escalated_level: None,
            approved_patterns: self.approved_patterns.clone(),
            approved_domains: self.approved_domains.clone(),
            approved_categories: self.approved_categories.clone(),
            started_at: now,
            last_activity: now,
            parent_id: Some(self.id.clone()),
        };

        // Explorers are always read-only
        if session_type == SessionType::Explorer {
            child.base_level = TrustLevel::Guest;
        }

        child
    }

    /// Get the effective trust level
    pub fn effective_level(&self) -> TrustLevel {
        self.escalated_level.unwrap_or(self.base_level)
    }

    /// Escalate the session to a higher trust level
    pub fn escalate(&mut self, level: TrustLevel) -> bool {
        // Can only escalate up to max for base level
        let max = self.base_level.max_escalation();
        if level > max {
            return false;
        }

        self.escalated_level = Some(level);
        self.last_activity = Utc::now();
        true
    }

    /// Approve a pattern for this session
    pub fn approve_pattern(&mut self, pattern: &str) {
        self.approved_patterns.insert(pattern.to_string());
        self.last_activity = Utc::now();
    }

    /// Approve a domain for this session
    pub fn approve_domain(&mut self, domain: &str) {
        self.approved_domains.insert(domain.to_lowercase());
        self.last_activity = Utc::now();
    }

    /// Approve a category for this session
    pub fn approve_category(&mut self, category: &str) {
        self.approved_categories.insert(category.to_string());
        self.last_activity = Utc::now();
    }

    /// Check if a pattern is approved
    pub fn has_approved_pattern(&self, pattern: &str) -> bool {
        self.approved_patterns.contains(pattern)
    }

    /// Check if a domain is approved
    pub fn has_approved_domain(&self, domain: &str) -> bool {
        self.approved_domains.contains(&domain.to_lowercase())
    }

    /// Check if a category is approved
    pub fn has_approved_category(&self, category: &str) -> bool {
        self.approved_categories.contains(category)
    }

    /// Record activity
    pub fn touch(&mut self) {
        self.last_activity = Utc::now();
    }
}

/// Session manager - handles session persistence and lifecycle
pub struct SessionManager {
    sessions_dir: PathBuf,
}

impl SessionManager {
    /// Create a new session manager
    pub fn new() -> Self {
        let paths = Paths::new();
        let sessions_dir = paths.data.join("trust").join("sessions");
        Self { sessions_dir }
    }

    /// Create with custom path
    pub fn with_path(sessions_dir: PathBuf) -> Self {
        Self { sessions_dir }
    }

    /// Start a new session
    pub fn start_session(
        &self,
        session_type: SessionType,
        working_dir: PathBuf,
        base_level: TrustLevel,
    ) -> Result<Session> {
        let id = generate_session_id();
        let session = Session::new(id, session_type, working_dir, base_level);
        self.save_session(&session)?;
        Ok(session)
    }

    /// Load a session by ID
    pub fn load_session(&self, id: &str) -> Result<Option<Session>> {
        let path = self.session_path(id);
        if !path.exists() {
            return Ok(None);
        }

        let content = std::fs::read_to_string(&path)
            .with_context(|| format!("Failed to read session {}", id))?;
        let session: Session = serde_json::from_str(&content)
            .with_context(|| format!("Failed to parse session {}", id))?;
        Ok(Some(session))
    }

    /// Save a session
    pub fn save_session(&self, session: &Session) -> Result<()> {
        std::fs::create_dir_all(&self.sessions_dir)?;
        let path = self.session_path(&session.id);
        let content = serde_json::to_string_pretty(session)?;
        std::fs::write(&path, content)?;
        Ok(())
    }

    /// End a session (remove its file)
    pub fn end_session(&self, id: &str) -> Result<()> {
        let path = self.session_path(id);
        if path.exists() {
            std::fs::remove_file(&path)?;
        }
        Ok(())
    }

    /// List all active sessions
    pub fn list_sessions(&self) -> Result<Vec<Session>> {
        let mut sessions = Vec::new();

        if !self.sessions_dir.exists() {
            return Ok(sessions);
        }

        for entry in std::fs::read_dir(&self.sessions_dir)? {
            let entry = entry?;
            let path = entry.path();

            if path.extension().map_or(false, |e| e == "json") {
                if let Ok(content) = std::fs::read_to_string(&path) {
                    if let Ok(session) = serde_json::from_str::<Session>(&content) {
                        sessions.push(session);
                    }
                }
            }
        }

        // Sort by last activity, most recent first
        sessions.sort_by(|a, b| b.last_activity.cmp(&a.last_activity));

        Ok(sessions)
    }

    /// Clean up stale sessions (older than given hours)
    pub fn cleanup_stale(&self, hours: i64) -> Result<usize> {
        let threshold = Utc::now() - chrono::Duration::hours(hours);
        let mut removed = 0;

        if !self.sessions_dir.exists() {
            return Ok(0);
        }

        for entry in std::fs::read_dir(&self.sessions_dir)? {
            let entry = entry?;
            let path = entry.path();

            if path.extension().map_or(false, |e| e == "json") {
                if let Ok(content) = std::fs::read_to_string(&path) {
                    if let Ok(session) = serde_json::from_str::<Session>(&content) {
                        if session.last_activity < threshold {
                            if std::fs::remove_file(&path).is_ok() {
                                removed += 1;
                            }
                        }
                    }
                }
            }
        }

        Ok(removed)
    }

    /// Get path for a session file
    fn session_path(&self, id: &str) -> PathBuf {
        self.sessions_dir.join(format!("{}.json", id))
    }
}

impl Default for SessionManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Generate a unique session ID
fn generate_session_id() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};

    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();

    // Use last 8 bytes + some randomness from nanos
    format!("ses_{:016x}", nanos & 0xFFFFFFFFFFFFFFFF)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_session_escalation() {
        let mut session = Session::new(
            "test".to_string(),
            SessionType::Loop,
            PathBuf::from("/tmp"),
            TrustLevel::Contractor,
        );

        // Can escalate to developer
        assert!(session.escalate(TrustLevel::Developer));
        assert_eq!(session.effective_level(), TrustLevel::Developer);

        // Cannot escalate to owner
        assert!(!session.escalate(TrustLevel::Owner));
    }

    #[test]
    fn test_child_session() {
        let parent = Session::new(
            "parent".to_string(),
            SessionType::Loop,
            PathBuf::from("/tmp"),
            TrustLevel::Developer,
        );

        let child = parent.child("child".to_string(), SessionType::Agent);
        assert_eq!(child.base_level, TrustLevel::Developer);
        assert_eq!(child.parent_id, Some("parent".to_string()));

        let explorer = parent.child("explorer".to_string(), SessionType::Explorer);
        assert_eq!(explorer.base_level, TrustLevel::Guest);
    }

    #[test]
    fn test_session_approvals() {
        let mut session = Session::new(
            "test".to_string(),
            SessionType::Terminal,
            PathBuf::from("/tmp"),
            TrustLevel::Developer,
        );

        session.approve_pattern("rm *.pyc");
        session.approve_domain("api.example.com");
        session.approve_category("network");

        assert!(session.has_approved_pattern("rm *.pyc"));
        assert!(session.has_approved_domain("api.example.com"));
        assert!(session.has_approved_category("network"));
    }
}
