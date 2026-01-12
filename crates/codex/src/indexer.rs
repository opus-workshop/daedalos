//! Index management for semantic code search.
//!
//! Manages the SQLite database that stores code chunks and their embeddings.

use anyhow::Result;
use ignore::WalkBuilder;
use rusqlite::{params, Connection};
use sha2::{Digest, Sha256};
use std::fs;
use std::path::{Path, PathBuf};

use crate::chunker::CodeChunker;

/// File extensions to index
const INDEXABLE_EXTENSIONS: &[&str] = &[
    // Languages
    "py", "pyw",                          // Python
    "js", "jsx", "mjs", "cjs",            // JavaScript
    "ts", "tsx", "mts", "cts",            // TypeScript
    "swift",                               // Swift
    "rs",                                  // Rust
    "go",                                  // Go
    "rb", "rake",                          // Ruby
    "java", "kt", "kts",                   // JVM
    "c", "h", "cpp", "hpp", "cc", "cxx",  // C/C++
    "cs",                                  // C#
    "php",                                 // PHP
    "lua",                                 // Lua
    "sh", "bash", "zsh", "fish",          // Shell
    "pl", "pm",                            // Perl
    "r",                                   // R
    "scala", "sc",                         // Scala
    "ex", "exs",                           // Elixir
    "erl", "hrl",                          // Erlang
    "hs", "lhs",                           // Haskell
    "ml", "mli",                           // OCaml
    "clj", "cljs", "cljc",                // Clojure
    "nim",                                 // Nim
    "zig",                                 // Zig
    "v",                                   // V
    "dart",                                // Dart
    "vue", "svelte",                       // Frontend frameworks
    // Config/Data
    "json", "yaml", "yml", "toml",
    "xml", "html", "css", "scss", "sass",
    "sql",
    "md", "txt", "rst",
    "nix", "dhall",
];

/// Maximum file size to index (500KB)
const MAX_FILE_SIZE: u64 = 500_000;

/// Manages the code index for a project
pub struct CodeIndex {
    /// Project root path
    pub project_path: PathBuf,
    /// Path to the SQLite database
    pub db_path: PathBuf,
    /// Code chunker
    chunker: CodeChunker,
    /// Database connection
    conn: Option<Connection>,
}

impl CodeIndex {
    /// Create a new index for a project
    pub fn new(project_path: impl AsRef<Path>) -> Result<Self> {
        let project_path = project_path.as_ref().canonicalize()
            .unwrap_or_else(|_| project_path.as_ref().to_path_buf());

        let index_dir = dirs::data_dir()
            .unwrap_or_else(|| PathBuf::from("~/.local/share"))
            .join("daedalos")
            .join("codex");

        let hash = Self::hash_path(&project_path);
        let db_path = index_dir.join(format!("{}.db", hash));

        Ok(Self {
            project_path,
            db_path,
            chunker: CodeChunker::new(),
            conn: None,
        })
    }

    /// Generate a unique hash for the project path
    fn hash_path(path: &Path) -> String {
        let mut hasher = Sha256::new();
        hasher.update(path.to_string_lossy().as_bytes());
        let result = hasher.finalize();
        hex::encode(&result[..8])
    }

    /// Hash file contents
    fn hash_content(content: &[u8]) -> String {
        let mut hasher = Sha256::new();
        hasher.update(content);
        let result = hasher.finalize();
        hex::encode(&result[..8])
    }

    /// Get or create database connection
    fn conn(&mut self) -> Result<&mut Connection> {
        if self.conn.is_none() {
            self.init_db()?;
        }
        Ok(self.conn.as_mut().unwrap())
    }

