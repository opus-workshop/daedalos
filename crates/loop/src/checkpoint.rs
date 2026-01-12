//! Checkpoint backends for loop state preservation
//!
//! Every iteration creates a rollback point. This module provides:
//! - GitCheckpoint: Git-based checkpoints (works everywhere)
//! - BtrfsCheckpoint: Instant, space-efficient snapshots (Linux + Btrfs)
//! - NoneCheckpoint: No checkpoints (for low-risk operations)

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use daedalos_core::Paths;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Checkpoint strategy
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CheckpointStrategy {
    Auto,
    Git,
    Btrfs,
    None,
}

/// Represents a checkpoint
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Checkpoint {
    pub id: String,
    pub name: String,
    pub created_at: DateTime<Utc>,
    pub path: String,
    pub backend: String,
}

/// Trait for checkpoint backends
pub trait CheckpointBackend: Send + Sync {
    /// Return the backend name
    fn name(&self) -> &str;

    /// Create a checkpoint, return checkpoint ID
    fn create(&self, name: &str, path: &PathBuf) -> Result<String>;

    /// Restore to a checkpoint
    fn restore(&self, checkpoint_id: &str, path: &PathBuf) -> Result<bool>;

    /// List available checkpoints
    fn list(&self, path: &PathBuf) -> Result<Vec<Checkpoint>>;

    /// Delete a checkpoint
    fn delete(&self, checkpoint_id: &str) -> Result<bool>;

    /// Check if a checkpoint exists
    fn exists(&self, checkpoint_id: &str) -> bool;
}

/// Git-based checkpoints
///
/// Works everywhere git works. Uses branches to store checkpoints.
pub struct GitCheckpoint {
    repo_path: Option<PathBuf>,
    branch_prefix: String,
}

impl GitCheckpoint {
    pub fn new(repo_path: Option<PathBuf>) -> Self {
        Self {
            repo_path,
            branch_prefix: "loop-checkpoint".to_string(),
        }
    }

    fn git(&self, args: &[&str], cwd: Option<&PathBuf>) -> Result<std::process::Output> {
        let working_dir = cwd.or(self.repo_path.as_ref());
        let mut cmd = std::process::Command::new("git");

        if let Some(dir) = working_dir {
            cmd.arg("-C").arg(dir);
        }

        for arg in args {
            cmd.arg(arg);
        }

        cmd.output().context("Failed to run git command")
    }
}

impl CheckpointBackend for GitCheckpoint {
    fn name(&self) -> &str {
        "git"
    }

    fn create(&self, name: &str, path: &PathBuf) -> Result<String> {
        let timestamp = Utc::now().format("%Y%m%d_%H%M%S");
        let checkpoint_id = format!("{}/{}_{}", self.branch_prefix, name, timestamp);

        // Stash any uncommitted changes
        let stash_result = self.git(&["stash", "push", "-m", &format!("loop-auto-stash-{}", checkpoint_id)], Some(path))?;
        let had_changes = !String::from_utf8_lossy(&stash_result.stdout).contains("No local changes");

        // Check if we have any commits
        let head_result = self.git(&["rev-parse", "HEAD"], Some(path))?;
        if !head_result.status.success() {
            // No commits yet, create initial commit
            self.git(&["add", "-A"], Some(path))?;
            self.git(&["commit", "-m", "Initial commit for loop checkpoint", "--allow-empty"], Some(path))?;
        }

        // Create checkpoint branch
        let result = self.git(&["branch", &checkpoint_id], Some(path))?;
        if !result.status.success() {
            let stderr = String::from_utf8_lossy(&result.stderr);
            anyhow::bail!("Failed to create checkpoint branch: {}", stderr);
        }

        // Restore stashed changes
        if had_changes {
            self.git(&["stash", "pop"], Some(path))?;
        }

        Ok(checkpoint_id)
    }

