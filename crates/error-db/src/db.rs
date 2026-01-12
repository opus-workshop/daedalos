//! SQLite database for error patterns and solutions
//!
//! Stores patterns with fuzzy matching support and multiple solutions per pattern.
//! Confidence scoring based on Bayesian smoothing: (success + 1) / (success + failure + 2)

use anyhow::{Context, Result};
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use uuid::Uuid;

/// An error pattern
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Pattern {
    pub id: String,
    pub pattern: String,
    pub scope: String,
    pub language: Option<String>,
    pub framework: Option<String>,
    pub tags: Vec<String>,
    pub created_at: String,
    pub updated_at: String,
}

/// A solution for a pattern
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Solution {
    pub id: String,
    pub pattern_id: String,
    pub solution: String,
    pub command: Option<String>,
    pub confidence: f64,
    pub success_count: i32,
    pub failure_count: i32,
    pub created_at: String,
    pub last_confirmed: Option<String>,
}

/// Database statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Stats {
    pub total_patterns: i32,
    pub total_solutions: i32,
    pub by_scope: std::collections::HashMap<String, i32>,
    pub by_language: std::collections::HashMap<String, i32>,
}

/// Get path to database file
pub fn get_db_path() -> PathBuf {
    let data_dir = dirs::data_dir()
        .unwrap_or_else(|| PathBuf::from("~/.local/share"))
        .join("daedalos")
        .join("error-db");

    if let Err(e) = fs::create_dir_all(&data_dir) {
        eprintln!("Warning: failed to create data directory: {}", e);
    }

    data_dir.join("errors.db")
}

/// SQLite database for error patterns
pub struct ErrorDatabase {
    conn: Connection,
}

