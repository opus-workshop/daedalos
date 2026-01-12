//! Event collector - aggregates events from all Daedalos tools
//!
//! Pulls events from:
//! - gates: Gate checks and approvals (JSONL files)
//! - loop: Iteration loop events (JSON state files)
//! - agent: Agent lifecycle events (JSONL logs)
//! - undo: File changes and checkpoints (JSONL timeline)
//! - mcp-hub: MCP server events (log files)
//! - journal: Custom user-logged events (JSONL files)

use crate::db::{Event, JournalDatabase};
use anyhow::Result;
use daedalos_core::Paths;
use serde_json::Value;
use std::collections::HashMap;
use std::fs::{self, File};
use std::io::{BufRead, BufReader};
use std::path::PathBuf;

/// Collector for aggregating events from all sources
pub struct EventCollector {
    data_dir: PathBuf,
}

impl EventCollector {
    /// Create a new event collector
    pub fn new() -> Self {
        let paths = Paths::new();
        Self {
            data_dir: paths.data,
        }
    }

    /// Collect all events from all sources since a given timestamp
    pub fn collect_all(
        &self,
        since: f64,
        sources: Option<&[&str]>,
        event_types: Option<&[&str]>,
        limit: usize,
    ) -> Vec<Event> {
        let mut events = Vec::new();

        // Collect from each source
        let all_sources = ["gates", "loop", "agent", "undo", "mcp-hub", "journal"];

        for source in &all_sources {
            if let Some(filter) = sources {
                if !filter.contains(source) {
                    continue;
                }
            }

            let source_events = match *source {
                "gates" => self.collect_gate_events(since),
                "loop" => self.collect_loop_events(since),
                "agent" => self.collect_agent_events(since),
                "undo" => self.collect_undo_events(since),
                "mcp-hub" => self.collect_mcp_events(since),
                "journal" => self.collect_journal_events(since),
                _ => Vec::new(),
            };

            for event in source_events {
                if let Some(types) = event_types {
                    if !types.contains(&event.event_type.as_str()) {
                        continue;
                    }
                }
                events.push(event);
            }
        }

        // Sort by timestamp descending (newest first)
        events.sort_by(|a, b| b.timestamp.partial_cmp(&a.timestamp).unwrap());

        // Apply limit
        events.truncate(limit);

        events
    }

    /// Collect gate check events from JSONL files
    fn collect_gate_events(&self, since: f64) -> Vec<Event> {
        let mut events = Vec::new();
        let gates_dir = self.data_dir.join("gates");

        if !gates_dir.exists() {
            return events;
        }

        // Read gates-*.jsonl files
        if let Ok(entries) = fs::read_dir(&gates_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().map_or(false, |e| e == "jsonl") {
                    if let Ok(file) = File::open(&path) {
                        let reader = BufReader::new(file);
                        for line in reader.lines().flatten() {
                            if let Ok(data) = serde_json::from_str::<Value>(&line) {
                                let ts = data.get("timestamp")
                                    .and_then(|v| v.as_f64())
                                    .unwrap_or(0.0);

                                if ts < since {
                                    continue;
                                }

                                let allowed = data.get("result")
                                    .and_then(|r| r.get("allowed"))
                                    .and_then(|a| a.as_bool())
                                    .unwrap_or(false);

                                let gate = data.get("gate")
                                    .and_then(|g| g.as_str())
                                    .unwrap_or("unknown");

                                let source = data.get("source")
                                    .and_then(|s| s.as_str())
                                    .unwrap_or("unknown");

                                let status = if allowed { "allowed" } else { "denied" };
                                let summary = format!("Gate '{}' {} (from {})", gate, status, source);

                                let mut event = Event {
                                    timestamp: ts,
                                    source: "gates".to_string(),
                                    event_type: "gate_check".to_string(),
                                    summary,
                                    details: HashMap::new(),
                                };

                                // Add relevant details
                                if let Some(obj) = data.as_object() {
                                    for (k, v) in obj {
                                        event.details.insert(k.clone(), v.clone());
                                    }
                                }

                                events.push(event);
                            }
                        }
                    }
                }
            }
        }

