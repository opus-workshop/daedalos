//! SQLite database for undo timeline
//!
//! Stores metadata in SQLite, file content in filesystem.
//! Hybrid storage: small files (<100KB) inline, large files on disk.

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use flate2::{read::GzDecoder, write::GzEncoder, Compression};
use rusqlite::{params, Connection, OptionalExtension};
use sha2::{Digest, Sha256};
use std::fs;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

/// Size threshold for inline storage (100KB)
const INLINE_THRESHOLD: usize = 100 * 1024;

/// Change types for undo entries
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChangeType {
    Edit,
    Create,
    Delete,
    Rename,
    Checkpoint,
}

impl ChangeType {
    pub fn as_str(&self) -> &'static str {
        match self {
            ChangeType::Edit => "edit",
            ChangeType::Create => "create",
            ChangeType::Delete => "delete",
            ChangeType::Rename => "rename",
            ChangeType::Checkpoint => "checkpoint",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "edit" => Some(ChangeType::Edit),
            "create" => Some(ChangeType::Create),
            "delete" => Some(ChangeType::Delete),
            "rename" => Some(ChangeType::Rename),
            "checkpoint" => Some(ChangeType::Checkpoint),
            _ => None,
        }
    }
}

/// A single entry in the undo timeline
#[derive(Debug, Clone)]
pub struct UndoEntry {
    pub id: String,
    pub timestamp: DateTime<Utc>,
    pub change_type: ChangeType,
    pub file_path: String,
    pub description: String,
    pub backup_hash: Option<String>,
    pub file_size: i64,
    pub project_path: String,
}

/// A named checkpoint
#[derive(Debug, Clone)]
pub struct Checkpoint {
    pub id: String,
    pub name: String,
    pub timestamp: DateTime<Utc>,
    pub description: String,
}

/// Database for undo timeline
pub struct UndoDatabase {
    conn: Connection,
    data_dir: PathBuf,
    backup_dir: PathBuf,
}

impl UndoDatabase {
    /// Create or open the database
    pub fn open(data_dir: &Path) -> Result<Self> {
        let data_dir = data_dir.to_path_buf();
        fs::create_dir_all(&data_dir)
            .with_context(|| format!("Failed to create data directory: {}", data_dir.display()))?;

        let backup_dir = data_dir.join("backups");
        fs::create_dir_all(&backup_dir)
            .with_context(|| format!("Failed to create backup directory: {}", backup_dir.display()))?;

        let db_path = data_dir.join("timeline.db");
        let conn = Connection::open(&db_path)
            .with_context(|| format!("Failed to open database: {}", db_path.display()))?;

        let db = Self {
            conn,
            data_dir,
            backup_dir,
        };
        db.init_schema()?;
        Ok(db)
    }

    /// Initialize database schema
    fn init_schema(&self) -> Result<()> {
        self.conn.execute_batch(
            r#"
            CREATE TABLE IF NOT EXISTS entries (
                id TEXT PRIMARY KEY,
                timestamp TEXT NOT NULL,
                change_type TEXT NOT NULL,
                file_path TEXT NOT NULL,
                description TEXT,
                backup_hash TEXT,
                file_size INTEGER,
                project_path TEXT,
                inline_content BLOB
            );

            CREATE INDEX IF NOT EXISTS idx_timestamp ON entries(timestamp DESC);
            CREATE INDEX IF NOT EXISTS idx_file_path ON entries(file_path);
            CREATE INDEX IF NOT EXISTS idx_change_type ON entries(change_type);

            CREATE TABLE IF NOT EXISTS checkpoints (
                id TEXT PRIMARY KEY,
                name TEXT NOT NULL UNIQUE,
                timestamp TEXT NOT NULL,
                description TEXT
            );

            CREATE INDEX IF NOT EXISTS idx_checkpoint_name ON checkpoints(name);
            CREATE INDEX IF NOT EXISTS idx_checkpoint_timestamp ON checkpoints(timestamp DESC);
            "#,
        )?;
        Ok(())
    }

    /// Generate a unique ID
    fn generate_id(&self, seed: &str) -> String {
        let mut hasher = Sha256::new();
        hasher.update(seed.as_bytes());
        hasher.update(Utc::now().to_rfc3339().as_bytes());
        hex::encode(&hasher.finalize()[..6])
    }

