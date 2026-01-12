//! backup - Project backup with optional encryption for Daedalos
//!
//! Create and manage project backups with support for:
//! - Full backups (tar archives)
//! - Git bundle backups (includes history)
//! - Compression (gzip)
//! - Encryption (age)
//!
//! Storage: ~/.local/share/daedalos/backups/

use anyhow::{bail, Context, Result};
use chrono::{Local, Utc};
use daedalos_core::Paths;
use flate2::write::GzEncoder;
use flate2::read::GzDecoder;
use flate2::Compression;
use serde::{Deserialize, Serialize};
use std::fs::{self, File};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;
use walkdir::WalkDir;

/// Default patterns to exclude from backups
const DEFAULT_EXCLUDES: &[&str] = &[
    ".git",
    "node_modules",
    "__pycache__",
    "*.pyc",
    ".venv",
    "venv",
    ".env",
    "target",
    "dist",
    "build",
    ".cache",
    ".DS_Store",
    "*.log",
];

/// Backup type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum BackupType {
    /// Full tar archive
    Full,
    /// Incremental backup (only changed files)
    Incremental,
    /// Git bundle (includes full history)
    Git,
}

impl BackupType {
    pub fn as_str(&self) -> &'static str {
        match self {
            BackupType::Full => "full",
            BackupType::Incremental => "incremental",
            BackupType::Git => "git",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "full" => Some(BackupType::Full),
            "incremental" => Some(BackupType::Incremental),
            "git" => Some(BackupType::Git),
            _ => None,
        }
    }
}

/// Backup metadata stored alongside archive
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackupMeta {
    /// Backup name
    pub name: String,
    /// Source project name
    pub project: String,
    /// Original source path
    pub path: String,
    /// Backup type
    #[serde(rename = "type")]
    pub backup_type: BackupType,
    /// Unix timestamp
    pub created: i64,
    /// ISO timestamp
    pub created_at: String,
    /// File size in bytes
    pub size: u64,
    /// Whether compressed
    pub compressed: bool,
    /// Whether encrypted
    pub encrypted: bool,
}

/// Backup manager
pub struct BackupManager {
    /// Backup storage directory
    backup_dir: PathBuf,
}

impl BackupManager {
    /// Create a new backup manager
    pub fn new() -> Result<Self> {
        let paths = Paths::new();
        let backup_dir = paths.data.join("backups");
        fs::create_dir_all(&backup_dir)?;
        Ok(Self { backup_dir })
    }

    /// Create with custom backup directory
    pub fn with_dir(backup_dir: PathBuf) -> Result<Self> {
        fs::create_dir_all(&backup_dir)?;
        Ok(Self { backup_dir })
    }

    /// Get the backup directory path
    pub fn backup_dir(&self) -> &Path {
        &self.backup_dir
    }

    /// Generate a backup name from project name
    pub fn generate_name(project: &str) -> String {
        let timestamp = Local::now().format("%Y%m%d-%H%M%S");
        format!("{}-{}", project, timestamp)
    }

    /// Get the git root for a path, or return the path itself
    pub fn get_project_root(path: &Path) -> PathBuf {
        let output = Command::new("git")
            .arg("-C")
            .arg(path)
            .arg("rev-parse")
            .arg("--show-toplevel")
            .output();

        match output {
            Ok(o) if o.status.success() => {
                String::from_utf8_lossy(&o.stdout)
                    .trim()
                    .into()
            }
            _ => path.to_path_buf(),
        }
    }

    /// Create a backup
    pub fn create(
        &self,
        source: &Path,
        name: Option<&str>,
        backup_type: BackupType,
        compress: bool,
        encrypt: bool,
        excludes: &[String],
    ) -> Result<BackupMeta> {
        let source = source.canonicalize()
            .with_context(|| format!("Source path not found: {}", source.display()))?;

        let project = source
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| "backup".to_string());

        let name = name
            .map(|n| n.to_string())
            .unwrap_or_else(|| Self::generate_name(&project));

