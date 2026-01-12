//! Sandbox backend implementations
//!
//! Backends handle the actual filesystem operations for creating sandboxes:
//! - Btrfs: Uses copy-on-write snapshots (instant, space-efficient)
//! - Overlay: Uses overlayfs mounts (fast, requires fuse-overlayfs or root)
//! - Rsync: Simple copy (universal fallback, slower but works everywhere)

use anyhow::{bail, Context, Result};
use std::path::Path;
use std::process::Command;

use crate::sandbox::Backend;

/// Create a sandbox copy using the specified backend
pub fn create_sandbox(backend: Backend, source: &Path, work_path: &Path) -> Result<()> {
    match backend {
        Backend::Btrfs => create_btrfs(source, work_path),
        Backend::Overlay => create_overlay(source, work_path),
        Backend::Rsync => create_rsync(source, work_path),
    }
}

/// Clean up a sandbox using the specified backend
pub fn cleanup_sandbox(backend: Backend, sandbox_path: &Path) -> Result<()> {
    let work_path = sandbox_path.join("work");

    match backend {
        Backend::Btrfs => cleanup_btrfs(&work_path),
        Backend::Overlay => cleanup_overlay(sandbox_path),
        Backend::Rsync => cleanup_rsync(sandbox_path),
    }
}

/// Create sandbox using Btrfs subvolume snapshot
fn create_btrfs(source: &Path, work_path: &Path) -> Result<()> {
    let status = Command::new("btrfs")
        .args(["subvolume", "snapshot"])
        .arg(source)
        .arg(work_path)
        .status()
        .context("Failed to run btrfs command")?;

    if !status.success() {
        bail!("Failed to create Btrfs snapshot");
    }

    Ok(())
}

/// Cleanup Btrfs subvolume
fn cleanup_btrfs(work_path: &Path) -> Result<()> {
    // Try to delete as subvolume first
    let status = Command::new("btrfs")
        .args(["subvolume", "delete"])
        .arg(work_path)
        .status();

    // Fall back to rm if that fails
    if !status.map(|s| s.success()).unwrap_or(false) {
        std::fs::remove_dir_all(work_path).ok();
    }

    Ok(())
}

/// Create sandbox using overlayfs
fn create_overlay(source: &Path, work_path: &Path) -> Result<()> {
    let sandbox_path = work_path.parent().context("Invalid work path")?;
    let upper_path = sandbox_path.join("upper");
    let workdir_path = sandbox_path.join("workdir");

    std::fs::create_dir_all(&work_path)?;
    std::fs::create_dir_all(&upper_path)?;
    std::fs::create_dir_all(&workdir_path)?;

    // Try fuse-overlayfs first (doesn't require root)
    let fuse_result = Command::new("fuse-overlayfs")
        .arg("-o")
        .arg(format!(
            "lowerdir={},upperdir={},workdir={}",
            source.display(),
            upper_path.display(),
            workdir_path.display()
        ))
        .arg(work_path)
        .status();

    if fuse_result.map(|s| s.success()).unwrap_or(false) {
        return Ok(());
    }

    // Fall back to sudo mount
    let mount_result = Command::new("sudo")
        .args(["mount", "-t", "overlay", "overlay", "-o"])
        .arg(format!(
            "lowerdir={},upperdir={},workdir={}",
            source.display(),
            upper_path.display(),
            workdir_path.display()
        ))
        .arg(work_path)
        .status();

    if !mount_result.map(|s| s.success()).unwrap_or(false) {
        // Clean up failed attempt
        std::fs::remove_dir_all(sandbox_path).ok();
        bail!("Failed to create overlay mount (may need sudo or fuse-overlayfs)");
    }

    Ok(())
}

/// Cleanup overlay mount
fn cleanup_overlay(sandbox_path: &Path) -> Result<()> {
    let work_path = sandbox_path.join("work");

    // Try to unmount
    let _ = Command::new("umount").arg(&work_path).status();
    let _ = Command::new("sudo")
        .args(["umount"])
        .arg(&work_path)
        .status();
    let _ = Command::new("fusermount")
        .args(["-u"])
        .arg(&work_path)
        .status();

    // Remove directories
    std::fs::remove_dir_all(sandbox_path).ok();

    Ok(())
}

/// Create sandbox using rsync copy
fn create_rsync(source: &Path, work_path: &Path) -> Result<()> {
    std::fs::create_dir_all(work_path)?;

    let status = Command::new("rsync")
        .args(["-a"])
        .arg(format!("{}/", source.display()))
        .arg(format!("{}/", work_path.display()))
        .status()
        .context("Failed to run rsync")?;

    if !status.success() {
        std::fs::remove_dir_all(work_path).ok();
        bail!("Failed to copy source with rsync");
    }

    Ok(())
}

/// Cleanup rsync copy (just remove the directory)
fn cleanup_rsync(sandbox_path: &Path) -> Result<()> {
    std::fs::remove_dir_all(sandbox_path)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn test_rsync_create_and_cleanup() -> Result<()> {
        let source = tempdir()?;
        let sandbox_dir = tempdir()?;

        // Create test file in source
        fs::write(source.path().join("test.txt"), "hello")?;

        let work_path = sandbox_dir.path().join("work");
        create_rsync(source.path(), &work_path)?;

        // Verify copy
        assert!(work_path.join("test.txt").exists());
        let content = fs::read_to_string(work_path.join("test.txt"))?;
        assert_eq!(content, "hello");

        // Cleanup
        cleanup_rsync(sandbox_dir.path())?;
        assert!(!sandbox_dir.path().exists());

        Ok(())
    }
}
