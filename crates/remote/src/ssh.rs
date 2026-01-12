//! SSH connection management
//!
//! Build and execute SSH commands for remote connections.

use crate::host::Host;
use anyhow::{Context, Result};
use std::process::{Command, ExitStatus, Stdio};

/// SSH connection handler
pub struct SshConnection {
    host: Host,
}

impl SshConnection {
    /// Create a new SSH connection for a host
    pub fn new(host: Host) -> Self {
        Self { host }
    }

    /// Build the base SSH command with host configuration
    fn build_base_command(&self) -> Command {
        let mut cmd = Command::new("ssh");

        // Add identity file if specified
        if let Some(ref key) = self.host.key {
            let expanded_key = shellexpand::tilde(key);
            cmd.arg("-i").arg(expanded_key.as_ref());
        }

        // Add port if not default
        if self.host.port != 22 {
            cmd.arg("-p").arg(self.host.port.to_string());
        }

        // Add connection string
        cmd.arg(self.host.connection_string());

        cmd
    }

    /// Connect to the remote host interactively
    pub fn connect(&self) -> Result<ExitStatus> {
        let mut cmd = self.build_base_command();

        cmd.stdin(Stdio::inherit())
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit());

        let status = cmd.status()
            .with_context(|| format!("Failed to SSH to {}", self.host.name))?;

        Ok(status)
    }

    /// Execute a command on the remote host
    pub fn exec(&self, remote_command: &str) -> Result<ExitStatus> {
        let mut cmd = self.build_base_command();

        cmd.arg(remote_command);

        cmd.stdin(Stdio::inherit())
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit());

        let status = cmd.status()
            .with_context(|| format!("Failed to execute command on {}", self.host.name))?;

        Ok(status)
    }

    /// Execute a command and capture output
    pub fn exec_capture(&self, remote_command: &str) -> Result<std::process::Output> {
        let mut cmd = self.build_base_command();

        cmd.arg(remote_command);

        let output = cmd.output()
            .with_context(|| format!("Failed to execute command on {}", self.host.name))?;

        Ok(output)
    }

    /// Start a dev session (tmux on remote)
    pub fn dev_session(&self) -> Result<ExitStatus> {
        let mut cmd = self.build_base_command();

        // Force TTY allocation
        cmd.arg("-t");

        // Get the remote path or default to home
        let remote_path = self.host.path.as_deref().unwrap_or("~");

        // Start or attach to tmux session
        let tmux_cmd = format!(
            "cd {} && (tmux attach -t dev 2>/dev/null || tmux new-session -s dev)",
            remote_path
        );

        cmd.arg(tmux_cmd);

        cmd.stdin(Stdio::inherit())
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit());

        let status = cmd.status()
            .with_context(|| format!("Failed to start dev session on {}", self.host.name))?;

        Ok(status)
    }

    /// Copy SSH key to remote host
    pub fn copy_id(&self) -> Result<ExitStatus> {
        let mut cmd = Command::new("ssh-copy-id");

        if self.host.port != 22 {
            cmd.arg("-p").arg(self.host.port.to_string());
        }

        cmd.arg(self.host.connection_string());

        cmd.stdin(Stdio::inherit())
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit());

        let status = cmd.status()
            .context("Failed to run ssh-copy-id")?;

        Ok(status)
    }

    /// Check if host is reachable (quick TCP check)
    pub fn check_reachable(&self) -> bool {
        use std::net::{TcpStream, ToSocketAddrs};
        use std::time::Duration;

        let addr = format!("{}:{}", self.host.host, self.host.port);

        if let Ok(mut addrs) = addr.to_socket_addrs() {
            if let Some(addr) = addrs.next() {
                return TcpStream::connect_timeout(&addr, Duration::from_secs(2)).is_ok();
            }
        }

        false
    }

    /// Get the underlying host
    pub fn host(&self) -> &Host {
        &self.host
    }
}

// Simple tilde expansion module
mod shellexpand {
    use std::borrow::Cow;

    pub fn tilde(path: &str) -> Cow<'_, str> {
        if path.starts_with('~') {
            if let Some(home) = dirs::home_dir() {
                let expanded = if path == "~" {
                    home
                } else if path.starts_with("~/") {
                    home.join(&path[2..])
                } else {
                    return Cow::Borrowed(path);
                };
                return Cow::Owned(expanded.to_string_lossy().into_owned());
            }
        }
        Cow::Borrowed(path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tilde_expansion() {
        // Test basic expansion
        let result = shellexpand::tilde("~/test");
        assert!(!result.starts_with('~') || dirs::home_dir().is_none());
    }
}