    /// Initialize the database
    fn init_db(&mut self) -> Result<()> {
        // Create directory if needed
        if let Some(parent) = self.db_path.parent() {
            fs::create_dir_all(parent)?;
        }

        let conn = Connection::open(&self.db_path)?;

        conn.execute_batch(r#"
            CREATE TABLE IF NOT EXISTS chunks (
                id INTEGER PRIMARY KEY,
                file_path TEXT NOT NULL,
                start_line INTEGER NOT NULL,
                end_line INTEGER NOT NULL,
                content TEXT NOT NULL,
                chunk_type TEXT NOT NULL,
                name TEXT NOT NULL,
                file_hash TEXT,
                created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
            );

            CREATE INDEX IF NOT EXISTS idx_chunks_file ON chunks(file_path);
            CREATE INDEX IF NOT EXISTS idx_chunks_hash ON chunks(file_hash);

            CREATE TABLE IF NOT EXISTS metadata (
                key TEXT PRIMARY KEY,
                value TEXT
            );

            CREATE TABLE IF NOT EXISTS file_hashes (
                file_path TEXT PRIMARY KEY,
                hash TEXT NOT NULL,
                indexed_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
            );

            -- FTS5 for keyword search
            CREATE VIRTUAL TABLE IF NOT EXISTS chunks_fts USING fts5(
                content,
                name,
                chunk_type,
                file_path,
                content='chunks',
                content_rowid='id'
            );

            -- Triggers to keep FTS in sync
            CREATE TRIGGER IF NOT EXISTS chunks_ai AFTER INSERT ON chunks BEGIN
                INSERT INTO chunks_fts(rowid, content, name, chunk_type, file_path)
                VALUES (new.id, new.content, new.name, new.chunk_type, new.file_path);
            END;

            CREATE TRIGGER IF NOT EXISTS chunks_ad AFTER DELETE ON chunks BEGIN
                INSERT INTO chunks_fts(chunks_fts, rowid, content, name, chunk_type, file_path)
                VALUES ('delete', old.id, old.content, old.name, old.chunk_type, old.file_path);
            END;

            CREATE TRIGGER IF NOT EXISTS chunks_au AFTER UPDATE ON chunks BEGIN
                INSERT INTO chunks_fts(chunks_fts, rowid, content, name, chunk_type, file_path)
                VALUES ('delete', old.id, old.content, old.name, old.chunk_type, old.file_path);
                INSERT INTO chunks_fts(rowid, content, name, chunk_type, file_path)
                VALUES (new.id, new.content, new.name, new.chunk_type, new.file_path);
            END;
        "#)?;

        self.conn = Some(conn);
        Ok(())
    }

    /// Check if the project has been indexed
    pub fn is_indexed(&mut self) -> bool {
        if !self.db_path.exists() {
            return false;
        }

        if let Ok(conn) = self.conn() {
            if let Ok(count) = conn.query_row::<i64, _, _>(
                "SELECT COUNT(*) FROM chunks",
                [],
                |row| row.get(0),
            ) {
                return count > 0;
            }
        }

        false
    }

    /// Check if a file should be indexed
    fn should_index(&self, path: &Path) -> bool {
        // Check extension
        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_lowercase();

        if !INDEXABLE_EXTENSIONS.contains(&ext.as_str()) {
            return false;
        }

        // Skip hidden files
        if path
            .file_name()
            .and_then(|n| n.to_str())
            .map(|n| n.starts_with('.'))
            .unwrap_or(false)
        {
            return false;
        }

        // Skip large files
        if let Ok(metadata) = path.metadata() {
            if metadata.len() > MAX_FILE_SIZE {
                return false;
            }
        }

        true
    }

    /// Check if a file needs re-indexing
    fn needs_reindex(&mut self, path: &Path, content: &[u8]) -> Result<bool> {
        let rel_path = path
            .strip_prefix(&self.project_path)
            .unwrap_or(path)
            .to_string_lossy();

        let current_hash = Self::hash_content(content);
        let conn = self.conn()?;

        let stored_hash: Option<String> = conn
            .query_row(
                "SELECT hash FROM file_hashes WHERE file_path = ?",
                [&rel_path],
                |row| row.get(0),
            )
            .ok();

        Ok(stored_hash.as_ref() != Some(&current_hash))
    }

    /// Get list of files to index
    fn get_indexable_files(&self) -> Vec<PathBuf> {
        let mut files = Vec::new();

        // Use ignore crate to respect .gitignore
        let walker = WalkBuilder::new(&self.project_path)
            .hidden(true)        // Skip hidden files
            .git_ignore(true)    // Respect .gitignore
            .git_global(true)    // Respect global gitignore
            .git_exclude(true)   // Respect .git/info/exclude
            .build();

        for entry in walker.flatten() {
            let path = entry.path();
            if path.is_file() && self.should_index(path) {
                files.push(path.to_path_buf());
            }
        }

        files.sort();
        files
    }

    /// Index the entire project
    pub fn index_project(&mut self, force: bool) -> Result<IndexStats> {
        let files = self.get_indexable_files();
        let total = files.len();

        if total == 0 {
            return Ok(IndexStats {
                files_indexed: 0,
                chunks_created: 0,
                skipped: 0,
            });
        }

        let mut indexed = 0;
        let mut chunks_created = 0;
        let mut skipped = 0;

        // Collect files that need indexing
        let mut files_to_index = Vec::new();
        for path in &files {
            if let Ok(content) = fs::read(path) {
                if force || self.needs_reindex(path, &content)? {
                    files_to_index.push((path.clone(), content));
                } else {
                    skipped += 1;
                }
            }
        }

        if files_to_index.is_empty() {
            return Ok(IndexStats {
                files_indexed: 0,
                chunks_created: 0,
                skipped,
            });
        }

        eprintln!("Indexing {} files...", files_to_index.len());

        for (i, (path, content)) in files_to_index.iter().enumerate() {
            match self.index_file(path, content) {
                Ok(count) => {
                    indexed += 1;
                    chunks_created += count;
                }
                Err(e) => {
                    eprintln!("  Error indexing {}: {}", path.display(), e);
                }
            }

            // Progress indicator
            if (i + 1) % 10 == 0 || i + 1 == files_to_index.len() {
                let pct = ((i + 1) as f64 / files_to_index.len() as f64) * 100.0;
                eprint!("\r  [{:5.1}%] {}/{} files", pct, i + 1, files_to_index.len());
            }
        }
        eprintln!();

        // Save metadata
        let conn = self.conn()?;
        conn.execute(
            "INSERT OR REPLACE INTO metadata (key, value) VALUES (?, ?)",
            params!["backend", "keyword"],
        )?;

        Ok(IndexStats {
            files_indexed: indexed,
            chunks_created,
            skipped,
        })
    }

    /// Index a single file
    fn index_file(&mut self, path: &Path, content: &[u8]) -> Result<usize> {
        let content_str = String::from_utf8_lossy(content);
        let rel_path = path
            .strip_prefix(&self.project_path)
            .unwrap_or(path)
            .to_string_lossy()
            .to_string();

        let file_hash = Self::hash_content(content);

        // Remove old chunks for this file
        {
            let conn = self.conn()?;
            conn.execute("DELETE FROM chunks WHERE file_path = ?", [&rel_path])?;
        }

        // Chunk the file
        let chunks = self.chunker.chunk_file(&rel_path, &content_str);
        let chunk_count = chunks.len();

        // Insert new chunks
        for chunk in chunks {
            let conn = self.conn()?;
            conn.execute(
                r#"
                INSERT INTO chunks (file_path, start_line, end_line, content, chunk_type, name, file_hash)
                VALUES (?, ?, ?, ?, ?, ?, ?)
                "#,
                params![
                    chunk.file_path,
                    chunk.start_line as i64,
                    chunk.end_line as i64,
                    chunk.content,
                    chunk.chunk_type,
                    chunk.name,
                    file_hash,
                ],
            )?;
        }

        // Update file hash
        let conn = self.conn()?;
        conn.execute(
            "INSERT OR REPLACE INTO file_hashes (file_path, hash) VALUES (?, ?)",
            params![rel_path, file_hash],
        )?;

        Ok(chunk_count)
    }

    /// Get index statistics
    pub fn get_stats(&mut self) -> Result<IndexStatistics> {
        let conn = self.conn()?;

        let chunk_count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM chunks",
            [],
            |row| row.get(0),
        )?;

        let file_count: i64 = conn.query_row(
            "SELECT COUNT(DISTINCT file_path) FROM chunks",
            [],
            |row| row.get(0),
        )?;

        let backend: String = conn
            .query_row(
                "SELECT value FROM metadata WHERE key = 'backend'",
                [],
                |row| row.get(0),
            )
            .unwrap_or_else(|_| "unknown".to_string());

        Ok(IndexStatistics {
            chunks: chunk_count as usize,
            files: file_count as usize,
            backend,
            db_path: self.db_path.to_string_lossy().to_string(),
            project_path: self.project_path.to_string_lossy().to_string(),
        })
    }

    /// Clear the index
    pub fn clear(&mut self) -> Result<()> {
        let conn = self.conn()?;
        conn.execute_batch(
            r#"
            DELETE FROM chunks;
            DELETE FROM file_hashes;
            DELETE FROM metadata;
            DELETE FROM chunks_fts;
            "#,
        )?;
        Ok(())
    }
}

/// Statistics from indexing operation
#[derive(Debug)]
pub struct IndexStats {
    pub files_indexed: usize,
    pub chunks_created: usize,
    pub skipped: usize,
}

/// Index statistics
#[derive(Debug)]
pub struct IndexStatistics {
    pub chunks: usize,
    pub files: usize,
    pub backend: String,
    pub db_path: String,
    pub project_path: String,
}
