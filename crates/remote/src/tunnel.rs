//! SSH tunnel management
//!
//! Create and manage SSH tunnels for port forwarding.

use crate::host::Host;
use anyhow::{Context, Result};
use std::process::{Child, Command, ExitStatus, Stdio};

/// Tunnel direction
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TunnelDirection {
    /// Local port forwards to remote (local:port -> remote:port)
    Forward,
    /// Remote port forwards to local (remote:port -> local:port)
    Reverse,
}

/// Configuration for an SSH tunnel
#[derive(Debug, Clone)]
pub struct TunnelConfig {
    /// Local port to bind
    pub local_port: u16,

    /// Remote port to connect to
    pub remote_port: u16,

    /// Tunnel direction
    pub direction: TunnelDirection,

    /// Local bind address (default: localhost)
    pub local_addr: String,

    /// Remote target address (default: localhost)
    pub remote_addr: String,

    /// Run in background
    pub background: bool,
}

impl TunnelConfig {
    /// Create a new forward tunnel configuration
    pub fn forward(local_port: u16, remote_port: u16) -> Self {
        Self {
            local_port,
            remote_port,
            direction: TunnelDirection::Forward,
            local_addr: "localhost".to_string(),
            remote_addr: "localhost".to_string(),
            background: false,
        }
    }

    /// Create a new reverse tunnel configuration
    pub fn reverse(local_port: u16, remote_port: u16) -> Self {
        Self {
            local_port,
            remote_port,
            direction: TunnelDirection::Reverse,
            local_addr: "localhost".to_string(),
            remote_addr: "localhost".to_string(),
            background: false,
        }
    }

    /// Set local bind address
    pub fn local_addr(mut self, addr: &str) -> Self {
        self.local_addr = addr.to_string();
        self
    }

    /// Set remote target address
    pub fn remote_addr(mut self, addr: &str) -> Self {
        self.remote_addr = addr.to_string();
        self
    }

    /// Run in background
    pub fn background(mut self, enabled: bool) -> Self {
        self.background = enabled;
        self
    }

    /// Get the port forwarding spec
    fn forwarding_spec(&self) -> String {
        match self.direction {
            TunnelDirection::Forward => {
                // -L [bind_address:]port:host:hostport
                format!(
                    "{}:{}:{}:{}",
                    self.local_addr, self.local_port, self.remote_addr, self.remote_port
                )
            }
            TunnelDirection::Reverse => {
                // -R [bind_address:]port:host:hostport
                format!(
                    "{}:{}:{}:{}",
                    self.remote_addr, self.remote_port, self.local_addr, self.local_port
                )
            }
        }
    }
}

/// SSH tunnel manager
pub struct Tunnel {
    host: Host,
    config: TunnelConfig,
}

impl Tunnel {
    /// Create a new tunnel
    pub fn new(host: Host, config: TunnelConfig) -> Self {
        Self { host, config }
    }

    /// Start the tunnel (blocking unless background is set)
    pub fn start(&self) -> Result<ExitStatus> {
        let mut cmd = self.build_command();

        cmd.stdin(Stdio::inherit())
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit());

        let status = cmd.status()
            .with_context(|| format!("Failed to create tunnel to {}", self.host.name))?;

        Ok(status)
    }

    /// Start the tunnel in background and return the child process
    pub fn start_background(&self) -> Result<Child> {
        let mut cmd = self.build_command();

        // Override to ensure background operation
        cmd.arg("-f").arg("-N");

        let child = cmd.spawn()
            .with_context(|| format!("Failed to start background tunnel to {}", self.host.name))?;

        Ok(child)
    }

    /// Build the SSH command for tunneling
    fn build_command(&self) -> Command {
        let mut cmd = Command::new("ssh");

        // Add identity file if specified
        if let Some(ref key) = self.host.key {
            let expanded_key = Self::expand_tilde(key);
            cmd.arg("-i").arg(&expanded_key);
        }

        // Add port if not default
        if self.host.port != 22 {
            cmd.arg("-p").arg(self.host.port.to_string());
        }

        // Background mode options
        if self.config.background {
            cmd.arg("-f");  // Go to background
            cmd.arg("-N");  // Don't execute remote command
        }

        // Add forwarding option
        match self.config.direction {
            TunnelDirection::Forward => {
                cmd.arg("-L").arg(self.config.forwarding_spec());
            }
            TunnelDirection::Reverse => {
                cmd.arg("-R").arg(self.config.forwarding_spec());
            }
        }

        // Add connection string
        cmd.arg(self.host.connection_string());

        cmd
    }

    /// Expand tilde in path
    fn expand_tilde(path: &str) -> String {
        if path.starts_with('~') {
            if let Some(home) = dirs::home_dir() {
                if path == "~" {
                    return home.to_string_lossy().to_string();
                } else if path.starts_with("~/") {
                    return home.join(&path[2..]).to_string_lossy().to_string();
                }
            }
        }
        path.to_string()
    }

    /// Get description of the tunnel
    pub fn description(&self) -> String {
        match self.config.direction {
            TunnelDirection::Forward => {
                format!(
                    "local:{} -> {}:{}",
                    self.config.local_port,
                    self.host.name,
                    self.config.remote_port
                )
            }
            TunnelDirection::Reverse => {
                format!(
                    "{}:{} -> local:{}",
                    self.host.name,
                    self.config.remote_port,
                    self.config.local_port
                )
            }
        }
    }

    /// Get the underlying host
    pub fn host(&self) -> &Host {
        &self.host
    }

    /// Get the tunnel config
    pub fn config(&self) -> &TunnelConfig {
        &self.config
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_forward_tunnel_config() {
        let config = TunnelConfig::forward(3000, 8080);

        assert_eq!(config.local_port, 3000);
        assert_eq!(config.remote_port, 8080);
        assert_eq!(config.direction, TunnelDirection::Forward);
        assert_eq!(config.forwarding_spec(), "localhost:3000:localhost:8080");
    }

    #[test]
    fn test_reverse_tunnel_config() {
        let config = TunnelConfig::reverse(9000, 9000);

        assert_eq!(config.direction, TunnelDirection::Reverse);
        assert_eq!(config.forwarding_spec(), "localhost:9000:localhost:9000");
    }

    #[test]
    fn test_tunnel_description() {
        let host = Host::new("prod", "192.168.1.100", "admin");
        let config = TunnelConfig::forward(3000, 8080);
        let tunnel = Tunnel::new(host, config);

        assert_eq!(tunnel.description(), "local:3000 -> prod:8080");
    }
}
