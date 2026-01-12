//! tmux session management for agents
//!
//! Provides functions for creating, managing, and interacting with tmux sessions.

use anyhow::{Context, Result};
use std::process::Command;

use crate::state::TMUX_SESSION_PREFIX;

/// Check if tmux is available
pub fn is_available() -> bool {
    Command::new("tmux")
        .arg("-V")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// Check if a tmux session exists
pub fn session_exists(session: &str) -> bool {
    Command::new("tmux")
        .args(["has-session", "-t", session])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// Get session name for an agent
#[allow(dead_code)]
pub fn session_name(agent_name: &str) -> String {
    format!("{}{}", TMUX_SESSION_PREFIX, agent_name)
}

/// Create a new tmux session for an agent
pub fn create_session(
    session: &str,
    working_dir: &str,
    command: &[&str],
    agent_name: &str,
    data_dir: &str,
) -> Result<()> {
    if session_exists(session) {
        anyhow::bail!("Session already exists: {}", session);
    }

    // Create detached session
    let status = Command::new("tmux")
        .args(["new-session", "-d", "-s", session, "-c", working_dir])
        .status()
        .context("Failed to create tmux session")?;

    if !status.success() {
        anyhow::bail!("tmux new-session failed");
    }

    // Set session options
    let _ = Command::new("tmux")
        .args(["set-option", "-t", session, "remain-on-exit", "off"])
        .status();

    // Set environment variables
    set_environment(session, "DAEDALOS_AGENT_NAME", agent_name)?;
    set_environment(session, "DAEDALOS_AGENT_SESSION", session)?;
    set_environment(session, "DAEDALOS_DATA_DIR", data_dir)?;
    set_environment(
        session,
        "DAEDALOS_MESSAGES_DIR",
        &format!("{}/messages", data_dir),
    )?;
    set_environment(
        session,
        "DAEDALOS_SIGNALS_DIR",
        &format!("{}/signals", data_dir),
    )?;
    set_environment(
        session,
        "DAEDALOS_SHARED_DIR",
        &format!("{}/shared", data_dir),
    )?;

    // Send command if provided
    if !command.is_empty() {
        let env_exports = format!(
            "export DAEDALOS_AGENT_NAME='{}' DAEDALOS_AGENT_SESSION='{}' DAEDALOS_DATA_DIR='{}' && ",
            agent_name, session, data_dir
        );
        let cmd_str = format!("{}{}", env_exports, command.join(" "));
        send_keys(session, &cmd_str)?;
        send_keys(session, "Enter")?;
    }

    Ok(())
}

/// Set an environment variable in a tmux session
pub fn set_environment(session: &str, name: &str, value: &str) -> Result<()> {
    Command::new("tmux")
        .args(["set-environment", "-t", session, name, value])
        .status()
        .context("Failed to set tmux environment")?;
    Ok(())
}

/// Kill a tmux session
pub fn kill_session(session: &str, force: bool) -> Result<()> {
    if !session_exists(session) {
        return Ok(());
    }

    if !force {
        // Try graceful shutdown first
        let _ = Command::new("tmux")
            .args(["send-keys", "-t", session, "C-c"])
            .status();
        std::thread::sleep(std::time::Duration::from_millis(500));
    }

    Command::new("tmux")
        .args(["kill-session", "-t", session])
        .status()
        .context("Failed to kill tmux session")?;

    Ok(())
}

/// Focus (attach/switch to) a tmux session
pub fn focus_session(session: &str) -> Result<()> {
    if !session_exists(session) {
        anyhow::bail!("Session does not exist: {}", session);
    }

    // Check if we're already in tmux
    if std::env::var("TMUX").is_ok() {
        // Switch client
        let status = Command::new("tmux")
            .args(["switch-client", "-t", session])
            .status()
            .context("Failed to switch tmux client")?;

        if !status.success() {
            anyhow::bail!("Failed to switch to session: {}", session);
        }
    } else {
        // Attach
        let status = Command::new("tmux")
            .args(["attach-session", "-t", session])
            .status()
            .context("Failed to attach to tmux session")?;

        if !status.success() {
            anyhow::bail!("Failed to attach to session: {}", session);
        }
    }

    Ok(())
}

/// Send keys to a tmux session
pub fn send_keys(session: &str, keys: &str) -> Result<()> {
    if !session_exists(session) {
        anyhow::bail!("Session does not exist: {}", session);
    }

    Command::new("tmux")
        .args(["send-keys", "-t", session, keys])
        .status()
        .context("Failed to send keys to tmux session")?;

    Ok(())
}

/// Get pane content from a tmux session
pub fn get_pane_content(session: &str, lines: u32) -> Result<String> {
    if !session_exists(session) {
        anyhow::bail!("Session does not exist: {}", session);
    }

    let output = Command::new("tmux")
        .args([
            "capture-pane",
            "-t",
            session,
            "-p",
            "-S",
            &format!("-{}", lines),
        ])
        .output()
        .context("Failed to capture tmux pane")?;

    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

/// Get the PID of the main process in the tmux pane
pub fn get_pane_pid(session: &str) -> Result<Option<u32>> {
    if !session_exists(session) {
        return Ok(None);
    }

    let output = Command::new("tmux")
        .args(["display-message", "-t", session, "-p", "#{pane_pid}"])
        .output()
        .context("Failed to get tmux pane PID")?;

    let pid_str = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if pid_str.is_empty() {
        return Ok(None);
    }

    let pane_pid: u32 = pid_str.parse().context("Failed to parse pane PID")?;

    // Try to get child process (the actual command running)
    if let Ok(child_output) = Command::new("pgrep")
        .args(["-P", &pane_pid.to_string()])
        .output()
    {
        let child_str = String::from_utf8_lossy(&child_output.stdout);
        if let Some(first_line) = child_str.lines().next() {
            if let Ok(child_pid) = first_line.trim().parse::<u32>() {
                return Ok(Some(child_pid));
            }
        }
    }

    Ok(Some(pane_pid))
}

/// List all agent tmux sessions
#[allow(dead_code)]
pub fn list_agent_sessions() -> Result<Vec<String>> {
    let output = Command::new("tmux")
        .args(["list-sessions", "-F", "#{session_name}"])
        .output()
        .context("Failed to list tmux sessions")?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    let sessions: Vec<String> = stdout
        .lines()
        .filter(|s| s.starts_with(TMUX_SESSION_PREFIX))
        .map(|s| s.strip_prefix(TMUX_SESSION_PREFIX).unwrap_or(s).to_string())
        .collect();

    Ok(sessions)
}

/// Pause process in tmux pane (SIGSTOP)
pub fn pause_process(session: &str) -> Result<()> {
    if let Some(pid) = get_pane_pid(session)? {
        Command::new("kill")
            .args(["-STOP", &pid.to_string()])
            .status()
            .context("Failed to pause process")?;
    }
    Ok(())
}

/// Resume process in tmux pane (SIGCONT)
pub fn resume_process(session: &str) -> Result<()> {
    if let Some(pid) = get_pane_pid(session)? {
        Command::new("kill")
            .args(["-CONT", &pid.to_string()])
            .status()
            .context("Failed to resume process")?;
    }
    Ok(())
}
