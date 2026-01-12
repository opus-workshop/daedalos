//! Sandbox management
//!
//! Core data structures and management for sandbox environments.

use anyhow::{bail, Context, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

use crate::backend;

/// Sandbox copy backend
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Backend {
    /// Btrfs copy-on-write snapshots (instant, space-efficient)
    Btrfs,
    /// OverlayFS mount (fast, requires fuse-overlayfs or root)
    Overlay,
    /// Rsync copy (universal fallback, slower)
    Rsync,
}

/// Sandbox metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Sandbox {
    /// Sandbox name
    pub name: String,
    /// Source directory that was sandboxed
    pub source: PathBuf,
    /// Creation timestamp
    pub created: DateTime<Utc>,
    /// Backend used for this sandbox
    pub backend: Backend,
    /// Network isolation enabled
    #[serde(default)]
    pub isolated_network: bool,
    /// Path to the sandbox directory
    #[serde(skip)]
    pub path: PathBuf,
}

impl Sandbox {
    /// Get the path to the sandbox working directory
    pub fn work_path(&self) -> PathBuf {
        self.path.join("work")
    }

    /// Load sandbox from directory
    fn load(path: &Path) -> Result<Self> {
        let metadata_path = path.join("metadata.json");

        let mut sandbox: Sandbox = if metadata_path.exists() {
            // New JSON format
            let content = fs::read_to_string(&metadata_path)?;
            serde_json::from_str(&content)?
        } else {
            // Legacy format (separate files)
            let name = path
                .file_name()
                .context("Invalid sandbox path")?
                .to_string_lossy()
                .to_string();

            let source = fs::read_to_string(path.join("source"))
                .unwrap_or_default()
                .trim()
                .into();

            let created_str = fs::read_to_string(path.join("created")).unwrap_or_default();
            let created = DateTime::parse_from_rfc3339(created_str.trim())
                .map(|dt| dt.with_timezone(&Utc))
                .unwrap_or_else(|_| Utc::now());

            let backend_str = fs::read_to_string(path.join("backend"))
                .unwrap_or_else(|_| "rsync".to_string());
            let backend = match backend_str.trim() {
                "btrfs" => Backend::Btrfs,
                "overlay" => Backend::Overlay,
                _ => Backend::Rsync,
            };

            let isolated_network = fs::read_to_string(path.join("isolated_network"))
                .map(|s| s.trim() == "1")
                .unwrap_or(false);

            Sandbox {
                name,
                source,
                created,
                backend,
                isolated_network,
                path: PathBuf::new(),
            }
        };

        sandbox.path = path.to_path_buf();
        Ok(sandbox)
    }

    /// Save sandbox metadata
    fn save(&self) -> Result<()> {
        let metadata_path = self.path.join("metadata.json");
        let content = serde_json::to_string_pretty(&self)?;
        fs::write(metadata_path, content)?;
        Ok(())
    }
}

/// Sandbox manager
pub struct SandboxManager {
    /// Root directory for all sandboxes
    root: PathBuf,
}

impl SandboxManager {
    /// Create a new sandbox manager
    pub fn new(root: &Path) -> Result<Self> {
        fs::create_dir_all(root)?;
        Ok(Self {
            root: root.to_path_buf(),
        })
    }

    /// Get path for a sandbox
    fn sandbox_path(&self, name: &str) -> PathBuf {
        self.root.join(name)
    }

    /// Check if a sandbox exists
    pub fn exists(&self, name: &str) -> bool {
        let path = self.sandbox_path(name);
        path.join("work").exists() || path.join("metadata.json").exists()
    }

    /// Create a new sandbox
    pub fn create(&self, name: &str, source: &Path, backend_type: Backend) -> Result<Sandbox> {
        let sandbox_path = self.sandbox_path(name);

        if sandbox_path.exists() {
            bail!("Sandbox '{}' already exists", name);
        }

        fs::create_dir_all(&sandbox_path)?;

        let work_path = sandbox_path.join("work");

        // Create the sandbox copy
        backend::create_sandbox(backend_type, source, &work_path)
            .with_context(|| format!("Failed to create sandbox with {:?} backend", backend_type))?;

        // Create and save metadata
        let sandbox = Sandbox {
            name: name.to_string(),
            source: source.to_path_buf(),
            created: Utc::now(),
            backend: backend_type,
            isolated_network: false,
            path: sandbox_path.clone(),
        };

        sandbox.save()?;

        // Also write legacy format for compatibility
        fs::write(sandbox_path.join("source"), source.display().to_string())?;
        fs::write(
            sandbox_path.join("created"),
            sandbox.created.to_rfc3339(),
        )?;
        fs::write(
            sandbox_path.join("backend"),
            format!("{:?}", backend_type).to_lowercase(),
        )?;

        Ok(sandbox)
    }

    /// Get a sandbox by name
    pub fn get(&self, name: &str) -> Result<Sandbox> {
        let path = self.sandbox_path(name);

        if !path.exists() {
            bail!("Sandbox '{}' not found", name);
        }

        Sandbox::load(&path)
    }

    /// List all sandboxes
    pub fn list(&self) -> Result<Vec<Sandbox>> {
        let mut sandboxes = Vec::new();

        if !self.root.exists() {
            return Ok(sandboxes);
        }

        for entry in fs::read_dir(&self.root)? {
            let entry = entry?;
            let path = entry.path();

            if !path.is_dir() {
                continue;
            }

            // Check if this is a valid sandbox
            if path.join("work").exists() || path.join("metadata.json").exists() {
                if let Ok(sandbox) = Sandbox::load(&path) {
                    sandboxes.push(sandbox);
                }
            }
        }

        // Sort by creation time, newest first
        sandboxes.sort_by(|a, b| b.created.cmp(&a.created));

        Ok(sandboxes)
    }

    /// Discard (delete) a sandbox
    pub fn discard(&self, name: &str) -> Result<()> {
        let sandbox = self.get(name)?;
        let sandbox_path = self.sandbox_path(name);

        backend::cleanup_sandbox(sandbox.backend, &sandbox_path)?;

        // Make sure everything is cleaned up
        if sandbox_path.exists() {
            fs::remove_dir_all(&sandbox_path)?;
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_sandbox_manager() -> Result<()> {
        let root = tempdir()?;
        let source = tempdir()?;

        // Create a test file
        fs::write(source.path().join("test.txt"), "hello")?;

        let manager = SandboxManager::new(root.path())?;

        // Create sandbox
        let sandbox = manager.create("test-sandbox", source.path(), Backend::Rsync)?;
        assert_eq!(sandbox.name, "test-sandbox");
        assert!(manager.exists("test-sandbox"));

        // Verify file was copied
        assert!(sandbox.work_path().join("test.txt").exists());

        // List sandboxes
        let list = manager.list()?;
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].name, "test-sandbox");

        // Get sandbox
        let retrieved = manager.get("test-sandbox")?;
        assert_eq!(retrieved.name, "test-sandbox");

        // Discard
        manager.discard("test-sandbox")?;
        assert!(!manager.exists("test-sandbox"));

        Ok(())
    }

    #[test]
    fn test_backend_serialization() {
        assert_eq!(
            serde_json::to_string(&Backend::Rsync).unwrap(),
            "\"rsync\""
        );
        assert_eq!(
            serde_json::to_string(&Backend::Btrfs).unwrap(),
            "\"btrfs\""
        );
        assert_eq!(
            serde_json::to_string(&Backend::Overlay).unwrap(),
            "\"overlay\""
        );
    }
}