    fn restore(&self, checkpoint_id: &str, path: &PathBuf) -> Result<bool> {
        // Stash current changes
        self.git(&["stash", "push", "-m", "loop-pre-restore"], Some(path))?;

        // Get current branch
        let current_result = self.git(&["rev-parse", "--abbrev-ref", "HEAD"], Some(path))?;
        let current_branch = String::from_utf8_lossy(&current_result.stdout).trim().to_string();

        // Checkout the checkpoint
        let result = self.git(&["checkout", checkpoint_id], Some(path))?;
        if !result.status.success() {
            self.git(&["stash", "pop"], Some(path))?;
            return Ok(false);
        }

        // If we were on a different branch, recreate it at this point
        if current_branch != checkpoint_id && !current_branch.starts_with(&self.branch_prefix) {
            self.git(&["branch", "-D", &current_branch], Some(path))?;
            self.git(&["checkout", "-b", &current_branch], Some(path))?;
        }

        Ok(true)
    }

    fn list(&self, path: &PathBuf) -> Result<Vec<Checkpoint>> {
        let result = self.git(
            &[
                "branch",
                "--list",
                &format!("{}/*", self.branch_prefix),
                "--format=%(refname:short)|%(creatordate:iso-strict)",
            ],
            Some(path),
        )?;

        let mut checkpoints = Vec::new();
        let output = String::from_utf8_lossy(&result.stdout);

        for line in output.lines() {
            if let Some((branch, date)) = line.split_once('|') {
                let name = branch
                    .strip_prefix(&format!("{}/", self.branch_prefix))
                    .unwrap_or(branch)
                    .to_string();

                let created_at = DateTime::parse_from_rfc3339(date)
                    .map(|d| d.with_timezone(&Utc))
                    .unwrap_or_else(|_| Utc::now());

                checkpoints.push(Checkpoint {
                    id: branch.to_string(),
                    name,
                    created_at,
                    path: path.to_string_lossy().to_string(),
                    backend: "git".to_string(),
                });
            }
        }

        checkpoints.sort_by(|a, b| b.created_at.cmp(&a.created_at));
        Ok(checkpoints)
    }

    fn delete(&self, checkpoint_id: &str) -> Result<bool> {
        let result = self.git(&["branch", "-D", checkpoint_id], None)?;
        Ok(result.status.success())
    }

    fn exists(&self, checkpoint_id: &str) -> bool {
        self.git(&["rev-parse", "--verify", checkpoint_id], None)
            .map(|r| r.status.success())
            .unwrap_or(false)
    }
}

/// Btrfs snapshot-based checkpoints
///
/// Instant snapshots with copy-on-write (minimal space usage).
/// Only works on Linux with Btrfs filesystem.
pub struct BtrfsCheckpoint {
    snapshot_dir: PathBuf,
}

impl BtrfsCheckpoint {
    pub fn new() -> Self {
        let paths = Paths::new();
        let snapshot_dir = paths.state("loop").join("snapshots");
        std::fs::create_dir_all(&snapshot_dir).ok();

        Self { snapshot_dir }
    }
}

impl CheckpointBackend for BtrfsCheckpoint {
    fn name(&self) -> &str {
        "btrfs"
    }

    fn create(&self, name: &str, path: &PathBuf) -> Result<String> {
        let timestamp = Utc::now().format("%Y%m%d_%H%M%S");
        let checkpoint_id = format!("{}_{}", name, timestamp);
        let snapshot_path = self.snapshot_dir.join(&checkpoint_id);

        let output = std::process::Command::new("btrfs")
            .args(["subvolume", "snapshot", "-r"])
            .arg(path)
            .arg(&snapshot_path)
            .output()
            .context("Failed to run btrfs command")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            anyhow::bail!("Failed to create Btrfs snapshot: {}", stderr);
        }

