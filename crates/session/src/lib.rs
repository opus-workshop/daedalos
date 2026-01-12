//! session - Save/restore terminal sessions for Daedalos
//!
//! "Capture and restore your complete terminal state."
//!
//! Sessions capture the state of your terminal environment:
//! - Current working directory
//! - Environment variables (filtered for security)
//! - Git branch and status
//! - Tmux window/pane layout
//! - Recent shell history
//! - Daedalos project context

use anyhow::{bail, Context, Result};
use chrono::{DateTime, Utc};
use daedalos_core::Paths;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::Command;

/// Session metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionInfo {
    /// Session name
    pub name: String,
    /// Unix timestamp when created
    pub created: i64,
    /// Working directory
    pub cwd: PathBuf,
    /// Username
    pub user: String,
    /// Hostname
    pub hostname: String,
    /// Shell path
    pub shell: String,
}

/// Git repository state
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitState {
    /// Repository root path
    pub root: PathBuf,
    /// Current branch name
    pub branch: String,
    /// Current commit hash
    pub commit: String,
    /// Number of modified files
    pub modified_count: usize,
}

/// Tmux window information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TmuxWindow {
    /// Window index
    pub index: usize,
    /// Window name
    pub name: String,
    /// Current path in the window
    pub path: PathBuf,
}

/// Tmux pane information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TmuxPane {
    /// Pane index
    pub index: usize,
    /// Current path in the pane
    pub path: PathBuf,
    /// Current command running
    pub command: String,
}

/// Complete session state
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Session {
    /// Session metadata
    pub info: SessionInfo,
    /// Environment variables (filtered)
    pub env: HashMap<String, String>,
    /// Daedalos-specific environment variables
    pub daedalos_env: HashMap<String, String>,
    /// Git state (if in a repo)
    pub git: Option<GitState>,
    /// Tmux windows (if in tmux)
    pub tmux_windows: Vec<TmuxWindow>,
    /// Tmux panes (if in tmux)
    pub tmux_panes: Vec<TmuxPane>,
    /// Recent shell history
    pub history: Vec<String>,
}

/// Session store for managing saved sessions
pub struct SessionStore {
    /// Path to sessions directory
    sessions_dir: PathBuf,
}

impl SessionStore {
    /// Create a new session store
    pub fn new(base_dir: &Path) -> Result<Self> {
        let sessions_dir = base_dir.join("sessions");
        fs::create_dir_all(&sessions_dir)
            .with_context(|| format!("Failed to create sessions directory: {}", sessions_dir.display()))?;
        Ok(Self { sessions_dir })
    }

    /// Create from Daedalos paths
    pub fn from_paths() -> Result<Self> {
        let paths = Paths::new();
        Self::new(&paths.data.join("session"))
    }

    /// Check if a session exists
    pub fn exists(&self, name: &str) -> bool {
        self.sessions_dir.join(name).is_dir()
    }

    /// Get path to a session directory
    fn session_path(&self, name: &str) -> PathBuf {
        self.sessions_dir.join(name)
    }

    /// Save a session
    pub fn save(&self, name: &str, session: &Session) -> Result<PathBuf> {
        let session_dir = self.session_path(name);

        // Remove existing if present
        if session_dir.exists() {
            fs::remove_dir_all(&session_dir)
                .with_context(|| format!("Failed to remove existing session: {}", name))?;
        }

        fs::create_dir_all(&session_dir)
            .with_context(|| format!("Failed to create session directory: {}", session_dir.display()))?;

        // Write session JSON
        let session_file = session_dir.join("session.json");
        let json = serde_json::to_string_pretty(session)?;
        fs::write(&session_file, json)?;

        // Also write legacy files for compatibility
        fs::write(session_dir.join("cwd"), session.info.cwd.to_string_lossy().as_ref())?;
        fs::write(session_dir.join("name"), name)?;

        // Write info.json for legacy compatibility
        let info_json = serde_json::to_string_pretty(&session.info)?;
        fs::write(session_dir.join("info.json"), info_json)?;

        Ok(session_dir)
    }

