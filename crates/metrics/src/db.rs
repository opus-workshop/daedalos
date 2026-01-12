//! Database module for metrics storage
//!
//! Uses SQLite to store aggregated daily metrics for fast queries.
//! Can also read from other Daedalos data sources (journal, focus).

use anyhow::{Context, Result};
use chrono::{NaiveDate, Utc};
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use std::path::Path;

/// Daily aggregated metrics
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DailyMetrics {
    /// Date in YYYY-MM-DD format
    pub date: String,
    /// Number of git commits
    pub commits: u32,
    /// Lines added
    pub insertions: u32,
    /// Lines removed
    pub deletions: u32,
    /// Number of files touched
    pub files_touched: u32,
    /// Number of focus sessions
    pub focus_sessions: u32,
    /// Total focus minutes
    pub focus_minutes: u32,
    /// Completed focus sessions
    pub focus_completed: u32,
    /// Number of journal events
    pub journal_events: u32,
    /// Number of errors logged
    pub errors: u32,
}

impl DailyMetrics {
    /// Create new metrics for a date
    pub fn new(date: &str) -> Self {
        Self {
            date: date.to_string(),
            ..Default::default()
        }
    }

    /// Calculate net lines changed (insertions - deletions)
    pub fn net_lines(&self) -> i32 {
        self.insertions as i32 - self.deletions as i32
    }

    /// Calculate total lines changed
    pub fn total_lines_changed(&self) -> u32 {
        self.insertions + self.deletions
    }

    /// Calculate focus completion rate (0-100)
    pub fn focus_completion_rate(&self) -> u32 {
        if self.focus_sessions == 0 {
            0
        } else {
            (self.focus_completed * 100) / self.focus_sessions
        }
    }
}

/// Database for storing metrics
pub struct MetricsDatabase {
    conn: Connection,
}

impl MetricsDatabase {
    /// Open or create the metrics database
    pub fn open(data_dir: &Path) -> Result<Self> {
        std::fs::create_dir_all(data_dir)
            .context("Failed to create metrics data directory")?;

        let db_path = data_dir.join("metrics.db");
        let conn = Connection::open(&db_path)
            .context("Failed to open metrics database")?;

        // Enable WAL mode for better concurrent access
        conn.execute_batch("PRAGMA journal_mode=WAL;")?;

        // Create tables
        conn.execute_batch(
            r#"
            CREATE TABLE IF NOT EXISTS daily_metrics (
                date TEXT PRIMARY KEY,
                commits INTEGER DEFAULT 0,
                insertions INTEGER DEFAULT 0,
                deletions INTEGER DEFAULT 0,
                files_touched INTEGER DEFAULT 0,
                focus_sessions INTEGER DEFAULT 0,
                focus_minutes INTEGER DEFAULT 0,
                focus_completed INTEGER DEFAULT 0,
                journal_events INTEGER DEFAULT 0,
                errors INTEGER DEFAULT 0,
                updated_at REAL DEFAULT (strftime('%s', 'now'))
            );

            CREATE INDEX IF NOT EXISTS idx_daily_date ON daily_metrics(date DESC);
            "#,
        )?;

        Ok(Self { conn })
    }

    /// Open database from default path
    pub fn open_default() -> Result<Self> {
        let paths = daedalos_core::Paths::new();
        let metrics_dir = paths.data.join("metrics");
        Self::open(&metrics_dir)
    }

    /// Store or update daily metrics
    pub fn upsert_daily(&self, metrics: &DailyMetrics) -> Result<()> {
        self.conn.execute(
            r#"
            INSERT INTO daily_metrics (
                date, commits, insertions, deletions, files_touched,
                focus_sessions, focus_minutes, focus_completed,
                journal_events, errors, updated_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, strftime('%s', 'now'))
            ON CONFLICT(date) DO UPDATE SET
                commits = ?2,
                insertions = ?3,
                deletions = ?4,
                files_touched = ?5,
                focus_sessions = ?6,
                focus_minutes = ?7,
                focus_completed = ?8,
                journal_events = ?9,
                errors = ?10,
                updated_at = strftime('%s', 'now')
            "#,
            params![
                metrics.date,
                metrics.commits,
                metrics.insertions,
                metrics.deletions,
                metrics.files_touched,
                metrics.focus_sessions,
                metrics.focus_minutes,
                metrics.focus_completed,
                metrics.journal_events,
                metrics.errors,
            ],
        )?;
        Ok(())
    }

