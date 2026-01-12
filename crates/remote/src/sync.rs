//! File synchronization with rsync
//!
//! Sync files to and from remote hosts using rsync.

use crate::host::Host;
use anyhow::{Context, Result};
use std::path::Path;
use std::process::{Command, ExitStatus, Stdio};

/// Direction of file sync
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SyncDirection {
    /// Local to remote
    ToRemote,
    /// Remote to local
    FromRemote,
}

/// Options for file synchronization
#[derive(Debug, Clone)]
pub struct SyncOptions {
    /// Sync direction
    pub direction: SyncDirection,

    /// Patterns to exclude
    pub excludes: Vec<String>,

    /// Dry run (show what would be done)
    pub dry_run: bool,

    /// Delete files on destination not in source
    pub delete: bool,

    /// Show progress
    pub progress: bool,
}

impl Default for SyncOptions {
    fn default() -> Self {
        Self {
            direction: SyncDirection::ToRemote,
            excludes: Vec::new(),
            dry_run: false,
            delete: false,
            progress: true,
        }
    }
}

impl SyncOptions {
    /// Create options for syncing to remote
    pub fn to_remote() -> Self {
        Self {
            direction: SyncDirection::ToRemote,
            ..Default::default()
        }
    }

    /// Create options for syncing from remote
    pub fn from_remote() -> Self {
        Self {
            direction: SyncDirection::FromRemote,
            ..Default::default()
        }
    }

    /// Add an exclude pattern
    pub fn exclude(mut self, pattern: &str) -> Self {
        self.excludes.push(pattern.to_string());
        self
    }

    /// Enable dry run
    pub fn dry_run(mut self, enabled: bool) -> Self {
        self.dry_run = enabled;
        self
    }

    /// Enable delete
    pub fn delete(mut self, enabled: bool) -> Self {
        self.delete = enabled;
        self
    }
}

/// File synchronizer using rsync
pub struct Syncer {
    host: Host,
}

impl Syncer {
    /// Create a new syncer for a host
    pub fn new(host: Host) -> Self {
        Self { host }
    }

    /// Sync files between local and remote
    pub fn sync(
        &self,
        local_path: &Path,
        remote_path: &str,
        options: &SyncOptions,
    ) -> Result<ExitStatus> {
        let mut cmd = Command::new("rsync");

        // Base options: archive, verbose, compress
        cmd.arg("-avz");

        // Progress
        if options.progress {
            cmd.arg("--progress");
        }

        // Dry run
        if options.dry_run {
            cmd.arg("--dry-run");
        }

        // Delete
        if options.delete {
            cmd.arg("--delete");
        }

        // Excludes
        for pattern in &options.excludes {
            cmd.arg("--exclude").arg(pattern);
        }

        // SSH options for custom port/key
        let ssh_opts = self.build_ssh_opts();
        if !ssh_opts.is_empty() {
            cmd.arg("-e").arg(format!("ssh {}", ssh_opts));
        }

        // Source and destination based on direction
        let remote_spec = format!(
            "{}:{}",
            self.host.connection_string(),
            remote_path
        );

        match options.direction {
            SyncDirection::ToRemote => {
                // Ensure local path ends with / for directory contents
                let local_str = local_path.to_string_lossy();
                let local_arg = if local_path.is_dir() && !local_str.ends_with('/') {
                    format!("{}/", local_str)
                } else {
                    local_str.to_string()
                };
                cmd.arg(&local_arg).arg(&remote_spec);
            }
            SyncDirection::FromRemote => {
                cmd.arg(&remote_spec).arg(local_path);
            }
        }

        cmd.stdin(Stdio::inherit())
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit());

        let status = cmd.status()
            .with_context(|| format!("Failed to sync with {}", self.host.name))?;

        Ok(status)
    }

    /// Build SSH options string
    fn build_ssh_opts(&self) -> String {
        let mut opts = Vec::new();

        if let Some(ref key) = self.host.key {
            let expanded = Self::expand_tilde(key);
            opts.push(format!("-i {}", expanded));
        }

        if self.host.port != 22 {
            opts.push(format!("-p {}", self.host.port));
        }

        opts.join(" ")
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

    /// Get the underlying host
    pub fn host(&self) -> &Host {
        &self.host
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sync_options_default() {
        let opts = SyncOptions::default();
        assert_eq!(opts.direction, SyncDirection::ToRemote);
        assert!(!opts.dry_run);
        assert!(opts.progress);
    }

    #[test]
    fn test_sync_options_builder() {
        let opts = SyncOptions::from_remote()
            .exclude("*.log")
            .exclude("node_modules")
            .dry_run(true);

        assert_eq!(opts.direction, SyncDirection::FromRemote);
        assert!(opts.dry_run);
        assert_eq!(opts.excludes.len(), 2);
    }
}