    /// Load a session by name
    pub fn load(&self, name: &str) -> Result<Session> {
        let session_dir = self.session_path(name);

        if !session_dir.exists() {
            bail!("Session not found: {}", name);
        }

        let session_file = session_dir.join("session.json");
        let json = fs::read_to_string(&session_file)
            .with_context(|| format!("Failed to read session file: {}", session_file.display()))?;

        let session: Session = serde_json::from_str(&json)?;
        Ok(session)
    }

    /// List all sessions
    pub fn list(&self) -> Result<Vec<SessionSummary>> {
        let mut sessions = Vec::new();

        if !self.sessions_dir.exists() {
            return Ok(sessions);
        }

        for entry in fs::read_dir(&self.sessions_dir)? {
            let entry = entry?;
            let path = entry.path();

            if !path.is_dir() {
                continue;
            }

            let name = path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("unknown")
                .to_string();

            // Try to load session info
            let session_file = path.join("session.json");
            if let Ok(json) = fs::read_to_string(&session_file) {
                if let Ok(session) = serde_json::from_str::<Session>(&json) {
                    sessions.push(SessionSummary {
                        name,
                        cwd: session.info.cwd,
                        created: DateTime::from_timestamp(session.info.created, 0)
                            .unwrap_or_else(|| Utc::now()),
                        git_branch: session.git.map(|g| g.branch),
                    });
                    continue;
                }
            }

            // Fallback to legacy format
            let cwd = fs::read_to_string(path.join("cwd"))
                .map(|s| PathBuf::from(s.trim()))
                .unwrap_or_default();

            let created = path
                .metadata()
                .ok()
                .and_then(|m| m.created().ok())
                .map(|t| DateTime::<Utc>::from(t))
                .unwrap_or_else(Utc::now);

            sessions.push(SessionSummary {
                name,
                cwd,
                created,
                git_branch: None,
            });
        }

        // Sort by creation time (newest first)
        sessions.sort_by(|a, b| b.created.cmp(&a.created));

        Ok(sessions)
    }

    /// Delete a session
    pub fn delete(&self, name: &str) -> Result<()> {
        let session_dir = self.session_path(name);

        if !session_dir.exists() {
            bail!("Session not found: {}", name);
        }

        fs::remove_dir_all(&session_dir)
            .with_context(|| format!("Failed to delete session: {}", name))?;

        Ok(())
    }

    /// Export a session to a tar.gz file
    pub fn export(&self, name: &str, output: &Path) -> Result<()> {
        let session_dir = self.session_path(name);

        if !session_dir.exists() {
            bail!("Session not found: {}", name);
        }

        let status = Command::new("tar")
            .args(["-czf", &output.to_string_lossy()])
            .arg("-C")
            .arg(&self.sessions_dir)
            .arg(name)
            .status()
            .context("Failed to run tar")?;

        if !status.success() {
            bail!("tar command failed");
        }

        Ok(())
    }

    /// Import a session from a tar.gz file
    pub fn import(&self, file: &Path) -> Result<String> {
        if !file.exists() {
            bail!("Import file not found: {}", file.display());
        }

        let status = Command::new("tar")
            .args(["-xzf", &file.to_string_lossy()])
            .arg("-C")
            .arg(&self.sessions_dir)
            .status()
            .context("Failed to run tar")?;

        if !status.success() {
            bail!("tar command failed");
        }

        // Extract session name from archive
        let name = file
            .file_stem()
            .and_then(|s| s.to_str())
            .map(|s| s.trim_end_matches(".session"))
            .unwrap_or("imported")
            .to_string();

        Ok(name)
    }
}

/// Summary of a session for listing
#[derive(Debug, Clone)]
pub struct SessionSummary {
    /// Session name
    pub name: String,
    /// Working directory
    pub cwd: PathBuf,
    /// Creation time
    pub created: DateTime<Utc>,
    /// Git branch if available
    pub git_branch: Option<String>,
}

