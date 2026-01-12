//! Scratch environment storage and management
//!
//! Handles creating, listing, and destroying scratch environments.
//! Supports multiple backends: git worktree, btrfs snapshots, and plain copy.

use anyhow::{bail, Context, Result};
use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

/// Default expiration time in hours
const DEFAULT_EXPIRY_HOURS: i64 = 24;

/// Storage mode for scratch environments
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ScratchMode {
    /// Btrfs copy-on-write snapshot (fastest, most space efficient)
    Btrfs,
    /// Git worktree (shares .git, efficient for git repos)
    Git,
    /// Plain rsync copy (fallback, works everywhere)
    Copy,
}

impl ScratchMode {
    pub fn as_str(&self) -> &'static str {
        match self {
            ScratchMode::Btrfs => "btrfs",
            ScratchMode::Git => "git",
            ScratchMode::Copy => "copy",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "btrfs" => Some(ScratchMode::Btrfs),
            "git" => Some(ScratchMode::Git),
            "copy" => Some(ScratchMode::Copy),
            _ => None,
        }
    }
}

/// Metadata for a scratch environment
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScratchInfo {
    /// Name of the scratch environment
    pub name: String,
    /// Original source directory
    pub original: PathBuf,
    /// Path to the scratch directory
    pub path: PathBuf,
    /// Storage mode used
    pub mode: ScratchMode,
    /// Creation timestamp
    pub created: DateTime<Utc>,
    /// Expiration timestamp (optional)
    pub expires: Option<DateTime<Utc>>,
}

/// Metadata file format
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct MetaFile {
    scratches: HashMap<String, ScratchEntry>,
}

/// Entry in the metadata file
#[derive(Debug, Clone, Serialize, Deserialize)]
struct ScratchEntry {
    original: PathBuf,
    mode: ScratchMode,
    created: DateTime<Utc>,
    expires: Option<DateTime<Utc>>,
}

/// Scratch environment store
pub struct ScratchStore {
    /// Root directory for scratch environments
    root: PathBuf,
    /// Path to metadata file
    meta_path: PathBuf,
}

impl ScratchStore {
    /// Create a new scratch store
    pub fn new(root: &Path) -> Result<Self> {
        fs::create_dir_all(root)
            .with_context(|| format!("Failed to create scratch directory: {}", root.display()))?;

        let meta_path = root.join("meta.json");

        Ok(Self {
            root: root.to_path_buf(),
            meta_path,
        })
    }

    /// Get the path where a scratch will be stored
    fn scratch_path(&self, name: &str) -> PathBuf {
        self.root.join(name)
    }

    /// Read metadata file
    fn read_meta(&self) -> Result<MetaFile> {
        if !self.meta_path.exists() {
            return Ok(MetaFile::default());
        }

        let content = fs::read_to_string(&self.meta_path)
            .with_context(|| format!("Failed to read metadata: {}", self.meta_path.display()))?;

        serde_json::from_str(&content).context("Failed to parse metadata JSON")
    }

    /// Write metadata file
    fn write_meta(&self, meta: &MetaFile) -> Result<()> {
        let content =
            serde_json::to_string_pretty(meta).context("Failed to serialize metadata")?;

        fs::write(&self.meta_path, content)
            .with_context(|| format!("Failed to write metadata: {}", self.meta_path.display()))
    }

    /// Check if a scratch environment exists
    pub fn exists(&self, name: &str) -> bool {
        self.scratch_path(name).exists()
    }

    /// Detect the best mode for a source directory
    pub fn detect_mode(&self, source: &Path) -> ScratchMode {
        // Check if on Btrfs filesystem
        if Self::is_btrfs(source) {
            return ScratchMode::Btrfs;
        }

        // Check if it's a git repository
        if Self::is_git_repo(source) {
            return ScratchMode::Git;
        }

        // Fallback to copy
        ScratchMode::Copy
    }

    /// Check if a path is on a Btrfs filesystem
    fn is_btrfs(path: &Path) -> bool {
        // Try to run btrfs subvolume show
        Command::new("btrfs")
            .args(["subvolume", "show"])
            .arg(path)
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
    }

    /// Check if a path is inside a git repository
    fn is_git_repo(path: &Path) -> bool {
        Command::new("git")
            .args(["-C"])
            .arg(path)
            .args(["rev-parse", "--is-inside-work-tree"])
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
    }

    /// Create a new scratch environment
    pub fn create(&self, name: &str, source: &Path, mode: ScratchMode) -> Result<PathBuf> {
        let dest = self.scratch_path(name);

        if dest.exists() {
            bail!("Scratch directory already exists: {}", dest.display());
        }

        // Create based on mode
        match mode {
            ScratchMode::Btrfs => self.create_btrfs(source, &dest)?,
            ScratchMode::Git => self.create_git(name, source, &dest)?,
            ScratchMode::Copy => self.create_copy(source, &dest)?,
        }

        // Record metadata
        let mut meta = self.read_meta()?;
        meta.scratches.insert(
            name.to_string(),
            ScratchEntry {
                original: source.to_path_buf(),
                mode,
                created: Utc::now(),
                expires: Some(Utc::now() + Duration::hours(DEFAULT_EXPIRY_HOURS)),
            },
        );
        self.write_meta(&meta)?;

        Ok(dest)
    }

    /// Create a Btrfs snapshot
    fn create_btrfs(&self, source: &Path, dest: &Path) -> Result<()> {
        let output = Command::new("btrfs")
            .args(["subvolume", "snapshot"])
            .arg(source)
            .arg(dest)
            .output()
            .context("Failed to run btrfs command")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            bail!("Failed to create Btrfs snapshot: {}", stderr.trim());
        }

