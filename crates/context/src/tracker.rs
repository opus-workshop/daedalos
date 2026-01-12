//! Context tracking for Claude Code sessions
//!
//! Tracks and analyzes conversation history to estimate context usage.
//! Read-only - never modifies conversation history.

use std::fs;
use std::path::PathBuf;

use anyhow::{Context, Result};
use chrono::Utc;
use serde::{Deserialize, Serialize};

use crate::estimator::TokenEstimator;

/// Claude's default context window size (200K tokens)
const DEFAULT_MAX_CONTEXT: usize = 200_000;

/// Token breakdown by category
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TokenBreakdown {
    pub system: usize,
    pub user: usize,
    pub assistant: usize,
    pub tool_calls: usize,
    pub tool_results: usize,
    pub files_read: usize,
}

impl TokenBreakdown {
    pub fn total(&self) -> usize {
        self.system + self.user + self.assistant + self.tool_calls + self.tool_results + self.files_read
    }
}

/// Warning level based on context usage
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum WarningLevel {
    Ok,
    Moderate,
    High,
    Critical,
}

impl WarningLevel {
    pub fn from_percentage(pct: f64) -> Self {
        if pct < 50.0 {
            WarningLevel::Ok
        } else if pct < 70.0 {
            WarningLevel::Moderate
        } else if pct < 85.0 {
            WarningLevel::High
        } else {
            WarningLevel::Critical
        }
    }
}

/// Context status summary
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextStatus {
    pub used: usize,
    pub max: usize,
    pub percentage: f64,
    pub remaining: usize,
    pub breakdown: TokenBreakdown,
    pub warning_level: WarningLevel,
}

/// File tracked in context
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileInContext {
    pub path: String,
    pub tokens: usize,
    pub size: usize,
}

/// Compaction suggestion
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompactionSuggestion {
    #[serde(rename = "type")]
    pub suggestion_type: String,
    pub description: String,
    pub savings: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub files: Option<Vec<FileInContext>>,
}

/// Checkpoint data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CheckpointData {
    pub name: String,
    pub created: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub project: Option<String>,
    pub status: ContextStatus,
    pub files: Vec<FileInContext>,
}

/// Checkpoint summary for listing
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CheckpointSummary {
    pub name: String,
    pub created: String,
    pub tokens: usize,
}

/// Context tracker for Claude Code sessions
pub struct ContextTracker {
    estimator: TokenEstimator,
    claude_dir: PathBuf,
    project_path: Option<PathBuf>,
    max_context: usize,
}

impl ContextTracker {
    /// Create a new context tracker
    pub fn new(project_path: Option<PathBuf>) -> Result<Self> {
        let claude_dir = dirs::home_dir()
            .context("Could not determine home directory")?
            .join(".claude");

        let project_path = project_path.or_else(|| Self::detect_project(&claude_dir));

        Ok(Self {
            estimator: TokenEstimator::new(),
            claude_dir,
            project_path,
            max_context: DEFAULT_MAX_CONTEXT,
        })
    }

    /// Detect current project from Claude Code state
    fn detect_project(claude_dir: &PathBuf) -> Option<PathBuf> {
        // Try current directory first
        let cwd = std::env::current_dir().ok()?;
        if cwd.join(".git").exists() {
            return Some(cwd);
        }

        // Check Claude's project directories
        let projects_dir = claude_dir.join("projects");
        if projects_dir.exists() {
            // Find most recently modified project
            let mut latest: Option<PathBuf> = None;
            let mut latest_time = std::time::SystemTime::UNIX_EPOCH;

            if let Ok(entries) = fs::read_dir(&projects_dir) {
                for entry in entries.flatten() {
                    let path = entry.path();
                    if path.is_dir() {
                        if let Ok(metadata) = path.metadata() {
                            if let Ok(mtime) = metadata.modified() {
                                if mtime > latest_time {
                                    latest_time = mtime;
                                    latest = Some(path);
                                }
                            }
                        }
                    }
                }
            }

            if latest.is_some() {
                return latest;
            }
        }

        Some(cwd)
    }