/// Capture current session state
pub fn capture_session(name: &str) -> Result<Session> {
    let cwd = std::env::current_dir().context("Failed to get current directory")?;
    let user = std::env::var("USER").unwrap_or_else(|_| "unknown".to_string());
    let hostname = get_hostname();
    let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/sh".to_string());

    let info = SessionInfo {
        name: name.to_string(),
        created: Utc::now().timestamp(),
        cwd: cwd.clone(),
        user,
        hostname,
        shell,
    };

    // Capture environment variables (filtered)
    let env = capture_environment();
    let daedalos_env = capture_daedalos_environment();

    // Capture git state
    let git = capture_git_state(&cwd);

    // Capture tmux state
    let (tmux_windows, tmux_panes) = if std::env::var("TMUX").is_ok() {
        (capture_tmux_windows(), capture_tmux_panes())
    } else {
        (Vec::new(), Vec::new())
    };

    // Capture shell history
    let history = capture_history();

    Ok(Session {
        info,
        env,
        daedalos_env,
        git,
        tmux_windows,
        tmux_panes,
        history,
    })
}

/// Get the hostname
fn get_hostname() -> String {
    Command::new("hostname")
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| s.trim().to_string())
        .unwrap_or_else(|| "unknown".to_string())
}

/// Capture environment variables, filtering out secrets
fn capture_environment() -> HashMap<String, String> {
    let secret_patterns = ["PASSWORD", "SECRET", "TOKEN", "KEY", "CREDENTIAL", "AUTH", "API_KEY"];

    std::env::vars()
        .filter(|(k, _)| {
            let upper = k.to_uppercase();
            !secret_patterns.iter().any(|p| upper.contains(p))
        })
        .collect()
}

/// Capture Daedalos-specific environment variables
fn capture_daedalos_environment() -> HashMap<String, String> {
    std::env::vars()
        .filter(|(k, _)| k.starts_with("DAEDALOS"))
        .collect()
}

/// Capture git state from current directory
fn capture_git_state(cwd: &Path) -> Option<GitState> {
    // Check if we're in a git repo
    let git_dir = Command::new("git")
        .args(["rev-parse", "--git-dir"])
        .current_dir(cwd)
        .output()
        .ok()?;

    if !git_dir.status.success() {
        return None;
    }

    let root = Command::new("git")
        .args(["rev-parse", "--show-toplevel"])
        .current_dir(cwd)
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| PathBuf::from(s.trim()))
        .unwrap_or_else(|| cwd.to_path_buf());

    let branch = Command::new("git")
        .args(["branch", "--show-current"])
        .current_dir(cwd)
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| s.trim().to_string())
        .unwrap_or_default();

    let commit = Command::new("git")
        .args(["rev-parse", "HEAD"])
        .current_dir(cwd)
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| s.trim().to_string())
        .unwrap_or_default();

    let modified_count = Command::new("git")
        .args(["status", "--porcelain"])
        .current_dir(cwd)
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| s.lines().count())
        .unwrap_or(0);

    Some(GitState {
        root,
        branch,
        commit,
        modified_count,
    })
}

/// Capture tmux windows
fn capture_tmux_windows() -> Vec<TmuxWindow> {
    let output = Command::new("tmux")
        .args(["list-windows", "-F", "#{window_index}:#{window_name}:#{pane_current_path}"])
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .unwrap_or_default();

    output
        .lines()
        .filter_map(|line| {
            let parts: Vec<&str> = line.splitn(3, ':').collect();
            if parts.len() == 3 {
                Some(TmuxWindow {
                    index: parts[0].parse().unwrap_or(0),
                    name: parts[1].to_string(),
                    path: PathBuf::from(parts[2]),
                })
            } else {
                None
            }
        })
        .collect()
}

