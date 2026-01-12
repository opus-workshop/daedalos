//! Narrative builder - synthesizes events into readable stories
//!
//! "What happened?" is the most important debugging question.
//! Raw events are overwhelming. A narrative is actionable.

use crate::db::Event;
use chrono::{Duration, Local, TimeZone, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Format timestamp for display
pub fn format_time(ts: f64) -> String {
    let dt = Utc.timestamp_opt(ts as i64, 0)
        .single()
        .unwrap_or_else(Utc::now);
    let local = dt.with_timezone(&Local);
    let now = Local::now();

    if local.date_naive() == now.date_naive() {
        local.format("%H:%M:%S").to_string()
    } else if local.date_naive() == (now - Duration::days(1)).date_naive() {
        format!("Yesterday {}", local.format("%H:%M"))
    } else {
        local.format("%Y-%m-%d %H:%M").to_string()
    }
}

/// Format duration in human-readable form
#[allow(dead_code)]
pub fn format_duration(seconds: f64) -> String {
    if seconds < 60.0 {
        format!("{}s", seconds as i64)
    } else if seconds < 3600.0 {
        let mins = (seconds / 60.0) as i64;
        let secs = (seconds % 60.0) as i64;
        format!("{}m {}s", mins, secs)
    } else {
        let hours = (seconds / 3600.0) as i64;
        let mins = ((seconds % 3600.0) / 60.0) as i64;
        format!("{}h {}m", hours, mins)
    }
}

/// Format timestamp as relative time (e.g., "2m ago")
#[allow(dead_code)]
pub fn format_relative_time(ts: f64) -> String {
    let dt = Utc.timestamp_opt(ts as i64, 0)
        .single()
        .unwrap_or_else(Utc::now);
    let now = Utc::now();
    let diff = now.signed_duration_since(dt);

    if diff.num_seconds() < 60 {
        "just now".to_string()
    } else if diff.num_minutes() < 60 {
        let mins = diff.num_minutes();
        format!("{} minute{} ago", mins, if mins != 1 { "s" } else { "" })
    } else if diff.num_hours() < 24 {
        let hours = diff.num_hours();
        format!("{} hour{} ago", hours, if hours != 1 { "s" } else { "" })
    } else {
        let days = diff.num_days();
        format!("{} day{} ago", days, if days != 1 { "s" } else { "" })
    }
}

/// Get an icon for an event source or type
fn get_icon(source: &str, event_type: &str) -> &'static str {
    // Note: Using simple ASCII characters for better terminal compatibility
    // Can be replaced with emojis if desired
    match event_type {
        "gate_check" => "[GATE]",
        "loop_started" => "[START]",
        "loop_completed" => "[DONE]",
        "loop_failed" => "[FAIL]",
        "loop_stopped" => "[STOP]",
        "agent_spawned" | "spawn" => "[AGENT]",
        "agent_killed" | "kill" => "[KILL]",
        "file_created" | "file_create" => "[+FILE]",
        "file_modified" | "file_edit" => "[~FILE]",
        "file_deleted" | "file_delete" => "[-FILE]",
        "checkpoint" => "[CHKPT]",
        _ => match source {
            "gates" => "[GATE]",
            "loop" => "[LOOP]",
            "agent" => "[AGENT]",
            "undo" => "[UNDO]",
            "mcp-hub" => "[MCP]",
            "user" | "journal" => "[USER]",
            _ => "[*]",
        },
    }
}

