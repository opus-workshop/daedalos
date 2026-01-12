//! Session management for oracle
//!
//! Sessions track conversation continuity across invocations.
//! They store the backend's session ID so we can resume conversations.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

/// Session data stored on disk
#[derive(Debug, Serialize, Deserialize)]
pub struct Session {
    /// Backend's session ID
    pub backend_session_id: String,

    /// Backend name that created this session
    pub backend: String,

    /// When the session was last used
    pub last_used: chrono::DateTime<chrono::Utc>,
}

/// Manages oracle sessions
pub struct SessionManager {
    /// Base directory for sessions
    base_dir: PathBuf,
}

impl SessionManager {
    /// Create a new session manager
    pub fn new() -> Result<Self> {
        let base_dir = dirs::data_local_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("oracle")
            .join("sessions");

        // Ensure directory exists
        fs::create_dir_all(&base_dir)
            .with_context(|| format!("Failed to create session directory: {:?}", base_dir))?;

        Ok(Self { base_dir })
    }

    /// Get the path for a named session
    fn session_path(&self, name: &str) -> PathBuf {
        self.base_dir.join(format!("{}.json", name))
    }

    /// Get the path to the "last" session symlink
    fn last_session_path(&self) -> PathBuf {
        self.base_dir.join("last.json")
    }

    /// Get the last used session ID
    pub fn get_last_session(&self) -> Result<Option<String>> {
        let path = self.last_session_path();
        self.read_session(&path)
    }

    /// Save a session as the "last" session
    pub fn save_last_session(&self, backend_session_id: &str) -> Result<()> {
        let path = self.last_session_path();
        self.write_session(&path, backend_session_id, "unknown")
    }

    /// Get or create a named session
    pub fn get_or_create_session(&self, name: &str) -> Result<Option<String>> {
        let path = self.session_path(name);
        self.read_session(&path)
    }

    /// Save a named session
    pub fn save_session(&self, name: &str, backend_session_id: &str) -> Result<()> {
        let path = self.session_path(name);
        self.write_session(&path, backend_session_id, "unknown")?;

        // Also update "last"
        self.save_last_session(backend_session_id)?;

        Ok(())
    }

    /// Read session from path
    fn read_session(&self, path: &PathBuf) -> Result<Option<String>> {
        if !path.exists() {
            return Ok(None);
        }

        let content = fs::read_to_string(path)
            .with_context(|| format!("Failed to read session: {:?}", path))?;

        let session: Session = serde_json::from_str(&content)
            .with_context(|| format!("Failed to parse session: {:?}", path))?;

        Ok(Some(session.backend_session_id))
    }

    /// Write session to path
    fn write_session(&self, path: &PathBuf, backend_session_id: &str, backend: &str) -> Result<()> {
        let session = Session {
            backend_session_id: backend_session_id.to_string(),
            backend: backend.to_string(),
            last_used: chrono::Utc::now(),
        };

        let content = serde_json::to_string_pretty(&session)?;
        fs::write(path, content)
            .with_context(|| format!("Failed to write session: {:?}", path))?;

        Ok(())
    }

    /// List all named sessions
    pub fn list_sessions(&self) -> Result<Vec<String>> {
        let mut sessions = Vec::new();

        for entry in fs::read_dir(&self.base_dir)? {
            let entry = entry?;
            let path = entry.path();

            if path.extension().map_or(false, |e| e == "json") {
                if let Some(name) = path.file_stem().and_then(|s| s.to_str()) {
                    if name != "last" {
                        sessions.push(name.to_string());
                    }
                }
            }
        }

        Ok(sessions)
    }

    /// Delete a session
    pub fn delete_session(&self, name: &str) -> Result<()> {
        let path = self.session_path(name);
        if path.exists() {
            fs::remove_file(&path)
                .with_context(|| format!("Failed to delete session: {:?}", path))?;
        }
        Ok(())
    }

    /// Get project-local session path (in .oracle/)
    pub fn project_session_path() -> Option<PathBuf> {
        // Find git root
        let output = std::process::Command::new("git")
            .args(["rev-parse", "--show-toplevel"])
            .output()
            .ok()?;

        if !output.status.success() {
            return None;
        }

        let root = String::from_utf8_lossy(&output.stdout).trim().to_string();
        Some(PathBuf::from(root).join(".oracle").join("session.json"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_session_manager() {
        let temp = TempDir::new().unwrap();
        let mgr = SessionManager {
            base_dir: temp.path().to_path_buf(),
        };

        // Initially no session
        assert!(mgr.get_last_session().unwrap().is_none());

        // Save a session
        mgr.save_last_session("test-123").unwrap();

        // Now we should get it back
        assert_eq!(
            mgr.get_last_session().unwrap(),
            Some("test-123".to_string())
        );

        // Named sessions
        assert!(mgr.get_or_create_session("myproject").unwrap().is_none());
        mgr.save_session("myproject", "proj-456").unwrap();
        assert_eq!(
            mgr.get_or_create_session("myproject").unwrap(),
            Some("proj-456".to_string())
        );
    }
}