/// Capture tmux panes
fn capture_tmux_panes() -> Vec<TmuxPane> {
    let output = Command::new("tmux")
        .args(["list-panes", "-F", "#{pane_index}:#{pane_current_path}:#{pane_current_command}"])
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .unwrap_or_default();

    output
        .lines()
        .filter_map(|line| {
            let parts: Vec<&str> = line.splitn(3, ':').collect();
            if parts.len() == 3 {
                Some(TmuxPane {
                    index: parts[0].parse().unwrap_or(0),
                    path: PathBuf::from(parts[1]),
                    command: parts[2].to_string(),
                })
            } else {
                None
            }
        })
        .collect()
}

/// Capture recent shell history
fn capture_history() -> Vec<String> {
    let histfile = std::env::var("HISTFILE").ok();

    let history_file = histfile
        .map(PathBuf::from)
        .or_else(|| {
            let home = dirs::home_dir()?;
            let shell = std::env::var("SHELL").unwrap_or_default();

            if shell.contains("zsh") {
                Some(home.join(".zsh_history"))
            } else {
                Some(home.join(".bash_history"))
            }
        });

    let Some(history_path) = history_file else {
        return Vec::new();
    };

    let Ok(file) = fs::File::open(&history_path) else {
        return Vec::new();
    };

    let reader = BufReader::new(file);
    let lines: Vec<String> = reader
        .lines()
        .filter_map(|l| l.ok())
        .collect();

    // Get last 100 lines
    lines.into_iter().rev().take(100).rev().collect()
}

/// Generate a restore script for a session
pub fn generate_restore_script(session: &Session) -> String {
    let mut script = String::new();

    // Change directory
    if session.info.cwd.exists() {
        script.push_str(&format!("cd '{}'\n", session.info.cwd.display()));
    }

    // Restore Daedalos environment
    for (key, value) in &session.daedalos_env {
        script.push_str(&format!("export {}='{}'\n", key, value));
    }

    // Source project environment if available
    let env_file = session.info.cwd.join(".daedalos/env.sh");
    if env_file.exists() {
        script.push_str(&format!("source '{}'\n", env_file.display()));
    }

    script
}

/// Format time as relative time ago
pub fn time_ago(dt: &DateTime<Utc>) -> String {
    let now = Utc::now();
    let duration = now.signed_duration_since(*dt);

    let seconds = duration.num_seconds();
    if seconds < 60 {
        return "just now".to_string();
    }

    let minutes = duration.num_minutes();
    if minutes < 60 {
        return format!("{} min ago", minutes);
    }

    let hours = duration.num_hours();
    if hours < 24 {
        return format!("{} hours ago", hours);
    }

    let days = duration.num_days();
    format!("{} days ago", days)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_time_ago() {
        let now = Utc::now();
        assert_eq!(time_ago(&now), "just now");

        let one_hour_ago = now - chrono::Duration::hours(1);
        assert_eq!(time_ago(&one_hour_ago), "1 hours ago");

        let two_days_ago = now - chrono::Duration::days(2);
        assert_eq!(time_ago(&two_days_ago), "2 days ago");
    }

    #[test]
    fn test_capture_environment_filters_secrets() {
        // Set a test secret
        std::env::set_var("TEST_PASSWORD", "secret123");
        std::env::set_var("TEST_NORMAL_VAR", "normal");

        let env = capture_environment();

        assert!(!env.contains_key("TEST_PASSWORD"));
        assert!(env.contains_key("TEST_NORMAL_VAR"));

        // Cleanup
        std::env::remove_var("TEST_PASSWORD");
        std::env::remove_var("TEST_NORMAL_VAR");
    }

    #[test]
    fn test_capture_daedalos_environment() {
        std::env::set_var("DAEDALOS_TEST", "value");
        std::env::set_var("OTHER_VAR", "other");

        let env = capture_daedalos_environment();

        assert!(env.contains_key("DAEDALOS_TEST"));
        assert!(!env.contains_key("OTHER_VAR"));

        std::env::remove_var("DAEDALOS_TEST");
        std::env::remove_var("OTHER_VAR");
    }
}
