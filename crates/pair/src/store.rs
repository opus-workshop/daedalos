//! Persistent storage for pair sessions

use anyhow::{Context, Result};
use std::fs;
use std::path::{Path, PathBuf};

use crate::session::PairSession;

/// Store for managing pair session metadata
pub struct PairStore {
    /// Base directory for pair data
    data_dir: PathBuf,
    /// Directory for tmux sockets
    socket_dir: PathBuf,
}

impl PairStore {
    /// Create a new pair store
    pub fn new(data_dir: &Path) -> Result<Self> {
        let socket_dir = data_dir.join("sockets");

        // Ensure directories exist
        fs::create_dir_all(&data_dir)
            .with_context(|| format!("Failed to create data dir: {}", data_dir.display()))?;
        fs::create_dir_all(&socket_dir)
            .with_context(|| format!("Failed to create socket dir: {}", socket_dir.display()))?;

        // Set socket directory permissions
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let perms = fs::Permissions::from_mode(0o700);
            fs::set_permissions(&socket_dir, perms).ok();
        }

        Ok(Self {
            data_dir: data_dir.to_path_buf(),
            socket_dir,
        })
    }

    /// Get the socket path for a session
    pub fn socket_path(&self, name: &str) -> PathBuf {
        self.socket_dir.join(format!("{}.sock", name))
    }

    /// Get the metadata file path for a session
    pub fn meta_path(&self, name: &str) -> PathBuf {
        self.data_dir.join(format!("{}.json", name))
    }

    /// Check if a session exists
    pub fn exists(&self, name: &str) -> bool {
        self.socket_path(name).exists()
    }

    /// Save session metadata
    pub fn save_session(&self, session: &PairSession) -> Result<()> {
        let path = self.meta_path(&session.name);
        let json = serde_json::to_string_pretty(session)
            .context("Failed to serialize session")?;
        fs::write(&path, json)
            .with_context(|| format!("Failed to write session metadata: {}", path.display()))?;
        Ok(())
    }

    /// Load session metadata
    pub fn load_session(&self, name: &str) -> Result<Option<PairSession>> {
        let path = self.meta_path(name);
        if !path.exists() {
            return Ok(None);
        }

        let json = fs::read_to_string(&path)
            .with_context(|| format!("Failed to read session metadata: {}", path.display()))?;
        let session: PairSession = serde_json::from_str(&json)
            .with_context(|| format!("Failed to parse session metadata: {}", path.display()))?;
        Ok(Some(session))
    }

    /// List all active sessions
    pub fn list_sessions(&self) -> Result<Vec<PairSession>> {
        let mut sessions = Vec::new();

        // Find all socket files
        let entries = match fs::read_dir(&self.socket_dir) {
            Ok(entries) => entries,
            Err(_) => return Ok(sessions),
        };

        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().map(|e| e == "sock").unwrap_or(false) {
                // Check if it's actually a socket
                #[cfg(unix)]
                {
                    use std::os::unix::fs::FileTypeExt;
                    if let Ok(meta) = fs::metadata(&path) {
                        if !meta.file_type().is_socket() {
                            continue;
                        }
                    }
                }

                // Get session name from socket filename
                if let Some(name) = path.file_stem().and_then(|s| s.to_str()) {
                    // Try to load metadata, or create minimal info
                    let session = self.load_session(name)?.unwrap_or_else(|| {
                        PairSession::new(
                            name.to_string(),
                            PathBuf::from("."),
                            path.clone(),
                            crate::session::PairMode::Equal,
                            "unknown".to_string(),
                            false,
                        )
                    });

                    // Only include if session is still active
                    if session.is_active() {
                        sessions.push(session);
                    }
                }
            }
        }

        // Sort by start time (newest first)
        sessions.sort_by(|a, b| b.started.cmp(&a.started));

        Ok(sessions)
    }

    /// Remove session metadata
    pub fn remove_session(&self, name: &str) -> Result<()> {
        let meta_path = self.meta_path(name);
        let socket_path = self.socket_path(name);

        // Remove metadata file
        if meta_path.exists() {
            fs::remove_file(&meta_path).ok();
        }

        // Remove socket file
        if socket_path.exists() {
            fs::remove_file(&socket_path).ok();
        }

        Ok(())
    }

    /// Get the most recently created session
    pub fn most_recent_session(&self) -> Result<Option<PairSession>> {
        let sessions = self.list_sessions()?;
        Ok(sessions.into_iter().next())
    }

    /// Clean up stale sessions (sockets that no longer work)
    pub fn cleanup_stale(&self) -> Result<usize> {
        let mut removed = 0;

        let entries = match fs::read_dir(&self.socket_dir) {
            Ok(entries) => entries,
            Err(_) => return Ok(0),
        };

        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().map(|e| e == "sock").unwrap_or(false) {
                if let Some(name) = path.file_stem().and_then(|s| s.to_str()) {
                    // Check if session is still active
                    let is_active = self.load_session(name)?
                        .map(|s| s.is_active())
                        .unwrap_or(false);

                    if !is_active {
                        self.remove_session(name)?;
                        removed += 1;
                    }
                }
            }
        }

        Ok(removed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_store_creation() {
        let temp = TempDir::new().unwrap();
        let store = PairStore::new(temp.path()).unwrap();
        assert!(store.socket_dir.exists());
    }

    #[test]
    fn test_socket_path() {
        let temp = TempDir::new().unwrap();
        let store = PairStore::new(temp.path()).unwrap();
        let path = store.socket_path("test-session");
        assert!(path.ends_with("sockets/test-session.sock"));
    }
}
