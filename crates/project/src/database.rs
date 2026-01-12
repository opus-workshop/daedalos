//! SQLite database operations for project index

use std::collections::HashMap;
use std::path::Path;

use anyhow::{Context, Result};
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};

const SCHEMA: &str = r#"
CREATE TABLE IF NOT EXISTS files (
    id INTEGER PRIMARY KEY,
    path TEXT UNIQUE NOT NULL,
    type TEXT,
    size INTEGER,
    lines INTEGER,
    modified REAL,
    hash TEXT
);

CREATE TABLE IF NOT EXISTS symbols (
    id INTEGER PRIMARY KEY,
    file_id INTEGER REFERENCES files(id),
    name TEXT NOT NULL,
    type TEXT,
    line_start INTEGER,
    line_end INTEGER,
    signature TEXT,
    visibility TEXT
);

CREATE TABLE IF NOT EXISTS dependencies (
    id INTEGER PRIMARY KEY,
    source_file_id INTEGER REFERENCES files(id),
    target_path TEXT,
    target_file_id INTEGER,
    import_type TEXT
);

CREATE TABLE IF NOT EXISTS conventions (
    id INTEGER PRIMARY KEY,
    pattern TEXT,
    type TEXT,
    occurrences INTEGER,
    examples TEXT
);

CREATE TABLE IF NOT EXISTS metadata (
    key TEXT PRIMARY KEY,
    value TEXT
);

CREATE INDEX IF NOT EXISTS idx_files_path ON files(path);
CREATE INDEX IF NOT EXISTS idx_symbols_name ON symbols(name);
CREATE INDEX IF NOT EXISTS idx_symbols_file ON symbols(file_id);
CREATE INDEX IF NOT EXISTS idx_symbols_type ON symbols(type);
CREATE INDEX IF NOT EXISTS idx_deps_source ON dependencies(source_file_id);
CREATE INDEX IF NOT EXISTS idx_deps_target ON dependencies(target_file_id);
"#;

/// Database statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Stats {
    pub file_count: i64,
    pub symbol_count: i64,
    pub dependency_count: i64,
    pub lines_by_type: HashMap<String, i64>,
}

/// File record from the database
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileRecord {
    pub id: i64,
    pub path: String,
    pub file_type: String,
    pub size: i64,
    pub lines: i64,
    pub modified: f64,
    pub hash: String,
}

/// Symbol record with file path
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SymbolRecord {
    pub id: i64,
    pub file_id: i64,
    pub name: String,
    pub symbol_type: String,
    pub line_start: i64,
    pub line_end: i64,
    pub signature: Option<String>,
    pub visibility: Option<String>,
    pub file_path: String,
}

/// Project database wrapper
pub struct ProjectDatabase {
    conn: Connection,
}

impl ProjectDatabase {
    /// Create or open a database at the given path
    pub fn new(db_path: &Path) -> Result<Self> {
        // Ensure parent directory exists
        if let Some(parent) = db_path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let conn = Connection::open(db_path).context("Failed to open database")?;
        conn.execute_batch(SCHEMA)
            .context("Failed to initialize schema")?;

        Ok(Self { conn })
    }

    /// Upsert a file record
    pub fn upsert_file(
        &self,
        path: &str,
        file_type: &str,
        size: i64,
        lines: i64,
        modified: f64,
        hash: &str,
    ) -> Result<i64> {
        self.conn.execute(
            r#"
            INSERT INTO files (path, type, size, lines, modified, hash)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6)
            ON CONFLICT(path) DO UPDATE SET
                type = excluded.type,
                size = excluded.size,
                lines = excluded.lines,
                modified = excluded.modified,
                hash = excluded.hash
            "#,
            params![path, file_type, size, lines, modified, hash],
        )?;

        let id: i64 = self
            .conn
            .query_row("SELECT id FROM files WHERE path = ?1", params![path], |row| {
                row.get(0)
            })?;