    /// Compute hash for file content
    fn compute_hash(content: &[u8]) -> String {
        let mut hasher = Sha256::new();
        hasher.update(content);
        hex::encode(&hasher.finalize()[..8])
    }

    /// Compress content with gzip
    fn compress(content: &[u8]) -> Result<Vec<u8>> {
        let mut encoder = GzEncoder::new(Vec::new(), Compression::fast());
        encoder.write_all(content)?;
        Ok(encoder.finish()?)
    }

    /// Decompress gzip content
    fn decompress(compressed: &[u8]) -> Result<Vec<u8>> {
        let mut decoder = GzDecoder::new(compressed);
        let mut content = Vec::new();
        decoder.read_to_end(&mut content)?;
        Ok(content)
    }

    /// Backup a file and return its hash
    pub fn backup_file(&self, file_path: &Path) -> Result<Option<String>> {
        if !file_path.exists() {
            return Ok(None);
        }

        let content = fs::read(file_path)
            .with_context(|| format!("Failed to read file: {}", file_path.display()))?;

        let hash = Self::compute_hash(&content);
        let compressed = Self::compress(&content)?;

        // For large files, store on disk
        if content.len() >= INLINE_THRESHOLD {
            let backup_path = self.backup_dir.join(&hash);
            if !backup_path.exists() {
                fs::write(&backup_path, &compressed)
                    .with_context(|| format!("Failed to write backup: {}", backup_path.display()))?;
            }
        }

        Ok(Some(hash))
    }

    /// Get backup content for a hash
    fn get_backup_content(&self, entry_id: &str, hash: &str) -> Result<Option<Vec<u8>>> {
        // First try inline storage
        let inline: Option<Vec<u8>> = self.conn.query_row(
            "SELECT inline_content FROM entries WHERE id = ? AND inline_content IS NOT NULL",
            params![entry_id],
            |row| row.get(0),
        ).optional()?;

        if let Some(compressed) = inline {
            return Ok(Some(Self::decompress(&compressed)?));
        }

        // Try disk storage
        let backup_path = self.backup_dir.join(hash);
        if backup_path.exists() {
            let compressed = fs::read(&backup_path)?;
            return Ok(Some(Self::decompress(&compressed)?));
        }

        Ok(None)
    }

    /// Add an entry to the timeline
    pub fn add_entry(&self, entry: &UndoEntry, content: Option<&[u8]>) -> Result<()> {
        let inline_content = if let Some(c) = content {
            if c.len() < INLINE_THRESHOLD {
                Some(Self::compress(c)?)
            } else {
                None
            }
        } else {
            None
        };

        self.conn.execute(
            r#"
            INSERT OR REPLACE INTO entries
            (id, timestamp, change_type, file_path, description, backup_hash, file_size, project_path, inline_content)
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
            "#,
            params![
                entry.id,
                entry.timestamp.to_rfc3339(),
                entry.change_type.as_str(),
                entry.file_path,
                entry.description,
                entry.backup_hash,
                entry.file_size,
                entry.project_path,
                inline_content,
            ],
        )?;
        Ok(())
    }

    /// Get timeline entries
    pub fn get_entries(&self, limit: u32, file_path: Option<&str>) -> Result<Vec<UndoEntry>> {
        let mut entries = Vec::new();

        if let Some(path) = file_path {
            let mut stmt = self.conn.prepare(
                "SELECT id, timestamp, change_type, file_path, description, backup_hash, file_size, project_path
                 FROM entries WHERE file_path = ? ORDER BY timestamp DESC LIMIT ?"
            )?;
            let rows = stmt.query_map(params![path, limit], |row| {
                Ok(UndoEntry {
                    id: row.get(0)?,
                    timestamp: DateTime::parse_from_rfc3339(&row.get::<_, String>(1)?)
                        .map(|dt| dt.with_timezone(&Utc))
                        .unwrap_or_else(|_| Utc::now()),
                    change_type: ChangeType::from_str(&row.get::<_, String>(2)?).unwrap_or(ChangeType::Edit),
                    file_path: row.get(3)?,
                    description: row.get(4)?,
                    backup_hash: row.get(5)?,
                    file_size: row.get(6)?,
                    project_path: row.get(7)?,
                })
            })?;
            for row in rows {
                entries.push(row?);
            }
        } else {
            let mut stmt = self.conn.prepare(
                "SELECT id, timestamp, change_type, file_path, description, backup_hash, file_size, project_path
                 FROM entries ORDER BY timestamp DESC LIMIT ?"
            )?;
            let rows = stmt.query_map(params![limit], |row| {
                Ok(UndoEntry {
                    id: row.get(0)?,
                    timestamp: DateTime::parse_from_rfc3339(&row.get::<_, String>(1)?)
                        .map(|dt| dt.with_timezone(&Utc))
                        .unwrap_or_else(|_| Utc::now()),
                    change_type: ChangeType::from_str(&row.get::<_, String>(2)?).unwrap_or(ChangeType::Edit),
                    file_path: row.get(3)?,
                    description: row.get(4)?,
                    backup_hash: row.get(5)?,
                    file_size: row.get(6)?,
                    project_path: row.get(7)?,
                })
            })?;
            for row in rows {
                entries.push(row?);
            }
        }

        Ok(entries)
    }

