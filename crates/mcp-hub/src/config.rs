//! Configuration management for MCP Hub

#![allow(dead_code)]

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

/// Default paths
fn config_dir() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("~/.config"))
        .join("daedalos")
        .join("mcp-hub")
}

fn data_dir() -> PathBuf {
    dirs::data_dir()
        .unwrap_or_else(|| PathBuf::from("~/.local/share"))
        .join("daedalos")
        .join("mcp-hub")
}

/// Get the socket path for the MCP Hub daemon
pub fn get_socket_path() -> PathBuf {
    // Check environment variable first
    if let Ok(path) = std::env::var("MCPHUB_SOCKET") {
        return PathBuf::from(path);
    }

    // Try /run/daedalos first (NixOS/systemd style)
    let system_socket = PathBuf::from("/run/daedalos/mcp-hub.sock");
    if system_socket.parent().map(|p| p.exists()).unwrap_or(false) {
        return system_socket;
    }

    // Fall back to user-local socket
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("~"))
        .join(".local")
        .join("run")
        .join("daedalos")
        .join("mcp-hub.sock")
}

/// MCP Hub configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    /// Servers to auto-start when daemon starts
    #[serde(default = "default_auto_start")]
    pub auto_start_servers: Vec<String>,

    /// Maximum number of concurrent servers
    #[serde(default = "default_max_servers")]
    pub max_servers: usize,

    /// Request timeout in seconds
    #[serde(default = "default_request_timeout")]
    pub request_timeout: u64,

    /// Server-specific configurations
    #[serde(default)]
    pub servers: HashMap<String, ServerConfig>,
}

fn default_auto_start() -> Vec<String> {
    vec!["filesystem".to_string()]
}

fn default_max_servers() -> usize {
    10
}

fn default_request_timeout() -> u64 {
    30
}

impl Default for Config {
    fn default() -> Self {
        Self {
            auto_start_servers: default_auto_start(),
            max_servers: default_max_servers(),
            request_timeout: default_request_timeout(),
            servers: HashMap::new(),
        }
    }
}

/// Server-specific configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerConfig {
    /// Additional environment variables
    #[serde(default)]
    pub env: HashMap<String, String>,

    /// Additional arguments
    #[serde(default)]
    pub args: Vec<String>,

    /// Whether this server is enabled
    #[serde(default = "default_enabled")]
    pub enabled: bool,
}

fn default_enabled() -> bool {
    true
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            env: HashMap::new(),
            args: Vec::new(),
            enabled: true,
        }
    }
}

/// Load configuration from file
pub fn load_config() -> Result<Config> {
    let config_path = config_dir().join("config.yaml");

    if !config_path.exists() {
        return Ok(Config::default());
    }

    let content = fs::read_to_string(&config_path)
        .with_context(|| format!("Failed to read config from {}", config_path.display()))?;

    // Try YAML first, then JSON
    if let Ok(config) = serde_yaml_parse(&content) {
        return Ok(config);
    }

    serde_json::from_str(&content).context("Failed to parse config as YAML or JSON")
}

/// Simple YAML parser (subset that handles our config format)
fn serde_yaml_parse(content: &str) -> Result<Config> {
    // For now, we'll use a simple approach: try to parse as JSON
    // In a real implementation, we'd use the serde_yaml crate
    // But to keep dependencies minimal, we support a simple YAML subset

    let mut config = Config::default();
    let mut current_section: Option<&str> = None;

    for line in content.lines() {
        let line = line.trim();

        // Skip comments and empty lines
        if line.is_empty() || line.starts_with('#') {
            continue;
        }

        // Handle section headers
        if !line.starts_with('-') && line.ends_with(':') && !line.contains(' ') {
            current_section = Some(line.trim_end_matches(':'));
            continue;
        }

        // Handle list items
        if line.starts_with("- ") {
            if current_section == Some("auto_start_servers") {
                config.auto_start_servers.push(line[2..].trim().to_string());
            }
            continue;
        }

        // Handle key-value pairs
        if let Some((key, value)) = line.split_once(':') {
            let key = key.trim();
            let value = value.trim();

            match key {
                "max_servers" => {
                    if let Ok(n) = value.parse() {
                        config.max_servers = n;
                    }
                }
                "request_timeout" => {
                    if let Ok(n) = value.parse() {
                        config.request_timeout = n;
                    }
                }
                _ => {}
            }
        }
    }

    Ok(config)
}

/// Save configuration to file
pub fn save_config(config: &Config) -> Result<()> {
    let config_path = config_dir().join("config.yaml");

    // Ensure directory exists
    if let Some(parent) = config_path.parent() {
        fs::create_dir_all(parent)?;
    }

    // Write as YAML-like format
    let mut content = String::new();
    content.push_str("# MCP Hub Configuration\n\n");

    content.push_str(&format!("max_servers: {}\n", config.max_servers));
    content.push_str(&format!("request_timeout: {}\n", config.request_timeout));

    if !config.auto_start_servers.is_empty() {
        content.push_str("\nauto_start_servers:\n");
        for server in &config.auto_start_servers {
            content.push_str(&format!("  - {}\n", server));
        }
    }

    fs::write(&config_path, content)?;
    Ok(())
}

/// Get the configuration file path
pub fn get_config_path() -> PathBuf {
    config_dir().join("config.yaml")
}

/// Get the data directory path
pub fn get_data_dir() -> PathBuf {
    data_dir()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = Config::default();
        assert_eq!(config.max_servers, 10);
        assert_eq!(config.request_timeout, 30);
        assert!(config.auto_start_servers.contains(&"filesystem".to_string()));
    }

    #[test]
    fn test_socket_path() {
        let path = get_socket_path();
        assert!(path.to_string_lossy().contains("mcp-hub.sock"));
    }

    #[test]
    fn test_yaml_parse() {
        let yaml = r#"
max_servers: 5
request_timeout: 60

auto_start_servers:
  - filesystem
  - github
"#;
        let config = serde_yaml_parse(yaml).unwrap();
        assert_eq!(config.max_servers, 5);
        assert_eq!(config.request_timeout, 60);
        assert!(config.auto_start_servers.contains(&"github".to_string()));
    }
}
