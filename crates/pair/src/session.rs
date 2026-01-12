//! Pair session management

use anyhow::{bail, Context, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

/// Pairing mode - controls who can drive
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PairMode {
    /// Both participants can control
    Equal,
    /// You control, partner watches
    Driver,
    /// Partner controls, you watch
    Navigator,
}

impl PairMode {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Equal => "equal",
            Self::Driver => "driver",
            Self::Navigator => "navigator",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "equal" => Some(Self::Equal),
            "driver" => Some(Self::Driver),
            "navigator" => Some(Self::Navigator),
            _ => None,
        }
    }
}

impl Default for PairMode {
    fn default() -> Self {
        Self::Equal
    }
}

/// Information about a pair session
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PairSession {
    /// Session name
    pub name: String,
    /// When the session was started
    pub started: DateTime<Utc>,
    /// Project directory
    pub project: PathBuf,
    /// Pairing mode
    pub mode: PairMode,
    /// Path to the tmux socket
    pub socket: PathBuf,
    /// Host user who started the session
    pub host: String,
    /// Whether using tmate
    pub tmate: bool,
}

impl PairSession {
    /// Create a new pair session
    pub fn new(
        name: String,
        project: PathBuf,
        socket: PathBuf,
        mode: PairMode,
        host: String,
        tmate: bool,
    ) -> Self {
        Self {
            name,
            started: Utc::now(),
            project,
            mode,
            socket,
            host,
            tmate,
        }
    }

    /// Check if the session's tmux socket exists and is active
    pub fn is_active(&self) -> bool {
        if !self.socket.exists() {
            return false;
        }

        // Try to check if the tmux session is responsive
        let program = if self.tmate { "tmate" } else { "tmux" };
        Command::new(program)
            .args(["-S", &self.socket.to_string_lossy(), "has-session", "-t", &self.name])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .map(|s| s.success())
            .unwrap_or(false)
    }

    /// Get the number of connected clients
    pub fn client_count(&self) -> usize {
        let program = if self.tmate { "tmate" } else { "tmux" };
        Command::new(program)
            .args(["-S", &self.socket.to_string_lossy(), "list-clients"])
            .output()
            .ok()
            .map(|o| {
                String::from_utf8_lossy(&o.stdout)
                    .lines()
                    .count()
            })
            .unwrap_or(0)
    }
}

/// Start a new pair session with tmux
pub fn start_session(
    name: &str,
    project: &Path,
    socket: &Path,
    _mode: PairMode,
) -> Result<()> {
    // Create socket directory if needed
    if let Some(parent) = socket.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("Failed to create socket directory: {}", parent.display()))?;

        // Set directory permissions to allow group access
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let perms = std::fs::Permissions::from_mode(0o700);
            std::fs::set_permissions(parent, perms).ok();
        }
    }

    // Create new detached tmux session
    let status = Command::new("tmux")
        .args([
            "-S", &socket.to_string_lossy(),
            "new-session",
            "-d",
            "-s", name,
            "-c", &project.to_string_lossy(),
        ])
        .status()
        .context("Failed to start tmux session")?;

    if !status.success() {
        bail!("tmux new-session failed");
    }

    // Make socket accessible to others (for local pairing)
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let perms = std::fs::Permissions::from_mode(0o777);
        std::fs::set_permissions(socket, perms)
            .context("Failed to set socket permissions")?;
    }

    Ok(())
}

/// Start a new pair session with tmate
pub fn start_tmate_session(
    name: &str,
    project: &Path,
    socket: &Path,
) -> Result<String> {
    // Create socket directory if needed
    if let Some(parent) = socket.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("Failed to create socket directory: {}", parent.display()))?;
    }

    // Create new detached tmate session
    let status = Command::new("tmate")
        .args([
            "-S", &socket.to_string_lossy(),
            "new-session",
            "-d",
            "-s", name,
            "-c", &project.to_string_lossy(),
        ])
        .status()
        .context("Failed to start tmate session")?;

    if !status.success() {
        bail!("tmate new-session failed");
    }

    // Wait a moment for tmate to establish connection
    std::thread::sleep(std::time::Duration::from_secs(2));

    // Get the SSH connection string
    let output = Command::new("tmate")
        .args(["-S", &socket.to_string_lossy(), "display", "-p", "#{tmate_ssh}"])
        .output()
        .context("Failed to get tmate SSH string")?;

    let ssh_string = String::from_utf8_lossy(&output.stdout).trim().to_string();

    Ok(ssh_string)
}

/// Join an existing pair session
pub fn join_session(socket: &Path, name: &str, readonly: bool, tmate: bool) -> Result<()> {
    if !socket.exists() {
        bail!("Session socket not found: {}", socket.display());
    }

    let program = if tmate { "tmate" } else { "tmux" };
    let socket_str = socket.to_string_lossy().into_owned();

    let args: Vec<&str> = if readonly {
        vec!["-S", &socket_str, "attach", "-t", name, "-r"]
    } else {
        vec!["-S", &socket_str, "attach", "-t", name]
    };

    let status = Command::new(program)
        .args(&args)
        .status()
        .context("Failed to attach to session")?;

    if !status.success() {
        bail!("Failed to attach to session");
    }

    Ok(())
}

/// Leave current pair session (detach from tmux)
pub fn leave_session() -> Result<()> {
    // Check if we're in a tmux session
    if std::env::var("TMUX").is_err() {
        bail!("Not in a pair session");
    }

    let status = Command::new("tmux")
        .arg("detach")
        .status()
        .context("Failed to detach from session")?;

    if !status.success() {
        bail!("Failed to detach from session");
    }

    Ok(())
}

/// End a pair session (kill the tmux session)
pub fn end_session(socket: &Path, name: &str, tmate: bool) -> Result<()> {
    let program = if tmate { "tmate" } else { "tmux" };

    // Kill the tmux session
    Command::new(program)
        .args(["-S", &socket.to_string_lossy(), "kill-session", "-t", name])
        .status()
        .ok();

    // Remove the socket file
    std::fs::remove_file(socket).ok();

    Ok(())
}

/// Get current tmux session name if in one
pub fn current_session_name() -> Option<String> {
    if std::env::var("TMUX").is_err() {
        return None;
    }

    Command::new("tmux")
        .args(["display-message", "-p", "#S"])
        .output()
        .ok()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
}