impl ErrorDatabase {
    /// Open or create the database
    pub fn open(db_path: Option<&Path>) -> Result<Self> {
        let path = db_path.map(|p| p.to_path_buf()).unwrap_or_else(get_db_path);

        // Ensure parent directory exists
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("Failed to create database directory: {}", parent.display()))?;
        }

        let conn = Connection::open(&path)
            .with_context(|| format!("Failed to open database: {}", path.display()))?;

        let mut db = Self { conn };
        db.init_schema()?;

        // Seed if empty
        let count: i32 = db.conn.query_row(
            "SELECT COUNT(*) FROM patterns",
            [],
            |row| row.get(0),
        )?;

        if count == 0 {
            db.seed_patterns()?;
        }

        Ok(db)
    }

    /// Initialize database schema
    fn init_schema(&mut self) -> Result<()> {
        self.conn.execute_batch(
            r#"
            CREATE TABLE IF NOT EXISTS patterns (
                id TEXT PRIMARY KEY,
                pattern TEXT NOT NULL,
                scope TEXT NOT NULL DEFAULT 'global',
                language TEXT,
                framework TEXT,
                tags TEXT,
                created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
                updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
            );

            CREATE TABLE IF NOT EXISTS solutions (
                id TEXT PRIMARY KEY,
                pattern_id TEXT NOT NULL REFERENCES patterns(id),
                solution TEXT NOT NULL,
                command TEXT,
                confidence REAL DEFAULT 0.5,
                success_count INTEGER DEFAULT 0,
                failure_count INTEGER DEFAULT 0,
                created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
                last_confirmed TIMESTAMP
            );

            CREATE TABLE IF NOT EXISTS usage_log (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                pattern_id TEXT,
                solution_id TEXT,
                outcome TEXT,
                timestamp TIMESTAMP DEFAULT CURRENT_TIMESTAMP
            );

            CREATE INDEX IF NOT EXISTS idx_patterns_language ON patterns(language);
            CREATE INDEX IF NOT EXISTS idx_patterns_scope ON patterns(scope);
            CREATE INDEX IF NOT EXISTS idx_solutions_pattern ON solutions(pattern_id);
            CREATE INDEX IF NOT EXISTS idx_solutions_confidence ON solutions(confidence DESC);
            "#,
        )?;
        Ok(())
    }

    /// Seed database with common error patterns
    fn seed_patterns(&mut self) -> Result<()> {
        let seed_data = vec![
            // Node.js / npm
            SeedPattern {
                pattern: "Cannot find module 'X'",
                scope: "language",
                language: Some("javascript"),
                solution: Some("The module is not installed. Run:\n\n  npm install <module>\n\nOr if it's a dev dependency:\n\n  npm install --save-dev <module>"),
                command: Some("npm install"),
            },
            SeedPattern {
                pattern: "ENOENT: no such file or directory",
                scope: "global",
                language: None,
                solution: Some("The file or directory doesn't exist. Check:\n1. Path spelling\n2. Working directory (pwd)\n3. Whether the file was actually created"),
                command: None,
            },
            SeedPattern {
                pattern: "EADDRINUSE",
                scope: "global",
                language: None,
                solution: Some("Port is already in use. Either:\n1. Kill the process: lsof -i :PORT then kill <PID>\n2. Use a different port\n3. Wait for the port to be released"),
                command: Some("lsof -i :"),
            },
            // Python
            SeedPattern {
                pattern: "ModuleNotFoundError: No module named",
                scope: "language",
                language: Some("python"),
                solution: Some("The module is not installed. Run:\n\n  pip install <module>\n\nOr in a virtual environment:\n\n  python -m pip install <module>"),
                command: Some("pip install"),
            },
            SeedPattern {
                pattern: "IndentationError",
                scope: "language",
                language: Some("python"),
                solution: Some("Python requires consistent indentation. Check:\n1. Tabs vs spaces (use 4 spaces)\n2. Proper indentation after : characters\n3. No mixing of indentation styles"),
                command: None,
            },
            SeedPattern {
                pattern: "SyntaxError: invalid syntax",
                scope: "language",
                language: Some("python"),
                solution: Some("Python syntax error. Common causes:\n1. Missing colon after if/for/def/class\n2. Unmatched parentheses/brackets\n3. Missing quotes around strings\n4. Python 2 vs 3 incompatibility"),
                command: None,
            },
            // Rust
            SeedPattern {
                pattern: "error[E0382]: borrow of moved value",
                scope: "language",
                language: Some("rust"),
                solution: Some("Value was moved and can't be used again. Options:\n1. Clone: value.clone()\n2. Use references: &value\n3. Restructure to avoid multiple uses"),
                command: None,
            },
            SeedPattern {
                pattern: "error[E0277]: the trait bound",
                scope: "language",
                language: Some("rust"),
                solution: Some("Type doesn't implement required trait. Options:\n1. Derive the trait: #[derive(Trait)]\n2. Implement manually\n3. Use a different type that implements it"),
                command: None,
            },
            // Git
            SeedPattern {
                pattern: "fatal: not a git repository",
                scope: "global",
                language: None,
                solution: Some("You're not in a git repository. Either:\n1. cd to a git repository\n2. Initialize: git init\n3. Clone: git clone <url>"),
                command: Some("git init"),
            },
            SeedPattern {
                pattern: "Your branch is behind",
                scope: "global",
                language: None,
                solution: Some("Remote has new commits. Pull them:\n\n  git pull\n\nOr if you have local changes:\n\n  git pull --rebase"),
                command: Some("git pull"),
            },
            SeedPattern {
                pattern: "CONFLICT (content): Merge conflict",
                scope: "global",
                language: None,
                solution: Some("Files have conflicting changes. Steps:\n1. Open conflicting files\n2. Look for <<<< ==== >>>> markers\n3. Choose which changes to keep\n4. Remove markers\n5. git add <files>\n6. git commit"),
                command: None,
            },
            // TypeScript
            SeedPattern {
                pattern: "Property 'X' does not exist on type",
                scope: "language",
                language: Some("typescript"),
                solution: Some("TypeScript doesn't know about this property. Options:\n1. Add it to the type definition\n2. Use type assertion: (obj as ExtendedType).X\n3. Check if the property name is correct"),
                command: None,
            },
            SeedPattern {
                pattern: "Type 'X' is not assignable to type 'Y'",
                scope: "language",
                language: Some("typescript"),
                solution: Some("Type mismatch. Options:\n1. Fix the type of the value\n2. Update the type annotation\n3. Use a type assertion if you're sure\n4. Check if it's a nullable type issue"),
                command: None,
            },
            // Swift
            SeedPattern {
                pattern: "Cannot convert value of type",
                scope: "language",
                language: Some("swift"),
                solution: Some("Type conversion needed. Options:\n1. Cast explicitly: value as Type\n2. Initialize new type: Type(value)\n3. Check optional unwrapping\n4. Use map/compactMap for collections"),
                command: None,
            },
            // General
            SeedPattern {
                pattern: "Permission denied",
                scope: "global",
                language: None,
                solution: Some("Insufficient permissions. Options:\n1. Check file permissions: ls -la\n2. Change ownership: chown user:group file\n3. Use sudo (if appropriate)\n4. Check if file is locked/open elsewhere"),
                command: None,
            },
            SeedPattern {
                pattern: "Connection refused",
                scope: "global",
                language: None,
                solution: Some("Can't connect to the service. Check:\n1. Is the service running?\n2. Correct host/port?\n3. Firewall rules?\n4. Network connectivity?"),
                command: None,
            },
            SeedPattern {
                pattern: "command not found",
                scope: "global",
                language: None,
                solution: Some("Command is not installed or not in PATH. Options:\n1. Install the command\n2. Check spelling\n3. Add to PATH: export PATH=$PATH:/path/to/bin\n4. Use full path to command"),
                command: None,
            },
            SeedPattern {
                pattern: "out of memory",
                scope: "global",
                language: None,
                solution: Some("System ran out of memory. Options:\n1. Close other applications\n2. Increase swap space\n3. Optimize the program's memory usage\n4. Add more RAM"),
                command: None,
            },
        ];

        for p in seed_data {
            self.add_pattern(
                &p.pattern,
                p.scope,
                p.language,
                None,
                None,
                p.solution.map(|s| (s.to_string(), p.command.map(|c| c.to_string()))),
            )?;
        }

        Ok(())
    }

    /// Add a new pattern
    pub fn add_pattern(
        &mut self,
        pattern: &str,
        scope: &str,
        language: Option<&str>,
        framework: Option<&str>,
        tags: Option<Vec<&str>>,
        solution: Option<(String, Option<String>)>,
    ) -> Result<String> {
        let pattern_id = Uuid::new_v4().to_string();
        let tags_str = tags.map(|t| t.join(",")).unwrap_or_default();

        self.conn.execute(
            "INSERT INTO patterns (id, pattern, scope, language, framework, tags)
             VALUES (?, ?, ?, ?, ?, ?)",
            params![pattern_id, pattern, scope, language, framework, tags_str],
        )?;

        if let Some((sol, cmd)) = solution {
            self.add_solution(&pattern_id, &sol, cmd.as_deref())?;
        }

        Ok(pattern_id)
    }

    /// Add a solution to an existing pattern
    pub fn add_solution(
        &mut self,
        pattern_id: &str,
        solution: &str,
        command: Option<&str>,
    ) -> Result<String> {
        let solution_id = Uuid::new_v4().to_string();

        self.conn.execute(
            "INSERT INTO solutions (id, pattern_id, solution, command)
             VALUES (?, ?, ?, ?)",
            params![solution_id, pattern_id, solution, command],
        )?;

        Ok(solution_id)
    }

    /// Get all patterns
    pub fn get_all_patterns(&self) -> Result<Vec<Pattern>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, pattern, scope, language, framework, tags, created_at, updated_at
             FROM patterns ORDER BY created_at DESC",
        )?;

        let patterns = stmt
            .query_map([], |row| {
                let tags_str: String = row.get::<_, Option<String>>(5)?.unwrap_or_default();
                Ok(Pattern {
                    id: row.get(0)?,
                    pattern: row.get(1)?,
                    scope: row.get(2)?,
                    language: row.get(3)?,
                    framework: row.get(4)?,
                    tags: if tags_str.is_empty() {
                        vec![]
                    } else {
                        tags_str.split(',').map(|s| s.to_string()).collect()
                    },
                    created_at: row.get(6)?,
                    updated_at: row.get(7)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(patterns)
    }

    /// Get a pattern by ID
    pub fn get_pattern(&self, pattern_id: &str) -> Result<Option<Pattern>> {
        self.conn
            .query_row(
                "SELECT id, pattern, scope, language, framework, tags, created_at, updated_at
                 FROM patterns WHERE id = ?",
                params![pattern_id],
                |row| {
                    let tags_str: String = row.get::<_, Option<String>>(5)?.unwrap_or_default();
                    Ok(Pattern {
                        id: row.get(0)?,
                        pattern: row.get(1)?,
                        scope: row.get(2)?,
                        language: row.get(3)?,
                        framework: row.get(4)?,
                        tags: if tags_str.is_empty() {
                            vec![]
                        } else {
                            tags_str.split(',').map(|s| s.to_string()).collect()
                        },
                        created_at: row.get(6)?,
                        updated_at: row.get(7)?,
                    })
                },
            )
            .optional()
            .map_err(Into::into)
    }

    /// Get solutions for a pattern, ordered by confidence
    pub fn get_solutions(&self, pattern_id: &str) -> Result<Vec<Solution>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, pattern_id, solution, command, confidence,
                    success_count, failure_count, created_at, last_confirmed
             FROM solutions WHERE pattern_id = ? ORDER BY confidence DESC",
        )?;

        let solutions = stmt
            .query_map(params![pattern_id], |row| {
                Ok(Solution {
                    id: row.get(0)?,
                    pattern_id: row.get(1)?,
                    solution: row.get(2)?,
                    command: row.get(3)?,
                    confidence: row.get(4)?,
                    success_count: row.get(5)?,
                    failure_count: row.get(6)?,
                    created_at: row.get(7)?,
                    last_confirmed: row.get(8)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(solutions)
    }

    /// Get a solution by ID
    pub fn get_solution(&self, solution_id: &str) -> Result<Option<Solution>> {
        self.conn
            .query_row(
                "SELECT id, pattern_id, solution, command, confidence,
                        success_count, failure_count, created_at, last_confirmed
                 FROM solutions WHERE id = ?",
                params![solution_id],
                |row| {
                    Ok(Solution {
                        id: row.get(0)?,
                        pattern_id: row.get(1)?,
                        solution: row.get(2)?,
                        command: row.get(3)?,
                        confidence: row.get(4)?,
                        success_count: row.get(5)?,
                        failure_count: row.get(6)?,
                        created_at: row.get(7)?,
                        last_confirmed: row.get(8)?,
                    })
                },
            )
            .optional()
            .map_err(Into::into)
    }

    /// Confirm a solution worked (increase confidence)
    pub fn confirm_solution(&mut self, solution_id: &str) -> Result<()> {
        // Bayesian confidence: (success + 1) / (success + failure + 2)
        self.conn.execute(
            "UPDATE solutions
             SET success_count = success_count + 1,
                 confidence = CAST(success_count + 2 AS REAL) / (success_count + failure_count + 3),
                 last_confirmed = CURRENT_TIMESTAMP
             WHERE id = ?",
            params![solution_id],
        )?;

        // Log the confirmation
        self.conn.execute(
            "INSERT INTO usage_log (solution_id, outcome) VALUES (?, 'confirmed')",
            params![solution_id],
        )?;

        Ok(())
    }

    /// Report that a solution didn't work (decrease confidence)
    pub fn report_failure(&mut self, solution_id: &str) -> Result<()> {
        // Bayesian confidence: (success + 1) / (success + failure + 2)
        self.conn.execute(
            "UPDATE solutions
             SET failure_count = failure_count + 1,
                 confidence = CAST(success_count + 1 AS REAL) / (success_count + failure_count + 3)
             WHERE id = ?",
            params![solution_id],
        )?;

        // Log the failure
        self.conn.execute(
            "INSERT INTO usage_log (solution_id, outcome) VALUES (?, 'reported')",
            params![solution_id],
        )?;

        Ok(())
    }

    /// Get database statistics
    pub fn stats(&self) -> Result<Stats> {
        let total_patterns: i32 = self.conn.query_row(
            "SELECT COUNT(*) FROM patterns",
            [],
            |row| row.get(0),
        )?;

        let total_solutions: i32 = self.conn.query_row(
            "SELECT COUNT(*) FROM solutions",
            [],
            |row| row.get(0),
        )?;

        let mut by_scope = std::collections::HashMap::new();
        let mut stmt = self.conn.prepare(
            "SELECT scope, COUNT(*) FROM patterns GROUP BY scope",
        )?;
        let scope_rows = stmt.query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, i32>(1)?))
        })?;
        for row in scope_rows {
            let (scope, count) = row?;
            by_scope.insert(scope, count);
        }

        let mut by_language = std::collections::HashMap::new();
        let mut stmt = self.conn.prepare(
            "SELECT language, COUNT(*) FROM patterns WHERE language IS NOT NULL GROUP BY language",
        )?;
        let lang_rows = stmt.query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, i32>(1)?))
        })?;
        for row in lang_rows {
            let (lang, count) = row?;
            by_language.insert(lang, count);
        }

        Ok(Stats {
            total_patterns,
            total_solutions,
            by_scope,
            by_language,
        })
    }
}