    /// Find conversation history file
    fn find_history(&self) -> Option<PathBuf> {
        let mut locations = vec![
            self.claude_dir.join("conversation.jsonl"),
            self.claude_dir.join("history.jsonl"),
        ];

        if let Some(ref project_path) = self.project_path {
            if let Some(project_name) = project_path.file_name() {
                let project_id = project_name.to_string_lossy();
                locations.push(
                    self.claude_dir
                        .join("projects")
                        .join(&*project_id)
                        .join("conversation.jsonl"),
                );
                locations.push(
                    self.claude_dir
                        .join("projects")
                        .join(&*project_id)
                        .join("history.jsonl"),
                );
            }
        }

        locations.into_iter().find(|loc| loc.exists())
    }

    /// Analyze conversation and return token breakdown
    fn get_conversation_tokens(&self) -> TokenBreakdown {
        let history_file = match self.find_history() {
            Some(f) => f,
            None => return self.estimate_current_session(),
        };

        let content = match fs::read_to_string(&history_file) {
            Ok(c) => c,
            Err(_) => return self.estimate_current_session(),
        };

        let mut tokens = TokenBreakdown::default();

        for line in content.lines() {
            if line.trim().is_empty() {
                continue;
            }

            if let Ok(entry) = serde_json::from_str::<serde_json::Value>(line) {
                let role = entry.get("role").and_then(|r| r.as_str()).unwrap_or("");
                let count = self.count_content_tokens(&entry);

                match role {
                    "system" => tokens.system += count,
                    "user" => tokens.user += count,
                    "assistant" => {
                        tokens.assistant += count;
                        // Check for tool calls
                        if let Some(tool_calls) = entry.get("tool_calls").and_then(|tc| tc.as_array()) {
                            for tc in tool_calls {
                                tokens.tool_calls += self.estimator.count(&tc.to_string());
                            }
                        }
                    }
                    "tool" | "tool_result" => tokens.tool_results += count,
                    _ => {}
                }
            }
        }

        tokens
    }

    /// Count tokens in a message's content
    fn count_content_tokens(&self, entry: &serde_json::Value) -> usize {
        if let Some(content) = entry.get("content") {
            if let Some(text) = content.as_str() {
                return self.estimator.count(text);
            }
            if let Some(arr) = content.as_array() {
                return arr
                    .iter()
                    .filter_map(|c| c.get("text").and_then(|t| t.as_str()))
                    .map(|t| self.estimator.count(t))
                    .sum();
            }
        }
        0
    }

    /// Estimate tokens for current session when history unavailable
    fn estimate_current_session(&self) -> TokenBreakdown {
        // Base estimates for a typical session
        TokenBreakdown {
            system: 8_000,       // System prompt + instructions
            user: 5_000,         // Estimated user messages
            assistant: 15_000,   // Estimated responses
            tool_calls: 3_000,
            tool_results: 20_000,
            files_read: 10_000,
        }
    }

    /// Get current context status
    pub fn get_status(&self) -> Result<ContextStatus> {
        let breakdown = self.get_conversation_tokens();
        let total = breakdown.total();
        let percentage = (total as f64 / self.max_context as f64) * 100.0;

        Ok(ContextStatus {
            used: total,
            max: self.max_context,
            percentage,
            remaining: self.max_context.saturating_sub(total),
            warning_level: WarningLevel::from_percentage(percentage),
            breakdown,
        })
    }

    /// Get list of files that have been read into context
    pub fn get_files_in_context(&self) -> Result<Vec<FileInContext>> {
        let mut files = Vec::new();

        let history_file = match self.find_history() {
            Some(f) => f,
            None => return Ok(files),
        };

        let content = match fs::read_to_string(&history_file) {
            Ok(c) => c,
            Err(_) => return Ok(files),
        };

        for line in content.lines() {
            if line.trim().is_empty() {
                continue;
            }

            if let Ok(entry) = serde_json::from_str::<serde_json::Value>(line) {
                let role = entry.get("role").and_then(|r| r.as_str()).unwrap_or("");

                if role == "tool_result" {
                    let tool_name = entry.get("name").and_then(|n| n.as_str()).unwrap_or("");

                    if matches!(tool_name, "Read" | "read_file" | "cat" | "View") {
                        let content_text = entry
                            .get("content")
                            .and_then(|c| c.as_str())
                            .unwrap_or("");
                        let file_path = entry
                            .get("file_path")
                            .and_then(|p| p.as_str())
                            .unwrap_or("unknown");

                        let tokens = self.estimator.count(content_text);
                        files.push(FileInContext {
                            path: file_path.to_string(),
                            tokens,
                            size: content_text.len(),
                        });
                    }
                }
            }
        }

        // Sort by token count descending
        files.sort_by(|a, b| b.tokens.cmp(&a.tokens));

        Ok(files)
    }

