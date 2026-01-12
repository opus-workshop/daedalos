//! Host configuration management
//!
//! Stores and retrieves remote host configurations.

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// A remote host configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Host {
    /// Friendly name for the host
    pub name: String,

    /// Hostname or IP address
    pub host: String,

    /// SSH username
    pub user: String,

    /// SSH port (default: 22)
    #[serde(default = "default_port")]
    pub port: u16,

    /// Path to SSH key (optional)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub key: Option<String>,

    /// Default remote working directory (optional)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,

    /// When this host was added
    #[serde(default = "Utc::now")]
    pub created: DateTime<Utc>,

    /// Last successful connection (optional)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_connected: Option<DateTime<Utc>>,
}

fn default_port() -> u16 {
    22
}

impl Host {
    /// Create a new host configuration
    pub fn new(name: &str, host: &str, user: &str) -> Self {
        Self {
            name: name.to_string(),
            host: host.to_string(),
            user: user.to_string(),
            port: 22,
            key: None,
            path: None,
            created: Utc::now(),
            last_connected: None,
        }
    }

    /// Set the SSH port
    pub fn with_port(mut self, port: u16) -> Self {
        self.port = port;
        self
    }

    /// Set the SSH key path
    pub fn with_key(mut self, key: &str) -> Self {
        self.key = Some(key.to_string());
        self
    }

    /// Set the default remote path
    pub fn with_path(mut self, path: &str) -> Self {
        self.path = Some(path.to_string());
        self
    }

    /// Get the SSH connection string (user@host)
    pub fn connection_string(&self) -> String {
        format!("{}@{}", self.user, self.host)
    }

    /// Get display address (user@host:port)
    pub fn display_address(&self) -> String {
        format!("{}@{}:{}", self.user, self.host, self.port)
    }
}

/// Store for host configurations
pub struct HostStore {
    /// Path to hosts.json file
    path: PathBuf,
}

impl HostStore {
    /// Create a new host store at the given config directory
    pub fn new(config_dir: &Path) -> Result<Self> {
        let path = config_dir.join("hosts.json");

        // Ensure config directory exists
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("Failed to create config directory: {}", parent.display()))?;
        }

        Ok(Self { path })
    }

    /// Load all hosts from the store
    pub fn load(&self) -> Result<Vec<Host>> {
        if !self.path.exists() {
            return Ok(Vec::new());
        }

        let content = std::fs::read_to_string(&self.path)
            .with_context(|| format!("Failed to read hosts file: {}", self.path.display()))?;

        let hosts: Vec<Host> = serde_json::from_str(&content)
            .with_context(|| "Failed to parse hosts file")?;

        Ok(hosts)
    }

    /// Save all hosts to the store
    pub fn save(&self, hosts: &[Host]) -> Result<()> {
        let content = serde_json::to_string_pretty(hosts)
            .context("Failed to serialize hosts")?;

        std::fs::write(&self.path, content)
            .with_context(|| format!("Failed to write hosts file: {}", self.path.display()))?;

        Ok(())
    }

    /// Get a host by name
    pub fn get(&self, name: &str) -> Result<Option<Host>> {
        let hosts = self.load()?;
        Ok(hosts.into_iter().find(|h| h.name == name))
    }

    /// Check if a host exists
    pub fn exists(&self, name: &str) -> Result<bool> {
        Ok(self.get(name)?.is_some())
    }

    /// Add or update a host
    pub fn add(&self, host: Host) -> Result<()> {
        let mut hosts = self.load()?;

        // Remove existing host with same name
        hosts.retain(|h| h.name != host.name);

        // Add new host
        hosts.push(host);

        self.save(&hosts)
    }

    /// Remove a host by name
    pub fn remove(&self, name: &str) -> Result<bool> {
        let mut hosts = self.load()?;
        let original_len = hosts.len();

        hosts.retain(|h| h.name != name);

        if hosts.len() < original_len {
            self.save(&hosts)?;
            Ok(true)
        } else {
            Ok(false)
        }
    }

    /// Update last connected time for a host
    pub fn update_last_connected(&self, name: &str) -> Result<()> {
        let mut hosts = self.load()?;

        if let Some(host) = hosts.iter_mut().find(|h| h.name == name) {
            host.last_connected = Some(Utc::now());
            self.save(&hosts)?;
        }

        Ok(())
    }

    /// List all host names
    pub fn list_names(&self) -> Result<Vec<String>> {
        let hosts = self.load()?;
        Ok(hosts.into_iter().map(|h| h.name).collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_host_creation() {
        let host = Host::new("prod", "192.168.1.100", "admin")
            .with_port(2222)
            .with_key("~/.ssh/id_rsa");

        assert_eq!(host.name, "prod");
        assert_eq!(host.host, "192.168.1.100");
        assert_eq!(host.user, "admin");
        assert_eq!(host.port, 2222);
        assert_eq!(host.key, Some("~/.ssh/id_rsa".to_string()));
    }

    #[test]
    fn test_connection_string() {
        let host = Host::new("test", "example.com", "user");
        assert_eq!(host.connection_string(), "user@example.com");
        assert_eq!(host.display_address(), "user@example.com:22");
    }

    #[test]
    fn test_host_store() -> Result<()> {
        let dir = tempdir()?;
        let store = HostStore::new(dir.path())?;

        // Initially empty
        assert!(store.load()?.is_empty());

        // Add a host
        let host = Host::new("test", "localhost", "user");
        store.add(host)?;

        // Should exist now
        assert!(store.exists("test")?);

        let loaded = store.get("test")?;
        assert!(loaded.is_some());
        assert_eq!(loaded.unwrap().host, "localhost");

        // Remove
        assert!(store.remove("test")?);
        assert!(!store.exists("test")?);

        Ok(())
    }
}
