//! Gate checking and history logging
//!
//! Handles gate checks, approval flows, and history recording.

use anyhow::{Context, Result};
use chrono::{DateTime, Local, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs::{self, File, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;

use crate::config::{load_project_config, GateAction, SupervisionConfig};

/// Get the data directory for gate history
pub fn get_data_dir() -> PathBuf {
    dirs::data_dir()
        .unwrap_or_else(|| PathBuf::from("~/.local/share"))
        .join("daedalos")
        .join("gates")
}

/// A request to pass through a gate
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GateRequest {
    pub gate: String,
    pub action: String,
    pub context: HashMap<String, serde_json::Value>,
    pub timestamp: f64,
    pub source: String,
}

/// Result of a gate check
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GateResult {
    pub allowed: bool,
    pub action: String,
    pub reason: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub approved_by: Option<String>,
}

/// Event logged to history
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GateEvent {
    pub timestamp: f64,
    pub gate: String,
    pub action: String,
    pub context: HashMap<String, serde_json::Value>,
    pub source: String,
    pub result: GateEventResult,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GateEventResult {
    pub allowed: bool,
    pub action: String,
    pub reason: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub approved_by: Option<String>,
}

impl From<&GateResult> for GateEventResult {
    fn from(result: &GateResult) -> Self {
        Self {
            allowed: result.allowed,
            action: result.action.clone(),
            reason: result.reason.clone(),
            approved_by: result.approved_by.clone(),
        }
    }
}

/// Ensure data directory exists
fn ensure_data_dir() -> Result<PathBuf> {
    let data_dir = get_data_dir();
    fs::create_dir_all(&data_dir)
        .with_context(|| format!("Failed to create data directory: {}", data_dir.display()))?;
    Ok(data_dir)
}

/// Log a gate event for history/audit
fn log_gate_event(request: &GateRequest, result: &GateResult) -> Result<()> {
    let data_dir = ensure_data_dir()?;

    let event = GateEvent {
        timestamp: request.timestamp,
        gate: request.gate.clone(),
        action: request.action.clone(),
        context: request.context.clone(),
        source: request.source.clone(),
        result: result.into(),
    };

    // Append to daily log file
    let timestamp = request.timestamp as i64;
    let datetime = DateTime::from_timestamp(timestamp, 0)
        .unwrap_or_else(|| Utc::now());
    let date_str = datetime.format("%Y-%m-%d").to_string();
    let log_file = data_dir.join(format!("gates-{}.jsonl", date_str));

    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log_file)
        .with_context(|| format!("Failed to open log file: {}", log_file.display()))?;

    let json = serde_json::to_string(&event).context("Failed to serialize event")?;
    writeln!(file, "{}", json).context("Failed to write to log file")?;

    Ok(())
}

/// Check if an action is allowed through a gate
pub fn check_gate(
    gate: &str,
    context: Option<HashMap<String, serde_json::Value>>,
    source: &str,
    config: Option<&SupervisionConfig>,
) -> Result<GateResult> {
    let config = match config {
        Some(c) => c.clone(),
        None => load_project_config(None)?,
    };

    let context = context.unwrap_or_default();
    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs_f64();

    // Check for sensitive path override
    let gate_action = if let Some(path) = context.get("path").and_then(|v| v.as_str()) {
        if config.is_sensitive_path(path) {
            config.get_gate("sensitive_file")
        } else {
            config.get_gate(gate)
        }
    } else {
        config.get_gate(gate)
    };

    let request = GateRequest {
        gate: gate.to_string(),
        action: gate_action.as_str().to_string(),
        context,
        timestamp,
        source: source.to_string(),
    };

    let result = match gate_action {
        GateAction::Allow => GateResult {
            allowed: true,
            action: "allow".into(),
            reason: "Gate configured to allow".into(),
            approved_by: Some("auto".into()),
        },
        GateAction::Notify => {
            // In non-interactive mode, we just log and proceed
            GateResult {
                allowed: true,
                action: "notify".into(),
                reason: "User notified, proceeding".into(),
                approved_by: Some("auto".into()),
            }
        }
        GateAction::Deny => GateResult {
            allowed: false,
            action: "deny".into(),
            reason: "Gate configured to deny".into(),
            approved_by: Some("auto".into()),
        },
        GateAction::Approve => {
            // Non-interactive mode - approval required but we can't prompt
            GateResult {
                allowed: false,
                action: "approve".into(),
                reason: "Approval required but running non-interactively".into(),
                approved_by: None,
            }
        }
    };

    // Log the event
    if let Err(e) = log_gate_event(&request, &result) {
        eprintln!("Warning: Failed to log gate event: {}", e);
    }

    Ok(result)
}

/// Get gate check history
pub fn get_gate_history(
    gate: Option<&str>,
    days: u32,
    limit: usize,
) -> Result<Vec<GateEvent>> {
    let data_dir = ensure_data_dir()?;
    let mut events = Vec::new();

    let today = Local::now().date_naive();

    // Search from tomorrow (UTC may be ahead of local) back to days ago
    for i in -1..days as i64 {
        let date = today - chrono::Duration::days(i);
        let date_str = date.format("%Y-%m-%d").to_string();
        let log_file = data_dir.join(format!("gates-{}.jsonl", date_str));

        if log_file.exists() {
            let file = File::open(&log_file)?;
            let reader = BufReader::new(file);

            for line in reader.lines() {
                if let Ok(line) = line {
                    if let Ok(event) = serde_json::from_str::<GateEvent>(&line) {
                        if gate.is_none() || gate == Some(event.gate.as_str()) {
                            events.push(event);
                        }
                    }
                }
            }
        }
    }

    // Sort by timestamp descending
    events.sort_by(|a, b| b.timestamp.partial_cmp(&a.timestamp).unwrap_or(std::cmp::Ordering::Equal));

    // Apply limit
    events.truncate(limit);

    Ok(events)
}

/// Check autonomy limits
#[allow(dead_code)]
pub fn check_autonomy_limits(
    config: &SupervisionConfig,
    iterations: u32,
    file_changes: u32,
    lines_changed: u32,
) -> Option<String> {
    if iterations > config.autonomy.max_iterations {
        return Some(format!(
            "Exceeded max iterations ({}/{})",
            iterations, config.autonomy.max_iterations
        ));
    }

    if file_changes > config.autonomy.max_file_changes {
        return Some(format!(
            "Exceeded max file changes ({}/{})",
            file_changes, config.autonomy.max_file_changes
        ));
    }

    if lines_changed > config.autonomy.max_lines_changed {
        return Some(format!(
            "Exceeded max lines changed ({}/{})",
            lines_changed, config.autonomy.max_lines_changed
        ));
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_gate_result_serialization() {
        let result = GateResult {
            allowed: true,
            action: "allow".into(),
            reason: "Test".into(),
            approved_by: Some("auto".into()),
        };
        let json = serde_json::to_string(&result).unwrap();
        assert!(json.contains("allowed"));
    }

    #[test]
    fn test_check_autonomy_limits() {
        let config = SupervisionConfig::default();

        // Under limits
        assert!(check_autonomy_limits(&config, 10, 50, 500).is_none());

        // Over iterations limit
        let result = check_autonomy_limits(&config, 100, 50, 500);
        assert!(result.is_some());
        assert!(result.unwrap().contains("iterations"));
    }
}
