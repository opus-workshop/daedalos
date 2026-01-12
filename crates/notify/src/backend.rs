//! Notification backends for different platforms

use anyhow::{bail, Result};
use std::process::Command;

/// Notification urgency levels
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Urgency {
    Low,
    #[default]
    Normal,
    Critical,
}

impl Urgency {
    pub fn as_str(&self) -> &'static str {
        match self {
            Urgency::Low => "low",
            Urgency::Normal => "normal",
            Urgency::Critical => "critical",
        }
    }
}

/// A notification to display
#[derive(Debug, Clone, Default)]
pub struct Notification {
    /// Notification title
    pub title: String,
    /// Notification message/body
    pub message: String,
    /// Icon name or path (optional)
    pub icon: Option<String>,
    /// Urgency level
    pub urgency: Urgency,
    /// Auto-dismiss timeout in seconds (optional)
    pub timeout: Option<u32>,
    /// Whether to play a sound
    pub sound: bool,
    /// Command to run when notification is clicked (optional)
    pub action: Option<String>,
}

impl Notification {
    /// Create a new notification with a message
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            title: crate::DEFAULT_TITLE.to_string(),
            message: message.into(),
            sound: true,
            ..Default::default()
        }
    }

    /// Set the title
    pub fn with_title(mut self, title: impl Into<String>) -> Self {
        self.title = title.into();
        self
    }

    /// Set the icon
    pub fn with_icon(mut self, icon: impl Into<String>) -> Self {
        self.icon = Some(icon.into());
        self
    }

    /// Set the urgency
    pub fn with_urgency(mut self, urgency: Urgency) -> Self {
        self.urgency = urgency;
        self
    }

    /// Set the timeout
    pub fn with_timeout(mut self, seconds: u32) -> Self {
        self.timeout = Some(seconds);
        self
    }

    /// Disable sound
    pub fn silent(mut self) -> Self {
        self.sound = false;
        self
    }

    /// Set click action
    pub fn with_action(mut self, action: impl Into<String>) -> Self {
        self.action = Some(action.into());
        self
    }
}

/// Available notification backends
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Backend {
    /// macOS terminal-notifier
    TerminalNotifier,
    /// macOS osascript
    Osascript,
    /// Linux notify-send
    NotifySend,
    /// KDE kdialog
    Kdialog,
    /// WSL PowerShell
    Wsl,
    /// Fallback echo
    Echo,
}

impl Backend {
    /// Detect the best available backend for the current platform
    pub fn detect() -> Self {
        #[cfg(target_os = "macos")]
        {
            if Self::command_exists("terminal-notifier") {
                return Self::TerminalNotifier;
            }
            return Self::Osascript;
        }

        #[cfg(target_os = "linux")]
        {
            // Check if running in WSL
            if std::env::var("WSL_DISTRO_NAME").is_ok() {
                return Self::Wsl;
            }
            if Self::command_exists("notify-send") {
                return Self::NotifySend;
            }
            if Self::command_exists("kdialog") {
                return Self::Kdialog;
            }
            return Self::Echo;
        }

        #[cfg(not(any(target_os = "macos", target_os = "linux")))]
        {
            Self::Echo
        }
    }

    /// Check if a command exists
    fn command_exists(cmd: &str) -> bool {
        Command::new("which")
            .arg(cmd)
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
    }