/// Helper struct for seeding patterns
struct SeedPattern {
    pattern: &'static str,
    scope: &'static str,
    language: Option<&'static str>,
    solution: Option<&'static str>,
    command: Option<&'static str>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_database_creation() -> Result<()> {
        let tmp = TempDir::new()?;
        let db_path = tmp.path().join("test.db");
        let _db = ErrorDatabase::open(Some(&db_path))?;
        assert!(db_path.exists());
        Ok(())
    }

    #[test]
    fn test_add_pattern() -> Result<()> {
        let tmp = TempDir::new()?;
        let db_path = tmp.path().join("test.db");
        let mut db = ErrorDatabase::open(Some(&db_path))?;

        let pattern_id = db.add_pattern(
            "Test error pattern",
            "global",
            None,
            None,
            None,
            Some(("Test solution".to_string(), None)),
        )?;

        assert!(!pattern_id.is_empty());

        let pattern = db.get_pattern(&pattern_id)?;
        assert!(pattern.is_some());
        assert_eq!(pattern.unwrap().pattern, "Test error pattern");

        let solutions = db.get_solutions(&pattern_id)?;
        assert_eq!(solutions.len(), 1);
        assert_eq!(solutions[0].solution, "Test solution");

        Ok(())
    }

    #[test]
    fn test_confidence_scoring() -> Result<()> {
        let tmp = TempDir::new()?;
        let db_path = tmp.path().join("test.db");
        let mut db = ErrorDatabase::open(Some(&db_path))?;

        let pattern_id = db.add_pattern(
            "Test error",
            "global",
            None,
            None,
            None,
            Some(("Test solution".to_string(), None)),
        )?;

        let solutions = db.get_solutions(&pattern_id)?;
        let solution_id = &solutions[0].id;

        // Initial confidence should be 0.5
        assert!((solutions[0].confidence - 0.5).abs() < 0.01);

        // Confirm should increase confidence
        db.confirm_solution(solution_id)?;
        let solutions = db.get_solutions(&pattern_id)?;
        assert!(solutions[0].confidence > 0.5);

        // Report failure should decrease confidence
        db.report_failure(solution_id)?;
        let solutions_after = db.get_solutions(&pattern_id)?;
        assert!(solutions_after[0].confidence < solutions[0].confidence);

        Ok(())
    }
}