        Ok(checkpoint_id)
    }

    fn restore(&self, checkpoint_id: &str, path: &PathBuf) -> Result<bool> {
        let snapshot_path = self.snapshot_dir.join(checkpoint_id);
        if !snapshot_path.exists() {
            return Ok(false);
        }

        // Check if path is a subvolume
        let check_result = std::process::Command::new("btrfs")
            .args(["subvolume", "show"])
            .arg(path)
            .output()?;

        if check_result.status.success() {
            // Delete the subvolume
            std::process::Command::new("btrfs")
                .args(["subvolume", "delete"])
                .arg(path)
                .output()?;
        }

        // Create writable snapshot from the read-only checkpoint
        let result = std::process::Command::new("btrfs")
            .args(["subvolume", "snapshot"])
            .arg(&snapshot_path)
            .arg(path)
            .output()?;

        Ok(result.status.success())
    }

    fn list(&self, _path: &PathBuf) -> Result<Vec<Checkpoint>> {
        let mut checkpoints = Vec::new();

        if self.snapshot_dir.exists() {
            for entry in std::fs::read_dir(&self.snapshot_dir)? {
                let entry = entry?;
                let path = entry.path();
                if path.is_dir() {
                    let name = path.file_name().unwrap().to_string_lossy().to_string();
                    let metadata = std::fs::metadata(&path)?;
                    let created_at = metadata
                        .created()
                        .map(|t| DateTime::<Utc>::from(t))
                        .unwrap_or_else(|_| Utc::now());

                    checkpoints.push(Checkpoint {
                        id: name.clone(),
                        name,
                        created_at,
                        path: path.to_string_lossy().to_string(),
                        backend: "btrfs".to_string(),
                    });
                }
            }
        }

        checkpoints.sort_by(|a, b| b.created_at.cmp(&a.created_at));
        Ok(checkpoints)
    }

    fn delete(&self, checkpoint_id: &str) -> Result<bool> {
        let snapshot_path = self.snapshot_dir.join(checkpoint_id);
        if snapshot_path.exists() {
            let result = std::process::Command::new("btrfs")
                .args(["subvolume", "delete"])
                .arg(&snapshot_path)
                .output()?;
            Ok(result.status.success())
        } else {
            Ok(false)
        }
    }

    fn exists(&self, checkpoint_id: &str) -> bool {
        self.snapshot_dir.join(checkpoint_id).exists()
    }
}

/// No-op checkpoint backend
///
/// Use for low-risk operations or when speed is critical.
pub struct NoneCheckpoint;

impl NoneCheckpoint {
    pub fn new() -> Self {
        Self
    }
}

impl CheckpointBackend for NoneCheckpoint {
    fn name(&self) -> &str {
        "none"
    }

    fn create(&self, name: &str, _path: &PathBuf) -> Result<String> {
        let timestamp = Utc::now().format("%Y%m%d_%H%M%S");
        Ok(format!("none_{}_{}", name, timestamp))
    }

    fn restore(&self, _checkpoint_id: &str, _path: &PathBuf) -> Result<bool> {
        // Can't restore without checkpoints
        Ok(false)
    }

    fn list(&self, _path: &PathBuf) -> Result<Vec<Checkpoint>> {
        Ok(Vec::new())
    }

    fn delete(&self, _checkpoint_id: &str) -> Result<bool> {
        Ok(true)
    }

    fn exists(&self, _checkpoint_id: &str) -> bool {
        false
    }
}

/// Auto-detect the best checkpoint backend for a path
pub fn detect_backend(path: &PathBuf) -> CheckpointStrategy {
    // Check for Btrfs
    let btrfs_result = std::process::Command::new("btrfs")
        .args(["subvolume", "show"])
        .arg(path)
        .output();

    if let Ok(result) = btrfs_result {
        if result.status.success() {
            return CheckpointStrategy::Btrfs;
        }
    }

    // Check for Git
    let git_result = std::process::Command::new("git")
        .args(["-C"])
        .arg(path)
        .args(["rev-parse", "--git-dir"])
        .output();

    if let Ok(result) = git_result {
        if result.status.success() {
            return CheckpointStrategy::Git;
        }
    }

    CheckpointStrategy::None
}

/// Factory function to get appropriate checkpoint backend
pub fn get_backend(
    path: &PathBuf,
    strategy: CheckpointStrategy,
) -> Result<Box<dyn CheckpointBackend>> {
    let strategy = if strategy == CheckpointStrategy::Auto {
        detect_backend(path)
    } else {
        strategy
    };

    match strategy {
        CheckpointStrategy::Btrfs => Ok(Box::new(BtrfsCheckpoint::new())),
        CheckpointStrategy::Git => Ok(Box::new(GitCheckpoint::new(Some(path.clone())))),
        CheckpointStrategy::None | CheckpointStrategy::Auto => Ok(Box::new(NoneCheckpoint::new())),
    }
}