    /// Get metrics for a specific date
    pub fn get_daily(&self, date: &str) -> Result<Option<DailyMetrics>> {
        let mut stmt = self.conn.prepare(
            r#"
            SELECT date, commits, insertions, deletions, files_touched,
                   focus_sessions, focus_minutes, focus_completed,
                   journal_events, errors
            FROM daily_metrics WHERE date = ?1
            "#,
        )?;

        let result = stmt.query_row([date], |row| {
            Ok(DailyMetrics {
                date: row.get(0)?,
                commits: row.get(1)?,
                insertions: row.get(2)?,
                deletions: row.get(3)?,
                files_touched: row.get(4)?,
                focus_sessions: row.get(5)?,
                focus_minutes: row.get(6)?,
                focus_completed: row.get(7)?,
                journal_events: row.get(8)?,
                errors: row.get(9)?,
            })
        });

        match result {
            Ok(metrics) => Ok(Some(metrics)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    /// Get metrics for a date range (inclusive)
    pub fn get_range(&self, start: &str, end: &str) -> Result<Vec<DailyMetrics>> {
        let mut stmt = self.conn.prepare(
            r#"
            SELECT date, commits, insertions, deletions, files_touched,
                   focus_sessions, focus_minutes, focus_completed,
                   journal_events, errors
            FROM daily_metrics
            WHERE date >= ?1 AND date <= ?2
            ORDER BY date ASC
            "#,
        )?;

        let rows = stmt.query_map([start, end], |row| {
            Ok(DailyMetrics {
                date: row.get(0)?,
                commits: row.get(1)?,
                insertions: row.get(2)?,
                deletions: row.get(3)?,
                files_touched: row.get(4)?,
                focus_sessions: row.get(5)?,
                focus_minutes: row.get(6)?,
                focus_completed: row.get(7)?,
                journal_events: row.get(8)?,
                errors: row.get(9)?,
            })
        })?;

        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }

    /// Get aggregated metrics for a date range
    pub fn get_aggregated(&self, start: &str, end: &str) -> Result<DailyMetrics> {
        let mut stmt = self.conn.prepare(
            r#"
            SELECT
                COALESCE(SUM(commits), 0),
                COALESCE(SUM(insertions), 0),
                COALESCE(SUM(deletions), 0),
                COALESCE(SUM(files_touched), 0),
                COALESCE(SUM(focus_sessions), 0),
                COALESCE(SUM(focus_minutes), 0),
                COALESCE(SUM(focus_completed), 0),
                COALESCE(SUM(journal_events), 0),
                COALESCE(SUM(errors), 0)
            FROM daily_metrics
            WHERE date >= ?1 AND date <= ?2
            "#,
        )?;

        stmt.query_row([start, end], |row| {
            Ok(DailyMetrics {
                date: format!("{} to {}", start, end),
                commits: row.get(0)?,
                insertions: row.get(1)?,
                deletions: row.get(2)?,
                files_touched: row.get(3)?,
                focus_sessions: row.get(4)?,
                focus_minutes: row.get(5)?,
                focus_completed: row.get(6)?,
                journal_events: row.get(7)?,
                errors: row.get(8)?,
            })
        }).map_err(Into::into)
    }

    /// Get metrics grouped by week
    pub fn get_weekly(&self, weeks: u32) -> Result<Vec<(String, DailyMetrics)>> {
        let mut stmt = self.conn.prepare(
            r#"
            SELECT
                strftime('%Y-W%W', date) as week,
                COALESCE(SUM(commits), 0),
                COALESCE(SUM(insertions), 0),
                COALESCE(SUM(deletions), 0),
                COALESCE(SUM(files_touched), 0),
                COALESCE(SUM(focus_sessions), 0),
                COALESCE(SUM(focus_minutes), 0),
                COALESCE(SUM(focus_completed), 0),
                COALESCE(SUM(journal_events), 0),
                COALESCE(SUM(errors), 0)
            FROM daily_metrics
            WHERE date >= date('now', '-' || ?1 || ' days')
            GROUP BY week
            ORDER BY week DESC
            LIMIT ?1
            "#,
        )?;

        let rows = stmt.query_map([weeks * 7], |row| {
            Ok((
                row.get::<_, String>(0)?,
                DailyMetrics {
                    date: row.get(0)?,
                    commits: row.get(1)?,
                    insertions: row.get(2)?,
                    deletions: row.get(3)?,
                    files_touched: row.get(4)?,
                    focus_sessions: row.get(5)?,
                    focus_minutes: row.get(6)?,
                    focus_completed: row.get(7)?,
                    journal_events: row.get(8)?,
                    errors: row.get(9)?,
                },
            ))
        })?;

        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }
}

/// Get date string for N days ago
pub fn date_ago(days: i64) -> String {
    let date = Utc::now() - chrono::Duration::days(days);
    date.format("%Y-%m-%d").to_string()
}

/// Get today's date string
pub fn today() -> String {
    Utc::now().format("%Y-%m-%d").to_string()
}

/// Get day of week name from date string
pub fn day_name(date: &str) -> String {
    if let Ok(parsed) = NaiveDate::parse_from_str(date, "%Y-%m-%d") {
        parsed.format("%a").to_string()
    } else {
        "???".to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_database_creation() {
        let dir = TempDir::new().unwrap();
        let db = MetricsDatabase::open(dir.path()).unwrap();

        let metrics = DailyMetrics {
            date: "2025-01-11".to_string(),
            commits: 5,
            insertions: 100,
            deletions: 50,
            ..Default::default()
        };

        db.upsert_daily(&metrics).unwrap();
        let retrieved = db.get_daily("2025-01-11").unwrap().unwrap();
        assert_eq!(retrieved.commits, 5);
        assert_eq!(retrieved.insertions, 100);
    }

    #[test]
    fn test_date_range() {
        let dir = TempDir::new().unwrap();
        let db = MetricsDatabase::open(dir.path()).unwrap();

        for i in 1..=7 {
            let metrics = DailyMetrics {
                date: format!("2025-01-{:02}", i),
                commits: i,
                ..Default::default()
            };
            db.upsert_daily(&metrics).unwrap();
        }

        let range = db.get_range("2025-01-01", "2025-01-07").unwrap();
        assert_eq!(range.len(), 7);

        let agg = db.get_aggregated("2025-01-01", "2025-01-07").unwrap();
        assert_eq!(agg.commits, 28); // 1+2+3+4+5+6+7
    }

    #[test]
    fn test_daily_metrics_calculations() {
        let metrics = DailyMetrics {
            insertions: 100,
            deletions: 30,
            focus_sessions: 5,
            focus_completed: 4,
            ..Default::default()
        };

        assert_eq!(metrics.net_lines(), 70);
        assert_eq!(metrics.total_lines_changed(), 130);
        assert_eq!(metrics.focus_completion_rate(), 80);
    }
}