/// Summary statistics for events
#[derive(Debug, Serialize, Deserialize)]
pub struct Summary {
    pub total_events: usize,
    pub time_range: TimeRange,
    pub by_source: HashMap<String, usize>,
    pub by_type: HashMap<String, usize>,
    pub highlights: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TimeRange {
    pub start: Option<String>,
    pub end: Option<String>,
}

/// Build a summary of events
pub fn build_summary(events: &[Event]) -> Summary {
    let mut summary = Summary {
        total_events: events.len(),
        time_range: TimeRange {
            start: None,
            end: None,
        },
        by_source: HashMap::new(),
        by_type: HashMap::new(),
        highlights: Vec::new(),
    };

    if events.is_empty() {
        return summary;
    }

    // Calculate time range
    let timestamps: Vec<f64> = events.iter().map(|e| e.timestamp).collect();
    let min_ts = timestamps.iter().cloned().fold(f64::INFINITY, f64::min);
    let max_ts = timestamps.iter().cloned().fold(f64::NEG_INFINITY, f64::max);

    summary.time_range.start = Some(format_time(min_ts));
    summary.time_range.end = Some(format_time(max_ts));

    // Count by source and type
    for event in events {
        *summary.by_source.entry(event.source.clone()).or_insert(0) += 1;
        *summary.by_type.entry(event.event_type.clone()).or_insert(0) += 1;
    }

    // Generate highlights
    let mut highlights = Vec::new();

    // Check for loops
    let loop_events: Vec<_> = events.iter().filter(|e| e.source == "loop").collect();
    if !loop_events.is_empty() {
        let completed = loop_events.iter().filter(|e| e.event_type == "loop_completed").count();
        let failed = loop_events.iter().filter(|e| e.event_type == "loop_failed").count();
        if completed > 0 {
            highlights.push(format!("{} loop(s) completed", completed));
        }
        if failed > 0 {
            highlights.push(format!("{} loop(s) failed", failed));
        }
    }

    // Check for gate denials
    let gate_events: Vec<_> = events.iter().filter(|e| e.source == "gates").collect();
    let denied = gate_events.iter().filter(|e| e.summary.to_lowercase().contains("denied")).count();
    if denied > 0 {
        highlights.push(format!("{} gate check(s) denied", denied));
    }

    // Check for agents
    let agent_events: Vec<_> = events.iter().filter(|e| e.source == "agent").collect();
    if !agent_events.is_empty() {
        highlights.push(format!("{} agent event(s)", agent_events.len()));
    }

    // Check for file changes
    let file_events: Vec<_> = events.iter().filter(|e| e.source == "undo").collect();
    if !file_events.is_empty() {
        let checkpoints = file_events.iter().filter(|e| e.event_type == "checkpoint").count();
        let changes = file_events.len() - checkpoints;
        if changes > 0 {
            highlights.push(format!("{} file change(s)", changes));
        }
        if checkpoints > 0 {
            highlights.push(format!("{} checkpoint(s)", checkpoints));
        }
    }

    summary.highlights = highlights;
    summary
}

/// Build a human-readable narrative from events
pub fn build_narrative(events: &[Event], verbose: bool) -> String {
    if events.is_empty() {
        return "No events found in the specified time range.".to_string();
    }

    let mut lines = Vec::new();
    let now = Local::now();

    // Group events by time period
    let mut today_events = Vec::new();
    let mut yesterday_events = Vec::new();
    let mut older_events = Vec::new();

    for event in events {
        let dt = Utc.timestamp_opt(event.timestamp as i64, 0)
            .single()
            .unwrap_or_else(Utc::now)
            .with_timezone(&Local);

        if dt.date_naive() == now.date_naive() {
            today_events.push(event);
        } else if dt.date_naive() == (now - Duration::days(1)).date_naive() {
            yesterday_events.push(event);
        } else {
            older_events.push(event);
        }
    }

    // Build narrative sections
    if !today_events.is_empty() {
        lines.push("## Today".to_string());
        lines.push(String::new());
        lines.extend(format_event_group(&today_events, verbose));
    }

    if !yesterday_events.is_empty() {
        lines.push(String::new());
        lines.push("## Yesterday".to_string());
        lines.push(String::new());
        lines.extend(format_event_group(&yesterday_events, verbose));
    }

    if !older_events.is_empty() {
        lines.push(String::new());
        lines.push("## Earlier".to_string());
        lines.push(String::new());
        lines.extend(format_event_group(&older_events, verbose));
    }

    lines.join("\n")
}

/// Format a group of events
fn format_event_group(events: &[&Event], verbose: bool) -> Vec<String> {
    let mut lines = Vec::new();

    for event in events {
        let time_str = format_time(event.timestamp);
        let icon = get_icon(&event.source, &event.event_type);

        if verbose {
            lines.push(format!("{}  {} [{}] {}", time_str, icon, event.source, event.summary));
            for (key, value) in &event.details {
                if !matches!(key.as_str(), "timestamp" | "source" | "event_type" | "summary") {
                    let value_str = match value {
                        serde_json::Value::String(s) => s.clone(),
                        _ => value.to_string(),
                    };
                    lines.push(format!("           {}: {}", key, value_str));
                }
            }
        } else {
            lines.push(format!("{}  {} {}", time_str, icon, event.summary));
        }
    }

    lines
}

/// Generate the "what happened" narrative
pub fn what_happened(events: &[Event], hours: f64, verbose: bool) -> String {
    if events.is_empty() {
        let period = if hours == 1.0 {
            "the last hour".to_string()
        } else {
            format!("the last {} hours", hours as i64)
        };
        return format!("Nothing recorded in {}.", period);
    }

    let summary = build_summary(events);
    let mut lines = Vec::new();

    // Header with summary
    let period = if hours == 1.0 {
        "the last hour".to_string()
    } else {
        format!("the last {} hours", hours as i64)
    };
    lines.push(format!("## What happened in {}?", period));
    lines.push(String::new());

    if !summary.highlights.is_empty() {
        lines.push(format!("**Highlights:** {}", summary.highlights.join(" | ")));
        lines.push(String::new());
    }

    let time_range = match (&summary.time_range.start, &summary.time_range.end) {
        (Some(start), Some(end)) => format!("{} to {}", start, end),
        _ => "unknown".to_string(),
    };
    lines.push(format!("**{} events** from {}", summary.total_events, time_range));
    lines.push(String::new());

    // Activity by source
    if !summary.by_source.is_empty() {
        let mut sources: Vec<_> = summary.by_source.iter().collect();
        sources.sort_by(|a, b| b.1.cmp(a.1));
        let sources_str: Vec<_> = sources.iter().map(|(k, v)| format!("{}: {}", k, v)).collect();
        lines.push(format!("By source: {}", sources_str.join(", ")));
        lines.push(String::new());
    }

    // Detailed narrative
    lines.push("---".to_string());
    lines.push(String::new());
    lines.push(build_narrative(events, verbose));

    lines.join("\n")
}

/// Format summary for display (non-JSON)
pub fn format_summary(summary: &Summary) -> String {
    let mut lines = Vec::new();

    lines.push(format!("Total events: {}", summary.total_events));

    let time_range = match (&summary.time_range.start, &summary.time_range.end) {
        (Some(start), Some(end)) => format!("{} - {}", start, end),
        _ => "N/A".to_string(),
    };
    lines.push(format!("Time range: {}", time_range));
    lines.push(String::new());

    if !summary.highlights.is_empty() {
        lines.push("Highlights:".to_string());
        for h in &summary.highlights {
            lines.push(format!("  * {}", h));
        }
        lines.push(String::new());
    }

    if !summary.by_source.is_empty() {
        lines.push("By source:".to_string());
        let mut sources: Vec<_> = summary.by_source.iter().collect();
        sources.sort_by(|a, b| b.1.cmp(a.1));
        for (source, count) in sources {
            lines.push(format!("  {}: {}", source, count));
        }
        lines.push(String::new());
    }

    if !summary.by_type.is_empty() {
        lines.push("By event type:".to_string());
        let mut types: Vec<_> = summary.by_type.iter().collect();
        types.sort_by(|a, b| b.1.cmp(a.1));
        for (etype, count) in types {
            lines.push(format!("  {}: {}", etype, count));
        }
    }

    lines.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_time() {
        let now = Utc::now().timestamp() as f64;
        let formatted = format_time(now);
        assert!(formatted.contains(':'));
    }

    #[test]
    fn test_format_duration() {
        assert_eq!(format_duration(30.0), "30s");
        assert_eq!(format_duration(90.0), "1m 30s");
        assert_eq!(format_duration(3700.0), "1h 1m");
    }

    #[test]
    fn test_format_relative_time() {
        let now = Utc::now().timestamp() as f64;
        assert_eq!(format_relative_time(now), "just now");
    }

    #[test]
    fn test_build_summary_empty() {
        let summary = build_summary(&[]);
        assert_eq!(summary.total_events, 0);
        assert!(summary.highlights.is_empty());
    }

    #[test]
    fn test_build_narrative_empty() {
        let narrative = build_narrative(&[], false);
        assert!(narrative.contains("No events found"));
    }
}
