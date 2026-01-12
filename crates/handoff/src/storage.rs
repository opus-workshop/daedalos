//! Handoff storage - save and retrieve handoff documents

use anyhow::{anyhow, Context, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

use daedalos_core::Paths;

/// Get the handoff storage directory
fn handoff_dir() -> std::path::PathBuf {
    Paths::new().data.join("handoff")
}

/// A handoff document
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Handoff {
    /// Name/identifier of the handoff
    pub name: String,
    /// When the handoff was created
    pub created: DateTime<Utc>,
    /// Who created the handoff
    pub from: String,
    /// Who the handoff is for (optional)
    pub to: Option<String>,
    /// Hours of history covered
    pub hours: u64,
    /// The full markdown content
    pub content: String,
}

/// Metadata about a handoff (for listing)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HandoffMetadata {
    /// Name of the handoff
    pub name: String,
    /// When it was created
    pub created: DateTime<Utc>,
    /// Who created it
    pub from: String,
    /// Who it's for
    pub to: Option<String>,
    /// File path
    pub path: PathBuf,
}

/// Storage for handoff documents
pub struct HandoffStorage {
    /// Directory where handoffs are stored
    dir: PathBuf,
}

impl HandoffStorage {
    /// Create a new storage instance
    pub fn new() -> Result<Self> {
        let dir = handoff_dir();
        fs::create_dir_all(&dir)
            .with_context(|| format!("Failed to create handoff directory: {}", dir.display()))?;

        Ok(Self { dir })
    }

    /// Save a handoff to disk
    pub fn save(&self, handoff: &Handoff) -> Result<PathBuf> {
        let filename = format!("{}.md", &handoff.name);
        let path = self.dir.join(&filename);

        fs::write(&path, &handoff.content)
            .with_context(|| format!("Failed to write handoff: {}", path.display()))?;

        Ok(path)
    }

    /// Get a handoff by name (or most recent if name is None)
    pub fn get(&self, name: Option<&str>) -> Result<Handoff> {
        let path = match name {
            Some(n) => {
                let mut path = self.dir.join(n);
                if !path.exists() {
                    path = self.dir.join(format!("{}.md", n));
                }
                if !path.exists() {
                    return Err(anyhow!("Handoff not found: {}", n));
                }
                path
            }
            None => {
                // Get most recent
                self.get_most_recent()?
                    .ok_or_else(|| anyhow!("No handoffs available"))?
            }
        };

        self.load_handoff(&path)
    }

    /// List all available handoffs
    pub fn list(&self) -> Result<Vec<HandoffMetadata>> {
        let mut handoffs = Vec::new();

        if !self.dir.exists() {
            return Ok(handoffs);
        }

        let entries = fs::read_dir(&self.dir)
            .with_context(|| format!("Failed to read handoff directory: {}", self.dir.display()))?;

        for entry in entries {
            let entry = entry?;
            let path = entry.path();

            if path.extension().map(|e| e == "md").unwrap_or(false) {
                if let Ok(metadata) = self.extract_metadata(&path) {
                    handoffs.push(metadata);
                }
            }
        }

        // Sort by creation time, most recent first
        handoffs.sort_by(|a, b| b.created.cmp(&a.created));

        Ok(handoffs)
    }

    /// Get the path to the most recent handoff
    fn get_most_recent(&self) -> Result<Option<PathBuf>> {
        let handoffs = self.list()?;
        Ok(handoffs.first().map(|h| h.path.clone()))
    }

    /// Load a handoff from a file path
    fn load_handoff(&self, path: &PathBuf) -> Result<Handoff> {
        let content = fs::read_to_string(path)
            .with_context(|| format!("Failed to read handoff: {}", path.display()))?;

        let name = path
            .file_stem()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_else(|| "unknown".to_string());

        // Extract metadata from content
        let (from, to, created, hours) = self.parse_header(&content);

        Ok(Handoff {
            name,
            created,
            from,
            to,
            hours,
            content,
        })
    }

    /// Extract metadata from a handoff file
    fn extract_metadata(&self, path: &PathBuf) -> Result<HandoffMetadata> {
        let content = fs::read_to_string(path)?;
        let name = path
            .file_stem()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_else(|| "unknown".to_string());

        let (from, to, created, _) = self.parse_header(&content);

        Ok(HandoffMetadata {
            name,
            created,
            from,
            to,
            path: path.clone(),
        })
    }

    /// Parse header fields from handoff content
    fn parse_header(&self, content: &str) -> (String, Option<String>, DateTime<Utc>, u64) {
        let mut from = "unknown".to_string();
        let mut to: Option<String> = None;
        let mut created = Utc::now();
        let mut hours = 8u64;

        for line in content.lines().take(10) {
            if line.starts_with("**From**:") {
                from = line.trim_start_matches("**From**:").trim().to_string();
            } else if line.starts_with("**To**:") {
                let t = line.trim_start_matches("**To**:").trim();
                if !t.is_empty() {
                    to = Some(t.to_string());
                }
            } else if line.starts_with("**Created**:") {
                let date_str = line.trim_start_matches("**Created**:").trim();
                if let Ok(dt) = chrono::NaiveDateTime::parse_from_str(date_str, "%Y-%m-%d %H:%M") {
                    created = dt.and_utc();
                }
            } else if line.starts_with("**Hours covered**:") {
                let h = line.trim_start_matches("**Hours covered**:").trim();
                hours = h.parse().unwrap_or(8);
            }
        }

        (from, to, created, hours)
    }

    /// Get the storage directory path
    #[allow(dead_code)]
    pub fn dir(&self) -> &PathBuf {
        &self.dir
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn test_storage() -> (HandoffStorage, TempDir) {
        let temp = TempDir::new().unwrap();
        let storage = HandoffStorage {
            dir: temp.path().to_path_buf(),
        };
        (storage, temp)
    }

    #[test]
    fn test_save_and_load() {
        let (storage, _temp) = test_storage();

        let handoff = Handoff {
            name: "test-handoff".to_string(),
            created: Utc::now(),
            from: "testuser".to_string(),
            to: Some("recipient".to_string()),
            hours: 8,
            content: "# Handoff: test-handoff\n\n**From**: testuser\n**To**: recipient\n**Created**: 2025-01-11 12:00\n**Hours covered**: 8\n\nTest content".to_string(),
        };

        let path = storage.save(&handoff).unwrap();
        assert!(path.exists());

        let loaded = storage.get(Some("test-handoff")).unwrap();
        assert_eq!(loaded.name, "test-handoff");
        assert_eq!(loaded.from, "testuser");
    }

    #[test]
    fn test_list_empty() {
        let (storage, _temp) = test_storage();
        let list = storage.list().unwrap();
        assert!(list.is_empty());
    }

    #[test]
    fn test_list_with_handoffs() {
        let (storage, _temp) = test_storage();

        // Save a handoff
        let handoff = Handoff {
            name: "test".to_string(),
            created: Utc::now(),
            from: "user".to_string(),
            to: None,
            hours: 8,
            content: "# Handoff: test\n\n**From**: user\n**Created**: 2025-01-11 12:00\n**Hours covered**: 8\n\nContent".to_string(),
        };
        storage.save(&handoff).unwrap();

        let list = storage.list().unwrap();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].name, "test");
    }

    #[test]
    fn test_get_most_recent() {
        let (storage, _temp) = test_storage();

        // No handoffs
        assert!(storage.get(None).is_err());
    }
}
