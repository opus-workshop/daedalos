//! Database module for journal event storage
//!
//! Uses SQLite to store events from all Daedalos tools.
//! Schema: timestamp, source, event_type, summary, details_json
//!
//! Note: JournalDatabase is available for future use by other tools
//! that may want to write events directly to the SQLite database.

#![allow(dead_code)]

use anyhow::{Context, Result};
use chrono::{DateTime, TimeZone, Utc};
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;

/// A single event in the journal
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Event {
    /// Unix timestamp (seconds since epoch)
    pub timestamp: f64,
    /// Which tool/component generated the event (gates, loop, agent, undo, mcp-hub, user)
    pub source: String,
    /// Type of event (e.g., loop_started, file_changed, gate_check)
    pub event_type: String,
    /// Human-readable summary
    pub summary: String,
    /// Additional details as key-value pairs
    #[serde(default)]
    pub details: HashMap<String, serde_json::Value>,
}

impl Event {
    /// Create a new event with the current timestamp
    pub fn new(source: &str, event_type: &str, summary: &str) -> Self {
        Self {
            timestamp: Utc::now().timestamp() as f64,
            source: source.to_string(),
            event_type: event_type.to_string(),
            summary: summary.to_string(),
            details: HashMap::new(),
        }
    }

    /// Get the timestamp as a DateTime
    pub fn datetime(&self) -> DateTime<Utc> {
        Utc.timestamp_opt(self.timestamp as i64, 0)
            .single()
            .unwrap_or_else(Utc::now)
    }

    /// Add a detail to the event
    pub fn with_detail(mut self, key: &str, value: impl Into<serde_json::Value>) -> Self {
        self.details.insert(key.to_string(), value.into());
        self
    }
}

/// Database for storing journal events
pub struct JournalDatabase {
    conn: Connection,
}

impl JournalDatabase {
    /// Open or create the journal database
    pub fn open(data_dir: &Path) -> Result<Self> {
        std::fs::create_dir_all(data_dir)
            .context("Failed to create journal data directory")?;

        let db_path = data_dir.join("events.db");
        let conn = Connection::open(&db_path)
            .context("Failed to open journal database")?;

        // Enable WAL mode for better concurrent access
        conn.execute_batch("PRAGMA journal_mode=WAL;")?;

        // Create tables
        conn.execute_batch(
            r#"
            CREATE TABLE IF NOT EXISTS events (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                timestamp REAL NOT NULL,
                source TEXT NOT NULL,
                event_type TEXT NOT NULL,
                summary TEXT NOT NULL,
                details_json TEXT DEFAULT '{}'
            );

            CREATE INDEX IF NOT EXISTS idx_events_timestamp ON events(timestamp DESC);
            CREATE INDEX IF NOT EXISTS idx_events_source ON events(source);
            CREATE INDEX IF NOT EXISTS idx_events_type ON events(event_type);
            "#,
        )?;

        Ok(Self { conn })
    }

    /// Log an event to the database
    pub fn log_event(&self, event: &Event) -> Result<i64> {
        let details_json = serde_json::to_string(&event.details)?;

        self.conn.execute(
            "INSERT INTO events (timestamp, source, event_type, summary, details_json)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![
                event.timestamp,
                event.source,
                event.event_type,
                event.summary,
                details_json,
            ],
        )?;