        Ok(id)
    }

    /// Get a file by path
    pub fn get_file(&self, path: &str) -> Result<Option<FileRecord>> {
        let mut stmt = self
            .conn
            .prepare("SELECT id, path, type, size, lines, modified, hash FROM files WHERE path = ?1")?;

        let mut rows = stmt.query(params![path])?;

        if let Some(row) = rows.next()? {
            Ok(Some(FileRecord {
                id: row.get(0)?,
                path: row.get(1)?,
                file_type: row.get::<_, Option<String>>(2)?.unwrap_or_default(),
                size: row.get(3)?,
                lines: row.get(4)?,
                modified: row.get(5)?,
                hash: row.get::<_, Option<String>>(6)?.unwrap_or_default(),
            }))
        } else {
            Ok(None)
        }
    }

    /// Get all files
    pub fn get_all_files(&self) -> Result<Vec<FileRecord>> {
        let mut stmt = self
            .conn
            .prepare("SELECT id, path, type, size, lines, modified, hash FROM files ORDER BY path")?;

        let rows = stmt.query_map([], |row| {
            Ok(FileRecord {
                id: row.get(0)?,
                path: row.get(1)?,
                file_type: row.get::<_, Option<String>>(2)?.unwrap_or_default(),
                size: row.get(3)?,
                lines: row.get(4)?,
                modified: row.get(5)?,
                hash: row.get::<_, Option<String>>(6)?.unwrap_or_default(),
            })
        })?;

        let mut files = Vec::new();
        for row in rows {
            files.push(row?);
        }
        Ok(files)
    }

    /// Add a symbol
    pub fn add_symbol(
        &self,
        file_id: i64,
        name: &str,
        symbol_type: &str,
        line_start: i64,
        line_end: i64,
        signature: Option<&str>,
        visibility: Option<&str>,
    ) -> Result<i64> {
        self.conn.execute(
            r#"
            INSERT INTO symbols (file_id, name, type, line_start, line_end, signature, visibility)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
            "#,
            params![file_id, name, symbol_type, line_start, line_end, signature, visibility],
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    /// Get all symbols with file paths
    pub fn get_all_symbols(&self) -> Result<Vec<SymbolRecord>> {
        let mut stmt = self.conn.prepare(
            r#"
            SELECT s.id, s.file_id, s.name, s.type, s.line_start, s.line_end,
                   s.signature, s.visibility, f.path
            FROM symbols s
            JOIN files f ON s.file_id = f.id
            ORDER BY s.name
            "#,
        )?;

        let rows = stmt.query_map([], |row| {
            Ok(SymbolRecord {
                id: row.get(0)?,
                file_id: row.get(1)?,
                name: row.get(2)?,
                symbol_type: row.get::<_, Option<String>>(3)?.unwrap_or_default(),
                line_start: row.get(4)?,
                line_end: row.get(5)?,
                signature: row.get(6)?,
                visibility: row.get(7)?,
                file_path: row.get(8)?,
            })
        })?;

        let mut symbols = Vec::new();
        for row in rows {
            symbols.push(row?);
        }
        Ok(symbols)
    }

    /// Search symbols by name pattern
    pub fn search_symbols(&self, pattern: &str) -> Result<Vec<SymbolRecord>> {
        let mut stmt = self.conn.prepare(
            r#"
            SELECT s.id, s.file_id, s.name, s.type, s.line_start, s.line_end,
                   s.signature, s.visibility, f.path
            FROM symbols s
            JOIN files f ON s.file_id = f.id
            WHERE s.name LIKE ?1
            ORDER BY s.name
            "#,
        )?;

        let pattern = format!("%{}%", pattern);
        let rows = stmt.query_map(params![pattern], |row| {
            Ok(SymbolRecord {
                id: row.get(0)?,
                file_id: row.get(1)?,
                name: row.get(2)?,
                symbol_type: row.get::<_, Option<String>>(3)?.unwrap_or_default(),
                line_start: row.get(4)?,
                line_end: row.get(5)?,
                signature: row.get(6)?,
                visibility: row.get(7)?,
                file_path: row.get(8)?,
            })
        })?;

        let mut symbols = Vec::new();
        for row in rows {
            symbols.push(row?);
        }
        Ok(symbols)
    }

    /// Clear symbols for a file
    pub fn clear_symbols_for_file(&self, file_id: i64) -> Result<()> {
        self.conn
            .execute("DELETE FROM symbols WHERE file_id = ?1", params![file_id])?;
        Ok(())
    }

    /// Add a dependency
    pub fn add_dependency(&self, source_file_id: i64, target_path: &str, import_type: &str) -> Result<i64> {
        // Try to resolve target file ID
        let target_file_id: Option<i64> = self
            .conn
            .query_row(
                "SELECT id FROM files WHERE path = ?1 OR path LIKE ?2",
                params![target_path, format!("%/{}", target_path)],
                |row| row.get(0),
            )
            .ok();

        self.conn.execute(
            r#"
            INSERT INTO dependencies (source_file_id, target_path, target_file_id, import_type)
            VALUES (?1, ?2, ?3, ?4)
            "#,
            params![source_file_id, target_path, target_file_id, import_type],
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    /// Get dependencies for a file
    pub fn get_file_dependencies(&self, file_path: &str) -> Result<Vec<String>> {
        let mut stmt = self.conn.prepare(
            r#"
            SELECT d.target_path
            FROM dependencies d
            JOIN files f ON d.source_file_id = f.id
            WHERE f.path = ?1
            "#,
        )?;

        let rows = stmt.query_map(params![file_path], |row| row.get::<_, String>(0))?;

        let mut deps = Vec::new();
        for row in rows {
            deps.push(row?);
        }
        Ok(deps)
    }

    /// Get files that depend on this file
    pub fn get_file_dependents(&self, file_path: &str) -> Result<Vec<String>> {
        // Get file ID first
        let file_id: Option<i64> = self
            .conn
            .query_row(
                "SELECT id FROM files WHERE path = ?1",
                params![file_path],
                |row| row.get(0),
            )
            .ok();

        let file_id = match file_id {
            Some(id) => id,
            None => return Ok(Vec::new()),
        };

        let mut stmt = self.conn.prepare(
            r#"
            SELECT f.path
            FROM dependencies d
            JOIN files f ON d.source_file_id = f.id
            WHERE d.target_file_id = ?1
            "#,
        )?;

        let rows = stmt.query_map(params![file_id], |row| row.get::<_, String>(0))?;

        let mut deps = Vec::new();
        for row in rows {
            deps.push(row?);
        }
        Ok(deps)
    }

    /// Get external dependencies (unresolved imports)
    pub fn get_external_dependencies(&self) -> Result<Vec<String>> {
        let mut stmt = self.conn.prepare(
            r#"
            SELECT DISTINCT target_path
            FROM dependencies
            WHERE target_file_id IS NULL
              AND target_path NOT LIKE '.%'
              AND target_path NOT LIKE '%/%'
            ORDER BY target_path
            "#,
        )?;

        let rows = stmt.query_map([], |row| row.get::<_, String>(0))?;

        let mut deps = Vec::new();
        for row in rows {
            deps.push(row?);
        }
        Ok(deps)
    }

    /// Clear dependencies for a file
    pub fn clear_dependencies_for_file(&self, file_id: i64) -> Result<()> {
        self.conn.execute(
            "DELETE FROM dependencies WHERE source_file_id = ?1",
            params![file_id],
        )?;
        Ok(())
    }

    /// Get all unique directory names from files
    pub fn get_directories(&self) -> Result<Vec<String>> {
        let files = self.get_all_files()?;
        let mut dirs: std::collections::HashSet<String> = std::collections::HashSet::new();

        for file in &files {
            let parts: Vec<_> = file.path.split('/').collect();
            for part in parts.iter().take(parts.len().saturating_sub(1)) {
                dirs.insert(part.to_lowercase());
            }
        }

        Ok(dirs.into_iter().collect())
    }

    /// Set metadata value
    pub fn set_metadata(&self, key: &str, value: &str) -> Result<()> {
        self.conn.execute(
            "INSERT OR REPLACE INTO metadata (key, value) VALUES (?1, ?2)",
            params![key, value],
        )?;
        Ok(())
    }

    /// Get metadata value
    pub fn get_metadata(&self, key: &str) -> Result<Option<String>> {
        let result: Result<String, _> = self.conn.query_row(
            "SELECT value FROM metadata WHERE key = ?1",
            params![key],
            |row| row.get(0),
        );
        Ok(result.ok())
    }

    /// Get database statistics
    pub fn get_stats(&self) -> Result<Stats> {
        let file_count: i64 = self
            .conn
            .query_row("SELECT COUNT(*) FROM files", [], |row| row.get(0))?;

        let symbol_count: i64 = self
            .conn
            .query_row("SELECT COUNT(*) FROM symbols", [], |row| row.get(0))?;

        let dependency_count: i64 = self
            .conn
            .query_row("SELECT COUNT(*) FROM dependencies", [], |row| row.get(0))?;

        let mut stmt = self
            .conn
            .prepare("SELECT type, SUM(lines) FROM files GROUP BY type")?;

        let rows = stmt.query_map([], |row| {
            Ok((
                row.get::<_, Option<String>>(0)?.unwrap_or_default(),
                row.get::<_, i64>(1)?,
            ))
        })?;

        let mut lines_by_type = HashMap::new();
        for row in rows {
            let (file_type, lines) = row?;
            if !file_type.is_empty() {
                lines_by_type.insert(file_type, lines);
            }
        }

        Ok(Stats {
            file_count,
            symbol_count,
            dependency_count,
            lines_by_type,
        })
    }
}