    /// Get a specific entry by ID
    pub fn get_entry(&self, entry_id: &str) -> Result<Option<UndoEntry>> {
        self.conn.query_row(
            "SELECT id, timestamp, change_type, file_path, description, backup_hash, file_size, project_path
             FROM entries WHERE id = ?",
            params![entry_id],
            |row| {
                Ok(UndoEntry {
                    id: row.get(0)?,
                    timestamp: DateTime::parse_from_rfc3339(&row.get::<_, String>(1)?)
                        .map(|dt| dt.with_timezone(&Utc))
                        .unwrap_or_else(|_| Utc::now()),
                    change_type: ChangeType::from_str(&row.get::<_, String>(2)?).unwrap_or(ChangeType::Edit),
                    file_path: row.get(3)?,
                    description: row.get(4)?,
                    backup_hash: row.get(5)?,
                    file_size: row.get(6)?,
                    project_path: row.get(7)?,
                })
            },
        ).optional().map_err(Into::into)
    }

    /// Restore a file from backup
    pub fn restore_file(&self, entry: &UndoEntry) -> Result<bool> {
        let hash = match &entry.backup_hash {
            Some(h) => h,
            None => return Ok(false),
        };

        let content = match self.get_backup_content(&entry.id, hash)? {
            Some(c) => c,
            None => return Ok(false),
        };

        let file_path = Path::new(&entry.file_path);
        if let Some(parent) = file_path.parent() {
            fs::create_dir_all(parent)?;
        }

        fs::write(file_path, content)
            .with_context(|| format!("Failed to restore file: {}", file_path.display()))?;

        Ok(true)
    }

    /// Create a named checkpoint
    pub fn create_checkpoint(&mut self, name: &str, description: &str) -> Result<String> {
        let id = self.generate_id(name);
        let timestamp = Utc::now();

        self.conn.execute(
            "INSERT OR REPLACE INTO checkpoints (id, name, timestamp, description) VALUES (?, ?, ?, ?)",
            params![id, name, timestamp.to_rfc3339(), description],
        )?;

        // Also add as a timeline entry
        let entry = UndoEntry {
            id: id.clone(),
            timestamp,
            change_type: ChangeType::Checkpoint,
            file_path: String::new(),
            description: format!("Checkpoint: {}", name),
            backup_hash: None,
            file_size: 0,
            project_path: String::new(),
        };
        self.add_entry(&entry, None)?;

        Ok(id)
    }

    /// Get checkpoint by name or ID
    pub fn get_checkpoint(&self, name_or_id: &str) -> Result<Option<Checkpoint>> {
        self.conn.query_row(
            "SELECT id, name, timestamp, description FROM checkpoints WHERE id = ? OR name = ? ORDER BY timestamp DESC LIMIT 1",
            params![name_or_id, name_or_id],
            |row| {
                Ok(Checkpoint {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    timestamp: DateTime::parse_from_rfc3339(&row.get::<_, String>(2)?)
                        .map(|dt| dt.with_timezone(&Utc))
                        .unwrap_or_else(|_| Utc::now()),
                    description: row.get(3)?,
                })
            },
        ).optional().map_err(Into::into)
    }