    /// Get the name of this backend
    pub fn name(&self) -> &'static str {
        match self {
            Self::TerminalNotifier => "terminal-notifier",
            Self::Osascript => "osascript",
            Self::NotifySend => "notify-send",
            Self::Kdialog => "kdialog",
            Self::Wsl => "wsl",
            Self::Echo => "echo",
        }
    }

    /// Send a notification using this backend
    pub fn send(&self, notification: &Notification) -> Result<()> {
        match self {
            Self::TerminalNotifier => self.send_terminal_notifier(notification),
            Self::Osascript => self.send_osascript(notification),
            Self::NotifySend => self.send_notify_send(notification),
            Self::Kdialog => self.send_kdialog(notification),
            Self::Wsl => self.send_wsl(notification),
            Self::Echo => self.send_echo(notification),
        }
    }

    fn send_terminal_notifier(&self, notification: &Notification) -> Result<()> {
        let mut cmd = Command::new("terminal-notifier");
        cmd.args([
            "-title",
            &notification.title,
            "-message",
            &notification.message,
            "-group",
            "daedalos",
        ]);

        if let Some(icon) = &notification.icon {
            cmd.args(["-appIcon", icon]);
        }

        if notification.urgency == Urgency::Critical {
            cmd.args(["-sound", "Basso"]);
        } else if notification.sound {
            cmd.args(["-sound", "default"]);
        }

        if let Some(action) = &notification.action {
            cmd.args(["-execute", action]);
        }

        let status = cmd.status()?;
        if !status.success() {
            bail!("terminal-notifier failed with status: {}", status);
        }
        Ok(())
    }

    fn send_osascript(&self, notification: &Notification) -> Result<()> {
        // Escape quotes in the message and title
        let title = notification.title.replace('"', r#"\""#);
        let message = notification.message.replace('"', r#"\""#);

        let mut script = format!(r#"display notification "{}" with title "{}""#, message, title);

        if notification.sound {
            script.push_str(r#" sound name "default""#);
        }

        let status = Command::new("osascript")
            .args(["-e", &script])
            .status()?;

        if !status.success() {
            bail!("osascript failed with status: {}", status);
        }
        Ok(())
    }

    fn send_notify_send(&self, notification: &Notification) -> Result<()> {
        let mut cmd = Command::new("notify-send");
        cmd.args([&notification.title, &notification.message]);

        if let Some(icon) = &notification.icon {
            cmd.args(["--icon", icon]);
        }

        if notification.urgency == Urgency::Critical {
            cmd.args(["--urgency", "critical"]);
        }

        if let Some(timeout) = notification.timeout {
            cmd.args(["--expire-time", &(timeout * 1000).to_string()]);
        }

        let status = cmd.status()?;
        if !status.success() {
            bail!("notify-send failed with status: {}", status);
        }
        Ok(())
    }

    fn send_kdialog(&self, notification: &Notification) -> Result<()> {
        let timeout = notification.timeout.unwrap_or(5);

        let status = Command::new("kdialog")
            .args([
                "--passivepopup",
                &notification.message,
                &timeout.to_string(),
                "--title",
                &notification.title,
            ])
            .status()?;

        if !status.success() {
            bail!("kdialog failed with status: {}", status);
        }
        Ok(())
    }

    fn send_wsl(&self, notification: &Notification) -> Result<()> {
        // Escape single quotes for PowerShell
        let title = notification.title.replace('\'', "''");
        let message = notification.message.replace('\'', "''");

        let ps_script = format!(
            r#"[Windows.UI.Notifications.ToastNotificationManager, Windows.UI.Notifications, ContentType = WindowsRuntime] | Out-Null; $template = [Windows.UI.Notifications.ToastNotificationManager]::GetTemplateContent([Windows.UI.Notifications.ToastTemplateType]::ToastText02); $template.GetElementsByTagName('text')[0].AppendChild($template.CreateTextNode('{}')) | Out-Null; $template.GetElementsByTagName('text')[1].AppendChild($template.CreateTextNode('{}')) | Out-Null; [Windows.UI.Notifications.ToastNotificationManager]::CreateToastNotifier('Daedalos').Show([Windows.UI.Notifications.ToastNotification]::new($template))"#,
            title, message
        );

        let status = Command::new("powershell.exe")
            .args(["-Command", &ps_script])
            .status()?;

        // WSL PowerShell can fail for various reasons, fall back to echo
        if !status.success() {
            println!("[{}] {}", notification.title, notification.message);
        }
        Ok(())
    }

    fn send_echo(&self, notification: &Notification) -> Result<()> {
        println!("[{}] {}", notification.title, notification.message);
        Ok(())
    }
}