    /// Get suggestions for reducing context usage
    pub fn get_compaction_suggestions(&self) -> Result<Vec<CompactionSuggestion>> {
        let mut suggestions = Vec::new();
        let status = self.get_status()?;

        // Large files suggestion
        let files = self.get_files_in_context()?;
        let large_files: Vec<_> = files.into_iter().filter(|f| f.tokens > 5000).collect();

        if !large_files.is_empty() {
            let savings: usize = large_files.iter().take(3).map(|f| f.tokens).sum();
            suggestions.push(CompactionSuggestion {
                suggestion_type: "large_files".to_string(),
                description: "Consider re-reading only relevant sections of large files".to_string(),
                savings: (savings as f64 * 0.7) as usize,
                files: Some(large_files.into_iter().take(3).collect()),
            });
        }

        // Tool results suggestion
        if status.breakdown.tool_results > 30_000 {
            suggestions.push(CompactionSuggestion {
                suggestion_type: "tool_results".to_string(),
                description: "Summarize verbose tool results instead of keeping full output"
                    .to_string(),
                savings: (status.breakdown.tool_results as f64 * 0.5) as usize,
                files: None,
            });
        }

        // Conversation history suggestion
        if status.percentage > 70.0 {
            suggestions.push(CompactionSuggestion {
                suggestion_type: "conversation".to_string(),
                description: "Consider starting a new session with a summary".to_string(),
                savings: (status.used as f64 * 0.6) as usize,
                files: None,
            });
        }

        Ok(suggestions)
    }

    /// Create a context checkpoint for later restoration
    pub fn checkpoint(&self, name: &str) -> Result<CheckpointData> {
        let checkpoint_dir = self.claude_dir.join("checkpoints");
        fs::create_dir_all(&checkpoint_dir)?;

        let status = self.get_status()?;
        let files = self.get_files_in_context()?;

        let checkpoint_data = CheckpointData {
            name: name.to_string(),
            created: Utc::now().to_rfc3339(),
            project: self.project_path.as_ref().map(|p| p.display().to_string()),
            status,
            files,
        };

        let checkpoint_file = checkpoint_dir.join(format!("{}.json", name));
        let json = serde_json::to_string_pretty(&checkpoint_data)?;
        fs::write(&checkpoint_file, json)?;

        Ok(checkpoint_data)
    }

    /// List available checkpoints
    pub fn list_checkpoints(&self) -> Result<Vec<CheckpointSummary>> {
        let checkpoint_dir = self.claude_dir.join("checkpoints");
        if !checkpoint_dir.exists() {
            return Ok(Vec::new());
        }

        let mut checkpoints = Vec::new();

        for entry in fs::read_dir(&checkpoint_dir)? {
            let entry = entry?;
            let path = entry.path();

            if path.extension().map(|e| e == "json").unwrap_or(false) {
                if let Ok(content) = fs::read_to_string(&path) {
                    if let Ok(data) = serde_json::from_str::<CheckpointData>(&content) {
                        checkpoints.push(CheckpointSummary {
                            name: data.name,
                            created: data.created,
                            tokens: data.status.used,
                        });
                    }
                }
            }
        }

        // Sort by creation time descending
        checkpoints.sort_by(|a, b| b.created.cmp(&a.created));

        Ok(checkpoints)
    }

    /// Restore from a checkpoint (returns checkpoint data)
    pub fn restore_checkpoint(&self, name: &str) -> Result<Option<CheckpointData>> {
        let checkpoint_file = self.claude_dir.join("checkpoints").join(format!("{}.json", name));

        if !checkpoint_file.exists() {
            return Ok(None);
        }

        let content = fs::read_to_string(&checkpoint_file)?;
        let data: CheckpointData = serde_json::from_str(&content)?;

        Ok(Some(data))
    }
}
