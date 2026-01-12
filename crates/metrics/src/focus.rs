//! Focus session statistics
//!
//! Reads focus session data from Daedalos focus tool logs.

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::Path;

/// Focus session entry from log file
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FocusSession {
    /// Session duration in minutes
    pub duration: u32,
    /// Whether the session was completed
    pub completed: bool,
    /// Session timestamp (optional)
    #[serde(default)]
    pub timestamp: Option<f64>,
    /// Session label/task (optional)
    #[serde(default)]
    pub label: Option<String>,
}

/// Aggregated focus statistics
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct FocusStats {
    /// Total number of sessions
    pub total_sessions: u32,
    /// Total focus minutes
    pub total_minutes: u32,
    /// Number of completed sessions
    pub completed: u32,
}

impl FocusStats {
    /// Calculate completion rate (0-100)
    pub fn completion_rate(&self) -> u32 {
        if self.total_sessions == 0 {
            0
        } else {
            (self.completed * 100) / self.total_sessions
        }
    }

    /// Get hours and minutes
    pub fn hours_minutes(&self) -> (u32, u32) {
        (self.total_minutes / 60, self.total_minutes % 60)
    }

    /// Average minutes per day given a day count
    pub fn avg_per_day(&self, days: u32) -> u32 {
        if days == 0 {
            0
        } else {
            self.total_minutes / days
        }
    }
}

/// Get focus stats for the last N days
pub fn get_focus_stats(days: u32, data_dir: &Path) -> Result<FocusStats> {
    let focus_dir = data_dir.join("focus");
    let mut stats = FocusStats::default();

    if !focus_dir.exists() {
        return Ok(stats);
    }

    for i in 0..days {
        let date = (chrono::Utc::now() - chrono::Duration::days(i as i64))
            .format("%Y-%m-%d")
            .to_string();

        let log_file = focus_dir.join(format!("sessions-{}.jsonl", date));
        if log_file.exists() {
            let day_stats = read_focus_sessions(&log_file)?;
            stats.total_sessions += day_stats.total_sessions;
            stats.total_minutes += day_stats.total_minutes;
            stats.completed += day_stats.completed;
        }
    }

    Ok(stats)
}

/// Get focus stats for a specific date
pub fn get_focus_stats_for_date(date: &str, data_dir: &Path) -> Result<FocusStats> {
    let focus_dir = data_dir.join("focus");
    let log_file = focus_dir.join(format!("sessions-{}.jsonl", date));

    if log_file.exists() {
        read_focus_sessions(&log_file)
    } else {
        Ok(FocusStats::default())
    }
}

/// Get daily focus stats for a range
pub fn get_daily_focus(days: u32, data_dir: &Path) -> Result<Vec<(String, FocusStats)>> {
    let focus_dir = data_dir.join("focus");
    let mut daily = Vec::new();

    for i in (0..days).rev() {
        let date = (chrono::Utc::now() - chrono::Duration::days(i as i64))
            .format("%Y-%m-%d")
            .to_string();

        let log_file = focus_dir.join(format!("sessions-{}.jsonl", date));
        let stats = if log_file.exists() {
            read_focus_sessions(&log_file)?
        } else {
            FocusStats::default()
        };

        daily.push((date, stats));
    }

    Ok(daily)
}

/// Read focus sessions from a JSONL file
fn read_focus_sessions(path: &Path) -> Result<FocusStats> {
    let file = File::open(path)?;
    let reader = BufReader::new(file);
    let mut stats = FocusStats::default();

    for line in reader.lines() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }

        // Try to parse as JSON
        if let Ok(session) = serde_json::from_str::<FocusSession>(&line) {
            stats.total_sessions += 1;
            stats.total_minutes += session.duration;
            if session.completed {
                stats.completed += 1;
            }
        } else {
            // Fallback: try to extract duration and completed from the line manually
            if let Some((duration, completed)) = parse_focus_line(&line) {
                stats.total_sessions += 1;
                stats.total_minutes += duration;
                if completed {
                    stats.completed += 1;
                }
            }
        }
    }

    Ok(stats)
}

/// Fallback parser for focus session lines
fn parse_focus_line(line: &str) -> Option<(u32, bool)> {
    // Try to find "duration": N pattern (with optional space after colon)
    let duration = line
        .find("\"duration\":")
        .and_then(|pos| {
            let rest = &line[pos + 11..];
            // Skip any whitespace after the colon
            let trimmed = rest.trim_start();
            // Find the first non-digit
            let end = trimmed.find(|c: char| !c.is_ascii_digit()).unwrap_or(trimmed.len());
            if end > 0 {
                trimmed[..end].parse::<u32>().ok()
            } else {
                None
            }
        })?;

    // Try to find "completed": true/false pattern (with or without space)
    let completed = line.contains("\"completed\": true") || line.contains("\"completed\":true");

    Some((duration, completed))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_focus_stats_calculations() {
        let stats = FocusStats {
            total_sessions: 10,
            total_minutes: 250,
            completed: 8,
        };

        assert_eq!(stats.completion_rate(), 80);
        assert_eq!(stats.hours_minutes(), (4, 10));
        assert_eq!(stats.avg_per_day(5), 50);
    }

    #[test]
    fn test_parse_focus_line() {
        let line = r#"{"duration": 25, "completed": true, "label": "coding"}"#;
        let (duration, completed) = parse_focus_line(line).unwrap();
        assert_eq!(duration, 25);
        assert!(completed);
    }

    #[test]
    fn test_parse_focus_line_incomplete() {
        let line = r#"{"duration": 30, "completed": false}"#;
        let (duration, completed) = parse_focus_line(line).unwrap();
        assert_eq!(duration, 30);
        assert!(!completed);
    }
}