        match backup_type {
            BackupType::Git => self.create_git_backup(&source, &name, &project, encrypt),
            BackupType::Full | BackupType::Incremental => {
                self.create_tar_backup(&source, &name, &project, compress, encrypt, excludes)
            }
        }
    }

    /// Create a git bundle backup
    fn create_git_backup(
        &self,
        source: &Path,
        name: &str,
        project: &str,
        encrypt: bool,
    ) -> Result<BackupMeta> {
        // Check if it's a git repo
        let git_dir = Command::new("git")
            .arg("-C")
            .arg(source)
            .arg("rev-parse")
            .arg("--git-dir")
            .output()?;

        if !git_dir.status.success() {
            bail!("Not a git repository: {}", source.display());
        }

        let backup_file = self.backup_dir.join(format!("{}.bundle", name));

        // Create git bundle
        let status = Command::new("git")
            .arg("-C")
            .arg(source)
            .arg("bundle")
            .arg("create")
            .arg(&backup_file)
            .arg("--all")
            .status()?;

        if !status.success() {
            bail!("Failed to create git bundle");
        }

        // Encrypt if requested
        let (final_file, encrypted) = if encrypt {
            match self.encrypt_file(&backup_file)? {
                Some(encrypted_file) => {
                    fs::remove_file(&backup_file)?;
                    (encrypted_file, true)
                }
                None => (backup_file.clone(), false),
            }
        } else {
            (backup_file.clone(), false)
        };

        // Get final size
        let final_size = fs::metadata(&final_file)?.len();

        // Create metadata
        let now = Utc::now();
        let meta = BackupMeta {
            name: name.to_string(),
            project: project.to_string(),
            path: source.to_string_lossy().to_string(),
            backup_type: BackupType::Git,
            created: now.timestamp(),
            created_at: now.to_rfc3339(),
            size: final_size,
            compressed: false,
            encrypted,
        };

        // Write metadata
        let meta_path = self.meta_path(name);
        let meta_file = File::create(&meta_path)?;
        serde_json::to_writer_pretty(meta_file, &meta)?;

        Ok(meta)
    }

    /// Create a tar archive backup
    fn create_tar_backup(
        &self,
        source: &Path,
        name: &str,
        project: &str,
        compress: bool,
        encrypt: bool,
        extra_excludes: &[String],
    ) -> Result<BackupMeta> {
        let backup_file = if compress {
            self.backup_dir.join(format!("{}.tar.gz", name))
        } else {
            self.backup_dir.join(format!("{}.tar", name))
        };

        // Build exclude patterns
        let mut excludes: Vec<String> = DEFAULT_EXCLUDES
            .iter()
            .map(|s| s.to_string())
            .collect();
        excludes.extend(extra_excludes.iter().cloned());

        // Create archive
        let file = File::create(&backup_file)?;

        if compress {
            let encoder = GzEncoder::new(file, Compression::default());
            self.write_tar(source, encoder, &excludes)?;
        } else {
            self.write_tar(source, file, &excludes)?;
        }

        // Encrypt if requested
        let (final_file, encrypted) = if encrypt {
            match self.encrypt_file(&backup_file)? {
                Some(encrypted_file) => {
                    fs::remove_file(&backup_file)?;
                    (encrypted_file, true)
                }
                None => (backup_file.clone(), false),
            }
        } else {
            (backup_file.clone(), false)
        };

        // Get final size
        let final_size = fs::metadata(&final_file)?.len();

        // Create metadata
        let now = Utc::now();
        let meta = BackupMeta {
            name: name.to_string(),
            project: project.to_string(),
            path: source.to_string_lossy().to_string(),
            backup_type: BackupType::Full,
            created: now.timestamp(),
            created_at: now.to_rfc3339(),
            size: final_size,
            compressed: compress,
            encrypted,
        };

        // Write metadata
        let meta_path = self.meta_path(name);
        let meta_file = File::create(&meta_path)?;
        serde_json::to_writer_pretty(meta_file, &meta)?;

        Ok(meta)
    }

    /// Write files to a tar archive
    fn write_tar<W: Write>(&self, source: &Path, writer: W, excludes: &[String]) -> Result<()> {
        let mut archive = tar::Builder::new(writer);

        let base_name = source.file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| "backup".to_string());

        for entry in WalkDir::new(source)
            .follow_links(false)
            .into_iter()
            .filter_entry(|e| !self.should_exclude(e.path(), source, excludes))
        {
            let entry = entry?;
            let path = entry.path();

            // Get relative path from source
            let rel_path = path.strip_prefix(source)?;
            let archive_path = PathBuf::from(&base_name).join(rel_path);

            if path.is_file() {
                archive.append_path_with_name(path, &archive_path)?;
            } else if path.is_dir() && path != source {
                archive.append_dir(&archive_path, path)?;
            }
        }

        archive.finish()?;
        Ok(())
    }

    /// Check if a path should be excluded
    fn should_exclude(&self, path: &Path, source: &Path, excludes: &[String]) -> bool {
        let rel_path = path.strip_prefix(source).unwrap_or(path);

        for pattern in excludes {
            // Check if filename matches pattern
            if let Some(name) = path.file_name() {
                let name_str = name.to_string_lossy();

                // Simple glob matching
                if pattern.starts_with('*') && pattern.ends_with('*') {
                    let middle = &pattern[1..pattern.len()-1];
                    if name_str.contains(middle) {
                        return true;
                    }
                } else if pattern.starts_with('*') {
                    let suffix = &pattern[1..];
                    if name_str.ends_with(suffix) {
                        return true;
                    }
                } else if pattern.ends_with('*') {
                    let prefix = &pattern[..pattern.len()-1];
                    if name_str.starts_with(prefix) {
                        return true;
                    }
                } else if name_str == pattern.as_str() {
                    return true;
                }
            }

            // Check full relative path
            let rel_str = rel_path.to_string_lossy();
            if rel_str.contains(pattern.as_str()) {
                return true;
            }
        }

        false
    }

    /// Encrypt a file using age (if available)
    fn encrypt_file(&self, path: &Path) -> Result<Option<PathBuf>> {
        // Check if age is available
        if Command::new("age").arg("--version").output().is_err() {
            eprintln!("Warning: age not available, skipping encryption");
            return Ok(None);
        }

        // Try to get encryption key from secrets tool
        let key_output = Command::new("secrets")
            .arg("key")
            .output();

        let recipient = match key_output {
            Ok(o) if o.status.success() => {
                String::from_utf8_lossy(&o.stdout).trim().to_string()
            }
            _ => {
                eprintln!("Warning: No encryption key found, skipping encryption");
                return Ok(None);
            }
        };

        if recipient.is_empty() {
            return Ok(None);
        }

        let encrypted_path = path.with_extension(
            format!("{}.age", path.extension().unwrap_or_default().to_string_lossy())
        );

        let status = Command::new("age")
            .arg("-r")
            .arg(&recipient)
            .arg("-o")
            .arg(&encrypted_path)
            .arg(path)
            .status()?;

        if status.success() {
            Ok(Some(encrypted_path))
        } else {
            bail!("age encryption failed");
        }
    }

    /// Decrypt a file using age
    fn decrypt_file(&self, path: &Path) -> Result<PathBuf> {
        // Find identity file
        let paths = Paths::new();
        let identity = paths.data.join("secrets/keys/identity.key");

        if !identity.exists() {
            bail!("No decryption key found at {}", identity.display());
        }

        let decrypted_path = path.with_extension("");

        let status = Command::new("age")
            .arg("-d")
            .arg("-i")
            .arg(&identity)
            .arg("-o")
            .arg(&decrypted_path)
            .arg(path)
            .status()?;

        if status.success() {
            Ok(decrypted_path)
        } else {
            bail!("age decryption failed");
        }
    }

    /// List all backups
    pub fn list(&self, project_filter: Option<&str>) -> Result<Vec<BackupMeta>> {
        let mut backups = Vec::new();

        if !self.backup_dir.exists() {
            return Ok(backups);
        }

        for entry in fs::read_dir(&self.backup_dir)? {
            let entry = entry?;
            let path = entry.path();

            if path.extension().map(|e| e == "json").unwrap_or(false) {
                if let Ok(meta) = self.load_meta(&path) {
                    if let Some(filter) = project_filter {
                        if meta.project != filter {
                            continue;
                        }
                    }
                    backups.push(meta);
                }
            }
        }

        // Sort by creation time (newest first)
        backups.sort_by(|a, b| b.created.cmp(&a.created));

        Ok(backups)
    }

    /// Get a specific backup's metadata
    pub fn get(&self, name: &str) -> Result<Option<BackupMeta>> {
        let meta_path = self.meta_path(name);
        if meta_path.exists() {
            Ok(Some(self.load_meta(&meta_path)?))
        } else {
            Ok(None)
        }
    }

    /// Find the backup file for a given name
    pub fn find_backup_file(&self, name: &str) -> Option<PathBuf> {
        let extensions = [".tar.gz", ".tar", ".bundle", ".tar.gz.age", ".tar.age", ".bundle.age"];

        for ext in extensions {
            let path = self.backup_dir.join(format!("{}{}", name, ext));
            if path.exists() {
                return Some(path);
            }
        }

        None
    }

    /// Restore a backup
    pub fn restore(&self, name: &str, target: Option<&Path>, force: bool) -> Result<PathBuf> {
        let meta = self.get(name)?
            .ok_or_else(|| anyhow::anyhow!("Backup not found: {}", name))?;

        let backup_file = self.find_backup_file(name)
            .ok_or_else(|| anyhow::anyhow!("Backup file not found for: {}", name))?;

        // Determine target directory
        let target = target
            .map(|p| p.to_path_buf())
            .unwrap_or_else(|| PathBuf::from(&meta.project));

        // Check if target exists
        if target.exists() && !force {
            bail!("Target exists: {}. Use --force to overwrite.", target.display());
        }

        // Handle decryption if needed
        let work_file = if meta.encrypted {
            self.decrypt_file(&backup_file)?
        } else {
            backup_file.clone()
        };

        // Restore based on backup type
        match meta.backup_type {
            BackupType::Git => {
                fs::create_dir_all(&target)?;
                let status = Command::new("git")
                    .arg("clone")
                    .arg(&work_file)
                    .arg(&target)
                    .status()?;

                if !status.success() {
                    bail!("Failed to restore git bundle");
                }
            }
            BackupType::Full | BackupType::Incremental => {
                let parent = target.parent().unwrap_or(Path::new("."));
                fs::create_dir_all(parent)?;

                let file = File::open(&work_file)?;

                if meta.compressed {
                    let decoder = GzDecoder::new(file);
                    let mut archive = tar::Archive::new(decoder);
                    archive.unpack(parent)?;
                } else {
                    let mut archive = tar::Archive::new(file);
                    archive.unpack(parent)?;
                }
            }
        }

        // Clean up decrypted temp file
        if meta.encrypted && work_file != backup_file {
            let _ = fs::remove_file(&work_file);
        }

        Ok(target)
    }

    /// Delete a backup
    pub fn delete(&self, name: &str) -> Result<()> {
        let mut deleted = false;

        // Delete backup file
        if let Some(backup_file) = self.find_backup_file(name) {
            fs::remove_file(&backup_file)?;
            deleted = true;
        }

        // Delete metadata
        let meta_path = self.meta_path(name);
        if meta_path.exists() {
            fs::remove_file(&meta_path)?;
            deleted = true;
        }

        if !deleted {
            bail!("Backup not found: {}", name);
        }

        Ok(())
    }

    /// Prune old backups, keeping the most recent N per project
    pub fn prune(&self, keep: usize, project_filter: Option<&str>, dry_run: bool) -> Result<Vec<String>> {
        let mut pruned = Vec::new();

        // Group backups by project
        let all_backups = self.list(None)?;
        let mut by_project: std::collections::HashMap<String, Vec<BackupMeta>> =
            std::collections::HashMap::new();

        for backup in all_backups {
            if let Some(filter) = project_filter {
                if backup.project != filter {
                    continue;
                }
            }
            by_project
                .entry(backup.project.clone())
                .or_default()
                .push(backup);
        }

        // Prune each project
        for (_project, mut backups) in by_project {
            // Sort by creation time (newest first)
            backups.sort_by(|a, b| b.created.cmp(&a.created));

            // Skip the first `keep` entries, delete the rest
            for backup in backups.into_iter().skip(keep) {
                if dry_run {
                    pruned.push(backup.name.clone());
                } else {
                    if self.delete(&backup.name).is_ok() {
                        pruned.push(backup.name);
                    }
                }
            }
        }

        Ok(pruned)
    }

    /// Export a backup to a specific location
    pub fn export(&self, name: &str, output: &Path) -> Result<()> {
        let backup_file = self.find_backup_file(name)
            .ok_or_else(|| anyhow::anyhow!("Backup not found: {}", name))?;

        fs::copy(&backup_file, output)?;

        // Also copy metadata
        let meta_path = self.meta_path(name);
        if meta_path.exists() {
            let output_meta = output.with_extension("meta.json");
            fs::copy(&meta_path, &output_meta)?;
        }

        Ok(())
    }

    /// Import a backup from an external file
    pub fn import(&self, file: &Path) -> Result<String> {
        let file_name = file
            .file_name()
            .ok_or_else(|| anyhow::anyhow!("Invalid file path"))?
            .to_string_lossy()
            .to_string();

        let dest = self.backup_dir.join(&file_name);

        if dest.exists() {
            bail!("Backup already exists: {}", file_name);
        }

        fs::copy(file, &dest)?;

        // Try to import metadata too
        let meta_file = file.with_extension("meta.json");
        if meta_file.exists() {
            let dest_meta = self.backup_dir.join(
                meta_file.file_name().unwrap()
            );
            fs::copy(&meta_file, &dest_meta)?;
        }

        Ok(file_name)
    }

    /// Get metadata file path for a backup
    fn meta_path(&self, name: &str) -> PathBuf {
        self.backup_dir.join(format!("{}.meta.json", name))
    }

    /// Load metadata from file
    fn load_meta(&self, path: &Path) -> Result<BackupMeta> {
        let file = File::open(path)?;
        let meta: BackupMeta = serde_json::from_reader(file)?;
        Ok(meta)
    }
}

impl Default for BackupManager {
    fn default() -> Self {
        Self::new().expect("Failed to create backup manager")
    }
}

/// Format bytes as human-readable size
pub fn format_size(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = KB * 1024;
    const GB: u64 = MB * 1024;

    if bytes >= GB {
        format!("{:.1}G", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.1}M", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.1}K", bytes as f64 / KB as f64)
    } else {
        format!("{}B", bytes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_size() {
        assert_eq!(format_size(500), "500B");
        assert_eq!(format_size(1024), "1.0K");
        assert_eq!(format_size(1536), "1.5K");
        assert_eq!(format_size(1048576), "1.0M");
        assert_eq!(format_size(1073741824), "1.0G");
    }

    #[test]
    fn test_generate_name() {
        let name = BackupManager::generate_name("myproject");
        assert!(name.starts_with("myproject-"));
        assert!(name.len() > "myproject-".len());
    }

    #[test]
    fn test_backup_type_roundtrip() {
        for bt in [BackupType::Full, BackupType::Incremental, BackupType::Git] {
            let s = bt.as_str();
            let parsed = BackupType::from_str(s).unwrap();
            assert_eq!(bt, parsed);
        }
    }
}