        events
    }

    /// Collect loop iteration events from JSON state files
    fn collect_loop_events(&self, since: f64) -> Vec<Event> {
        let mut events = Vec::new();
        let loop_dir = self.data_dir.join("loop");

        if !loop_dir.exists() {
            return events;
        }

        // Read *.json state files
        if let Ok(entries) = fs::read_dir(&loop_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().map_or(false, |e| e == "json") {
                    if let Ok(contents) = fs::read_to_string(&path) {
                        if let Ok(data) = serde_json::from_str::<Value>(&contents) {
                            let started = data.get("started")
                                .and_then(|v| v.as_f64())
                                .unwrap_or(0.0);

                            if started < since {
                                continue;
                            }

                            let task = data.get("task")
                                .and_then(|t| t.as_str())
                                .unwrap_or("unknown task");

                            let status = data.get("status")
                                .and_then(|s| s.as_str())
                                .unwrap_or("unknown");

                            let iterations = data.get("iteration")
                                .and_then(|i| i.as_u64())
                                .unwrap_or(0);

                            // Start event
                            events.push(Event {
                                timestamp: started,
                                source: "loop".to_string(),
                                event_type: "loop_started".to_string(),
                                summary: format!("Loop started: {}", task),
                                details: json_to_details(&data),
                            });

                            // End event if completed/failed/stopped
                            if matches!(status, "completed" | "failed" | "stopped") {
                                let ended = data.get("ended")
                                    .and_then(|v| v.as_f64())
                                    .unwrap_or(started);

                                events.push(Event {
                                    timestamp: ended,
                                    source: "loop".to_string(),
                                    event_type: format!("loop_{}", status),
                                    summary: format!("Loop {} after {} iterations: {}", status, iterations, task),
                                    details: json_to_details(&data),
                                });
                            }
                        }
                    }
                }
            }
        }

        events
    }

    /// Collect agent lifecycle events from JSONL logs
    fn collect_agent_events(&self, since: f64) -> Vec<Event> {
        let mut events = Vec::new();
        let agent_dir = self.data_dir.join("agent");

        if !agent_dir.exists() {
            return events;
        }

        // Read *.jsonl log files
        if let Ok(entries) = fs::read_dir(&agent_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().map_or(false, |e| e == "jsonl") {
                    if let Ok(file) = File::open(&path) {
                        let reader = BufReader::new(file);
                        for line in reader.lines().flatten() {
                            if let Ok(data) = serde_json::from_str::<Value>(&line) {
                                let ts = data.get("timestamp")
                                    .and_then(|v| v.as_f64())
                                    .unwrap_or(0.0);

                                if ts < since {
                                    continue;
                                }

                                let event_type = data.get("event")
                                    .and_then(|e| e.as_str())
                                    .unwrap_or("agent_event")
                                    .to_string();

                                let agent_name = data.get("name")
                                    .and_then(|n| n.as_str())
                                    .unwrap_or("unknown");

                                let summary = data.get("summary")
                                    .and_then(|s| s.as_str())
                                    .map(|s| s.to_string())
                                    .unwrap_or_else(|| format!("Agent event: {}", agent_name));

                                events.push(Event {
                                    timestamp: ts,
                                    source: "agent".to_string(),
                                    event_type,
                                    summary,
                                    details: json_to_details(&data),
                                });
                            }
                        }
                    }
                }
            }
        }

        events
    }

    /// Collect file change events from undo timeline
    fn collect_undo_events(&self, since: f64) -> Vec<Event> {
        let mut events = Vec::new();
        let undo_dir = self.data_dir.join("undo");
        let timeline_file = undo_dir.join("timeline.jsonl");

        if !timeline_file.exists() {
            return events;
        }

        if let Ok(file) = File::open(&timeline_file) {
            let reader = BufReader::new(file);
            for line in reader.lines().flatten() {
                if let Ok(data) = serde_json::from_str::<Value>(&line) {
                    let ts = data.get("timestamp")
                        .and_then(|v| v.as_f64())
                        .unwrap_or(0.0);

                    if ts < since {
                        continue;
                    }

                    let file_path = data.get("path")
                        .and_then(|p| p.as_str())
                        .unwrap_or("unknown");

                    let action = data.get("action")
                        .and_then(|a| a.as_str())
                        .unwrap_or("modified");

                    let checkpoint = data.get("checkpoint")
                        .and_then(|c| c.as_str());

                    if let Some(cp) = checkpoint {
                        events.push(Event {
                            timestamp: ts,
                            source: "undo".to_string(),
                            event_type: "checkpoint".to_string(),
                            summary: format!("Checkpoint created: {}", cp),
                            details: json_to_details(&data),
                        });
                    } else {
                        events.push(Event {
                            timestamp: ts,
                            source: "undo".to_string(),
                            event_type: format!("file_{}", action),
                            summary: format!("File {}: {}", action, file_path),
                            details: json_to_details(&data),
                        });
                    }
                }
            }
        }

        events
    }

    /// Collect MCP hub events from log files
    fn collect_mcp_events(&self, since: f64) -> Vec<Event> {
        let mut events = Vec::new();
        let mcp_dir = self.data_dir.join("mcp-hub");

        if !mcp_dir.exists() {
            return events;
        }

        // Read *.log files
        if let Ok(entries) = fs::read_dir(&mcp_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().map_or(false, |e| e == "log") {
                    if let Ok(metadata) = fs::metadata(&path) {
                        if let Ok(modified) = metadata.modified() {
                            let mtime = modified
                                .duration_since(std::time::UNIX_EPOCH)
                                .map(|d| d.as_secs_f64())
                                .unwrap_or(0.0);

                            if mtime < since {
                                continue;
                            }

                            // Basic parsing of log files for start/stop events
                            if let Ok(contents) = fs::read_to_string(&path) {
                                for line in contents.lines() {
                                    let line_lower = line.to_lowercase();
                                    if line_lower.contains("started") || line_lower.contains("stopped") {
                                        let summary = line.chars().take(100).collect::<String>();
                                        events.push(Event {
                                            timestamp: mtime,
                                            source: "mcp-hub".to_string(),
                                            event_type: "mcp_event".to_string(),
                                            summary,
                                            details: HashMap::new(),
                                        });
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        events
    }

    /// Collect events logged directly to the journal
    fn collect_journal_events(&self, since: f64) -> Vec<Event> {
        let mut events = Vec::new();
        let journal_dir = self.data_dir.join("journal");

        if !journal_dir.exists() {
            return events;
        }

        // Read journal-*.jsonl files
        if let Ok(entries) = fs::read_dir(&journal_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                let filename = path.file_name()
                    .and_then(|f| f.to_str())
                    .unwrap_or("");

                if filename.starts_with("journal-") && filename.ends_with(".jsonl") {
                    if let Ok(file) = File::open(&path) {
                        let reader = BufReader::new(file);
                        for line in reader.lines().flatten() {
                            if let Ok(data) = serde_json::from_str::<Value>(&line) {
                                let ts = data.get("timestamp")
                                    .and_then(|v| v.as_f64())
                                    .unwrap_or(0.0);

                                if ts < since {
                                    continue;
                                }

                                let source = data.get("source")
                                    .and_then(|s| s.as_str())
                                    .unwrap_or("user")
                                    .to_string();

                                let event_type = data.get("event_type")
                                    .and_then(|t| t.as_str())
                                    .unwrap_or("custom")
                                    .to_string();

                                let summary = data.get("summary")
                                    .and_then(|s| s.as_str())
                                    .unwrap_or("")
                                    .to_string();

                                events.push(Event {
                                    timestamp: ts,
                                    source,
                                    event_type,
                                    summary,
                                    details: json_to_details(&data),
                                });
                            }
                        }
                    }
                }
            }
        }

        events
    }

    /// Sync collected events to the database
    #[allow(dead_code)]
    pub fn sync_to_db(&self, db: &JournalDatabase, since: f64) -> Result<usize> {
        let events = self.collect_all(since, None, None, 10000);
        let mut count = 0;

        for event in events {
            // Note: This is a simple approach - in production, we'd want to
            // deduplicate based on (timestamp, source, event_type, summary)
            db.log_event(&event)?;
            count += 1;
        }

        Ok(count)
    }
}

impl Default for EventCollector {
    fn default() -> Self {
        Self::new()
    }
}

/// Convert a JSON Value to a HashMap for event details
fn json_to_details(value: &Value) -> HashMap<String, Value> {
    let mut details = HashMap::new();
    if let Some(obj) = value.as_object() {
        for (k, v) in obj {
            // Skip the standard fields
            if !matches!(k.as_str(), "timestamp" | "source" | "event_type" | "summary") {
                details.insert(k.clone(), v.clone());
            }
        }
    }
    details
}

/// Log a custom event to the journal JSONL file
pub fn log_event_to_file(source: &str, event_type: &str, summary: &str) -> Result<()> {
    let paths = Paths::new();
    let journal_dir = paths.data.join("journal");
    fs::create_dir_all(&journal_dir)?;

    let event = Event::new(source, event_type, summary);

    // Write to daily log file
    let date_str = chrono::Local::now().format("%Y-%m-%d");
    let log_file = journal_dir.join(format!("journal-{}.jsonl", date_str));

    let mut file = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log_file)?;

    use std::io::Write;
    writeln!(file, "{}", serde_json::to_string(&event)?)?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_event_creation() {
        let event = Event::new("test", "test_event", "Test summary");
        assert_eq!(event.source, "test");
        assert_eq!(event.event_type, "test_event");
        assert_eq!(event.summary, "Test summary");
        assert!(event.timestamp > 0.0);
    }

    #[test]
    fn test_collector_creation() {
        let collector = EventCollector::new();
        // Should not panic
        let events = collector.collect_all(0.0, None, None, 100);
        // May or may not have events depending on system state
        assert!(events.len() <= 100);
    }
}