        Ok(self.conn.last_insert_rowid())
    }

    /// Query events with filters
    pub fn query_events(
        &self,
        since: f64,
        source: Option<&str>,
        event_type: Option<&str>,
        limit: u32,
    ) -> Result<Vec<Event>> {
        let mut sql = String::from(
            "SELECT timestamp, source, event_type, summary, details_json
             FROM events WHERE timestamp >= ?1",
        );

        if source.is_some() {
            sql.push_str(" AND source = ?2");
        }
        if event_type.is_some() {
            sql.push_str(if source.is_some() {
                " AND event_type = ?3"
            } else {
                " AND event_type = ?2"
            });
        }

        sql.push_str(" ORDER BY timestamp DESC LIMIT ?");

        let mut stmt = self.conn.prepare(&sql)?;

        let events = match (source, event_type) {
            (Some(s), Some(t)) => {
                let rows = stmt.query_map(params![since, s, t, limit], |row| {
                    Ok(Self::row_to_event(row))
                })?;
                rows.collect::<Result<Vec<_>, _>>()?
            }
            (Some(s), None) => {
                let rows = stmt.query_map(params![since, s, limit], |row| {
                    Ok(Self::row_to_event(row))
                })?;
                rows.collect::<Result<Vec<_>, _>>()?
            }
            (None, Some(t)) => {
                let rows = stmt.query_map(params![since, t, limit], |row| {
                    Ok(Self::row_to_event(row))
                })?;
                rows.collect::<Result<Vec<_>, _>>()?
            }
            (None, None) => {
                let rows = stmt.query_map(params![since, limit], |row| {
                    Ok(Self::row_to_event(row))
                })?;
                rows.collect::<Result<Vec<_>, _>>()?
            }
        };

        Ok(events)
    }

    /// Get the count of events by source
    pub fn count_by_source(&self, since: f64) -> Result<HashMap<String, u64>> {
        let mut stmt = self.conn.prepare(
            "SELECT source, COUNT(*) FROM events WHERE timestamp >= ?1 GROUP BY source",
        )?;

        let rows = stmt.query_map([since], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, u64>(1)?))
        })?;

        let mut counts = HashMap::new();
        for row in rows {
            let (source, count) = row?;
            counts.insert(source, count);
        }

        Ok(counts)
    }

    /// Get the count of events by type
    pub fn count_by_type(&self, since: f64) -> Result<HashMap<String, u64>> {
        let mut stmt = self.conn.prepare(
            "SELECT event_type, COUNT(*) FROM events WHERE timestamp >= ?1 GROUP BY event_type",
        )?;

        let rows = stmt.query_map([since], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, u64>(1)?))
        })?;

        let mut counts = HashMap::new();
        for row in rows {
            let (event_type, count) = row?;
            counts.insert(event_type, count);
        }

        Ok(counts)
    }

    /// Get the total event count and time range
    pub fn get_stats(&self, since: f64) -> Result<(u64, Option<f64>, Option<f64>)> {
        let count: u64 = self.conn.query_row(
            "SELECT COUNT(*) FROM events WHERE timestamp >= ?1",
            [since],
            |row| row.get(0),
        )?;

        let min_ts: Option<f64> = self.conn.query_row(
            "SELECT MIN(timestamp) FROM events WHERE timestamp >= ?1",
            [since],
            |row| row.get(0),
        ).optional()?.flatten();

        let max_ts: Option<f64> = self.conn.query_row(
            "SELECT MAX(timestamp) FROM events WHERE timestamp >= ?1",
            [since],
            |row| row.get(0),
        ).optional()?.flatten();

        Ok((count, min_ts, max_ts))
    }

    /// Convert a database row to an Event
    fn row_to_event(row: &rusqlite::Row) -> Event {
        let timestamp: f64 = row.get(0).unwrap_or(0.0);
        let source: String = row.get(1).unwrap_or_default();
        let event_type: String = row.get(2).unwrap_or_default();
        let summary: String = row.get(3).unwrap_or_default();
        let details_json: String = row.get(4).unwrap_or_else(|_| "{}".to_string());

        let details: HashMap<String, serde_json::Value> =
            serde_json::from_str(&details_json).unwrap_or_default();

        Event {
            timestamp,
            source,
            event_type,
            summary,
            details,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_database_creation() {
        let dir = TempDir::new().unwrap();
        let db = JournalDatabase::open(dir.path()).unwrap();

        let event = Event::new("test", "test_event", "Test event");
        let id = db.log_event(&event).unwrap();
        assert!(id > 0);
    }

    #[test]
    fn test_query_events() {
        let dir = TempDir::new().unwrap();
        let db = JournalDatabase::open(dir.path()).unwrap();

        // Log some events
        db.log_event(&Event::new("gates", "gate_check", "Gate check")).unwrap();
        db.log_event(&Event::new("loop", "loop_started", "Loop started")).unwrap();
        db.log_event(&Event::new("gates", "gate_check", "Another gate check")).unwrap();

        // Query all
        let events = db.query_events(0.0, None, None, 100).unwrap();
        assert_eq!(events.len(), 3);

        // Query by source
        let events = db.query_events(0.0, Some("gates"), None, 100).unwrap();
        assert_eq!(events.len(), 2);

        // Query by type
        let events = db.query_events(0.0, None, Some("loop_started"), 100).unwrap();
        assert_eq!(events.len(), 1);
    }
}