    /// List all checkpoints
    pub fn list_checkpoints(&self, limit: u32) -> Result<Vec<Checkpoint>> {
        let mut checkpoints = Vec::new();
        let mut stmt = self.conn.prepare(
            "SELECT id, name, timestamp, description FROM checkpoints ORDER BY timestamp DESC LIMIT ?"
        )?;
        let rows = stmt.query_map(params![limit], |row| {
            Ok(Checkpoint {
                id: row.get(0)?,
                name: row.get(1)?,
                timestamp: DateTime::parse_from_rfc3339(&row.get::<_, String>(2)?)
                    .map(|dt| dt.with_timezone(&Utc))
                    .unwrap_or_else(|_| Utc::now()),
                description: row.get(3)?,
            })
        })?;
        for row in rows {
            checkpoints.push(row?);
        }
        Ok(checkpoints)
    }

    /// Record a file change (backup current state before change)
    pub fn record_change(&mut self, file_path: &Path, change_type: ChangeType, description: &str) -> Result<String> {
        let id = self.generate_id(&file_path.to_string_lossy());
        let timestamp = Utc::now();

        // Read file content before change (if file exists)
        let (content, file_size, backup_hash) = if file_path.exists() {
            let content = fs::read(file_path)?;
            let size = content.len() as i64;
            let hash = self.backup_file(file_path)?;
            (Some(content), size, hash)
        } else {
            (None, 0, None)
        };

        let project_path = file_path
            .parent()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_default();

        let entry = UndoEntry {
            id: id.clone(),
            timestamp,
            change_type,
            file_path: file_path.to_string_lossy().to_string(),
            description: description.to_string(),
            backup_hash,
            file_size,
            project_path,
        };

        self.add_entry(&entry, content.as_deref())?;
        Ok(id)
    }

    /// Get entries since a checkpoint
    pub fn get_entries_since_checkpoint(&self, checkpoint_name: &str) -> Result<Vec<UndoEntry>> {
        let checkpoint = self.get_checkpoint(checkpoint_name)?;
        match checkpoint {
            Some(cp) => {
                let mut entries = Vec::new();
                let mut stmt = self.conn.prepare(
                    "SELECT id, timestamp, change_type, file_path, description, backup_hash, file_size, project_path
                     FROM entries WHERE timestamp > ? AND change_type != 'checkpoint' ORDER BY timestamp DESC"
                )?;
                let rows = stmt.query_map(params![cp.timestamp.to_rfc3339()], |row| {
                    Ok(UndoEntry {
                        id: row.get(0)?,
                        timestamp: DateTime::parse_from_rfc3339(&row.get::<_, String>(1)?)
                            .map(|dt| dt.with_timezone(&Utc))
                            .unwrap_or_else(|_| Utc::now()),
                        change_type: ChangeType::from_str(&row.get::<_, String>(2)?).unwrap_or(ChangeType::Edit),
                        file_path: row.get(3)?,
                        description: row.get(4)?,
                        backup_hash: row.get(5)?,
                        file_size: row.get(6)?,
                        project_path: row.get(7)?,
                    })
                })?;
                for row in rows {
                    entries.push(row?);
                }
                Ok(entries)
            }
            None => Ok(Vec::new()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_database_creation() -> Result<()> {
        let tmp = TempDir::new()?;
        let _db = UndoDatabase::open(tmp.path())?;
        assert!(tmp.path().join("timeline.db").exists());
        assert!(tmp.path().join("backups").is_dir());
        Ok(())
    }

    #[test]
    fn test_checkpoint() -> Result<()> {
        let tmp = TempDir::new()?;
        let mut db = UndoDatabase::open(tmp.path())?;

        let id = db.create_checkpoint("test-checkpoint", "Test description")?;
        assert!(!id.is_empty());

        let cp = db.get_checkpoint("test-checkpoint")?;
        assert!(cp.is_some());
        assert_eq!(cp.unwrap().name, "test-checkpoint");
        Ok(())
    }

    #[test]
    fn test_compress_decompress() -> Result<()> {
        let original = b"Hello, world! This is some test content.";
        let compressed = UndoDatabase::compress(original)?;
        let decompressed = UndoDatabase::decompress(&compressed)?;
        assert_eq!(original.as_slice(), decompressed.as_slice());
        Ok(())
    }
}