        Ok(())
    }

    /// Create a git worktree
    fn create_git(&self, name: &str, source: &Path, dest: &Path) -> Result<()> {
        let branch_name = format!("scratch-{}", name);

        let output = Command::new("git")
            .args(["-C"])
            .arg(source)
            .args(["worktree", "add", "-b", &branch_name])
            .arg(dest)
            .output()
            .context("Failed to run git worktree command")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            bail!("Failed to create git worktree: {}", stderr.trim());
        }

        Ok(())
    }

    /// Create a copy using rsync or cp
    fn create_copy(&self, source: &Path, dest: &Path) -> Result<()> {
        fs::create_dir_all(dest)
            .with_context(|| format!("Failed to create destination: {}", dest.display()))?;

        // Try rsync first (better for large directories)
        let rsync_result = Command::new("rsync")
            .args(["-a", "--exclude=.git"])
            .arg(format!("{}/", source.display()))
            .arg(format!("{}/", dest.display()))
            .output();

        match rsync_result {
            Ok(output) if output.status.success() => return Ok(()),
            _ => {
                // Fall back to cp -r
                let output = Command::new("cp")
                    .args(["-r"])
                    .arg(source)
                    .arg(dest.parent().unwrap_or(dest))
                    .output()
                    .context("Failed to copy directory")?;

                if !output.status.success() {
                    let stderr = String::from_utf8_lossy(&output.stderr);
                    bail!("Failed to copy source: {}", stderr.trim());
                }
            }
        }

        Ok(())
    }

    /// List all scratch environments
    pub fn list(&self) -> Result<Vec<ScratchInfo>> {
        let meta = self.read_meta()?;

        let mut scratches: Vec<ScratchInfo> = meta
            .scratches
            .iter()
            .map(|(name, entry)| ScratchInfo {
                name: name.clone(),
                original: entry.original.clone(),
                path: self.scratch_path(name),
                mode: entry.mode,
                created: entry.created,
                expires: entry.expires,
            })
            .collect();

        // Sort by creation time (newest first)
        scratches.sort_by(|a, b| b.created.cmp(&a.created));

        Ok(scratches)
    }

    /// Destroy a scratch environment
    pub fn destroy(&self, name: &str) -> Result<()> {
        let meta = self.read_meta()?;

        let entry = meta
            .scratches
            .get(name)
            .ok_or_else(|| anyhow::anyhow!("Scratch not found in metadata: {}", name))?;

        let dest = self.scratch_path(name);

        // Delete based on mode
        match entry.mode {
            ScratchMode::Btrfs => self.delete_btrfs(name, &dest)?,
            ScratchMode::Git => self.delete_git(name, &entry.original, &dest)?,
            ScratchMode::Copy => self.delete_copy(&dest)?,
        }

        // Remove from metadata
        let mut meta = self.read_meta()?;
        meta.scratches.remove(name);
        self.write_meta(&meta)?;

        Ok(())
    }

    /// Delete a Btrfs subvolume
    fn delete_btrfs(&self, _name: &str, dest: &Path) -> Result<()> {
        // Try btrfs delete first
        let btrfs_result = Command::new("btrfs")
            .args(["subvolume", "delete"])
            .arg(dest)
            .output();

        match btrfs_result {
            Ok(output) if output.status.success() => return Ok(()),
            _ => {
                // Fall back to rm -rf if btrfs delete fails
                self.delete_copy(dest)?;
            }
        }

        Ok(())
    }

    /// Delete a git worktree
    fn delete_git(&self, name: &str, original: &Path, dest: &Path) -> Result<()> {
        let branch_name = format!("scratch-{}", name);

        // Remove the worktree
        let _ = Command::new("git")
            .args(["-C"])
            .arg(original)
            .args(["worktree", "remove", "--force"])
            .arg(dest)
            .output();

        // Delete the branch
        let _ = Command::new("git")
            .args(["-C"])
            .arg(original)
            .args(["branch", "-D", &branch_name])
            .output();

        // Clean up any remaining files
        if dest.exists() {
            self.delete_copy(dest)?;
        }

        Ok(())
    }

    /// Delete a directory recursively
    fn delete_copy(&self, dest: &Path) -> Result<()> {
        if dest.exists() {
            fs::remove_dir_all(dest)
                .with_context(|| format!("Failed to remove directory: {}", dest.display()))?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;

    #[test]
    fn test_scratch_mode_roundtrip() {
        for mode in [ScratchMode::Btrfs, ScratchMode::Git, ScratchMode::Copy] {
            let s = mode.as_str();
            let parsed = ScratchMode::from_str(s).unwrap();
            assert_eq!(mode, parsed);
        }
    }

    #[test]
    fn test_scratch_mode_invalid() {
        assert!(ScratchMode::from_str("invalid").is_none());
    }

    #[test]
    fn test_store_creation() {
        let temp_dir = env::temp_dir().join("scratch_test_store");
        let _ = fs::remove_dir_all(&temp_dir);

        let _store = ScratchStore::new(&temp_dir).unwrap();
        assert!(temp_dir.exists());

        // Cleanup
        let _ = fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_exists_nonexistent() {
        let temp_dir = env::temp_dir().join("scratch_test_exists");
        let _ = fs::remove_dir_all(&temp_dir);

        let store = ScratchStore::new(&temp_dir).unwrap();
        assert!(!store.exists("nonexistent"));

        // Cleanup
        let _ = fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_list_empty() {
        let temp_dir = env::temp_dir().join("scratch_test_list");
        let _ = fs::remove_dir_all(&temp_dir);

        let store = ScratchStore::new(&temp_dir).unwrap();
        let scratches = store.list().unwrap();
        assert!(scratches.is_empty());

        // Cleanup
        let _ = fs::remove_dir_all(&temp_dir);
    }
}
