//! daedalos-notify - Desktop notifications for Daedalos
//!
//! Provides a unified cross-platform notification system for long-running tasks.
//! Works on macOS (osascript/terminal-notifier), Linux (notify-send), and WSL.

mod backend;
mod history;

pub use backend::{Backend, Notification, Urgency};
pub use history::{NotificationHistory, NotificationRecord};

use anyhow::Result;
use daedalos_core::Paths;
use std::path::PathBuf;
use std::time::Instant;

/// Default notification title
pub const DEFAULT_TITLE: &str = "Daedalos";

/// Get the data directory for notifications
pub fn data_dir() -> PathBuf {
    Paths::new().data.join("notify")
}

/// Send a notification with the detected backend
pub fn send(notification: &Notification) -> Result<()> {
    let backend = Backend::detect();
    backend.send(notification)?;

    // Log to history
    let history = NotificationHistory::new()?;
    history.log(notification)?;

    Ok(())
}

/// Send a simple notification
pub fn simple(message: &str) -> Result<()> {
    send(&Notification::new(message))
}

/// Send a success notification
pub fn success(message: &str) -> Result<()> {
    send(&Notification {
        title: "Success".to_string(),
        message: message.to_string(),
        urgency: Urgency::Normal,
        sound: true,
        ..Default::default()
    })
}

/// Send an error notification
pub fn error(message: &str) -> Result<()> {
    send(&Notification {
        title: "Error".to_string(),
        message: message.to_string(),
        urgency: Urgency::Critical,
        sound: true,
        ..Default::default()
    })
}

/// Send a warning notification
pub fn warn(message: &str) -> Result<()> {
    send(&Notification {
        title: "Warning".to_string(),
        message: message.to_string(),
        urgency: Urgency::Normal,
        sound: true,
        ..Default::default()
    })
}

/// Send a progress notification (no sound)
pub fn progress(message: &str) -> Result<()> {
    send(&Notification {
        title: "In Progress".to_string(),
        message: message.to_string(),
        urgency: Urgency::Low,
        sound: false,
        ..Default::default()
    })
}

/// Watch a command and notify on completion
pub async fn watch(command: &str) -> Result<i32> {
    use tokio::process::Command;

    let start = Instant::now();

    // Run the command
    let output = if cfg!(target_os = "windows") {
        Command::new("cmd")
            .args(["/C", command])
            .status()
            .await?
    } else {
        Command::new("sh")
            .args(["-c", command])
            .status()
            .await?
    };

    let duration = start.elapsed();
    let duration_str = daedalos_core::format::duration(duration.as_secs_f64());
    let exit_code = output.code().unwrap_or(-1);

    if output.success() {
        send(&Notification {
            title: "Complete".to_string(),
            message: format!("Command finished in {}", duration_str),
            urgency: Urgency::Normal,
            sound: true,
            ..Default::default()
        })?;
    } else {
        send(&Notification {
            title: "Failed".to_string(),
            message: format!("Command failed after {} (exit: {})", duration_str, exit_code),
            urgency: Urgency::Critical,
            sound: true,
            ..Default::default()
        })?;
    }

    Ok(exit_code)
}
