//! Review - Human code review workflow for Daedalos
//!
//! "Trust but verify. Especially when AI is writing code."
//!
//! The review tool creates a structured checkpoint between "code written" and
//! "code deployed." It's the human-in-the-loop for AI development.
//!
//! When an agent finishes a task, it requests review. A human examines the
//! changes, asks questions, and explicitly approves or rejects. This creates
//! accountability and catches issues before they compound.

use anyhow::{bail, Context, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

/// Review status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ReviewStatus {
    /// Waiting for review
    Pending,
    /// Changes approved
    Approved,
    /// Changes rejected
    Rejected,
}

impl ReviewStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            ReviewStatus::Pending => "pending",
            ReviewStatus::Approved => "approved",
            ReviewStatus::Rejected => "rejected",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "pending" => Some(ReviewStatus::Pending),
            "approved" => Some(ReviewStatus::Approved),
            "rejected" => Some(ReviewStatus::Rejected),
            _ => None,
        }
    }
}

/// A code review request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReviewRequest {
    /// Unique review ID
    pub id: String,
    /// Current status
    pub status: ReviewStatus,
    /// Git ref for the changes (e.g., "HEAD^..HEAD", "--cached")
    pub git_ref: String,
    /// Optional message describing the changes
    pub message: Option<String>,
    /// Who requested the review
    pub requested_by: String,
    /// When the review was requested
    pub requested_at: DateTime<Utc>,
    /// Project directory
    pub project: PathBuf,
    /// Git branch name
    pub branch: String,
    /// Number of files changed
    pub file_count: usize,
    /// Git diff stats summary
    pub stats: String,
}

/// A comment on a review
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReviewComment {
    /// Who made the comment
    pub by: String,
    /// When the comment was made
    pub at: DateTime<Utc>,
    /// The comment text
    pub comment: String,
}

/// Review decision (approval or rejection)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReviewDecision {
    /// Who made the decision
    pub by: String,
    /// When the decision was made
    pub at: DateTime<Utc>,
    /// Optional comment
    pub comment: Option<String>,
    /// The decision type
    pub decision: ReviewStatus,
}

/// Review store - manages persistence of review requests
pub struct ReviewStore {
    /// Directory where reviews are stored
    root: PathBuf,
}

impl ReviewStore {
    /// Create a new review store
    pub fn new(root: &Path) -> Result<Self> {
        fs::create_dir_all(root)
            .with_context(|| format!("Failed to create review directory: {}", root.display()))?;

        Ok(Self {
            root: root.to_path_buf(),
        })
    }

    /// Generate a unique review ID
    pub fn generate_id() -> String {
        let timestamp = chrono::Utc::now().format("%Y%m%d%H%M%S");
        let random: u32 = rand_u32();
        format!("review-{}-{:08x}", timestamp, random)
    }

    /// Get the path for a review file
    fn review_path(&self, id: &str) -> PathBuf {
        self.root.join(format!("{}.json", id))
    }

    /// Get the path for comments file
    fn comments_path(&self, id: &str) -> PathBuf {
        self.root.join(format!("{}.comments.json", id))
    }

    /// Get the path for decision file
    fn decision_path(&self, id: &str) -> PathBuf {
        self.root.join(format!("{}.decision.json", id))
    }

    /// Check if a review exists
    pub fn exists(&self, id: &str) -> bool {
        self.review_path(id).exists()
    }

    /// Save a review request
    pub fn save(&self, review: &ReviewRequest) -> Result<()> {
        let path = self.review_path(&review.id);
        let content = serde_json::to_string_pretty(review)
            .context("Failed to serialize review")?;
        fs::write(&path, content)
            .with_context(|| format!("Failed to write review: {}", path.display()))
    }

    /// Load a review request
    pub fn load(&self, id: &str) -> Result<ReviewRequest> {
        let path = self.review_path(id);
        let content = fs::read_to_string(&path)
            .with_context(|| format!("Failed to read review: {}", path.display()))?;
        serde_json::from_str(&content).context("Failed to parse review JSON")
    }

    /// Update a review's status
    pub fn update_status(&self, id: &str, status: ReviewStatus) -> Result<()> {
        let mut review = self.load(id)?;
        review.status = status;
        self.save(&review)
    }

    /// List all reviews, optionally filtered by status
    pub fn list(&self, status_filter: Option<ReviewStatus>) -> Result<Vec<ReviewRequest>> {
        let mut reviews = Vec::new();

        if !self.root.exists() {
            return Ok(reviews);
        }

        for entry in fs::read_dir(&self.root)? {
            let entry = entry?;
            let path = entry.path();

            // Only look at main review files (not .comments.json or .decision.json)
            if path.extension().map(|e| e == "json").unwrap_or(false) {
                let filename = path.file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or("");

                // Skip auxiliary files
                if filename.ends_with(".comments") || filename.ends_with(".decision") {
                    continue;
                }

                if let Ok(content) = fs::read_to_string(&path) {
                    if let Ok(review) = serde_json::from_str::<ReviewRequest>(&content) {
                        // Apply status filter
                        if let Some(filter) = status_filter {
                            if review.status != filter {
                                continue;
                            }
                        }
                        reviews.push(review);
                    }
                }
            }
        }

        // Sort by request time (newest first)
        reviews.sort_by(|a, b| b.requested_at.cmp(&a.requested_at));

        Ok(reviews)
    }

    /// Get the most recent pending review
    pub fn most_recent_pending(&self) -> Result<Option<ReviewRequest>> {
        let reviews = self.list(Some(ReviewStatus::Pending))?;
        Ok(reviews.into_iter().next())
    }

    /// Add a comment to a review
    pub fn add_comment(&self, id: &str, comment: ReviewComment) -> Result<()> {
        let path = self.comments_path(id);

        let mut comments: Vec<ReviewComment> = if path.exists() {
            let content = fs::read_to_string(&path)?;
            serde_json::from_str(&content).unwrap_or_default()
        } else {
            Vec::new()
        };

        comments.push(comment);

        let content = serde_json::to_string_pretty(&comments)?;
        fs::write(&path, content)?;

        Ok(())
    }

    /// Get comments for a review
    pub fn get_comments(&self, id: &str) -> Result<Vec<ReviewComment>> {
        let path = self.comments_path(id);

        if !path.exists() {
            return Ok(Vec::new());
        }

        let content = fs::read_to_string(&path)?;
        let comments: Vec<ReviewComment> = serde_json::from_str(&content)?;
        Ok(comments)
    }

    /// Save a review decision
    pub fn save_decision(&self, id: &str, decision: ReviewDecision) -> Result<()> {
        let path = self.decision_path(id);
        let content = serde_json::to_string_pretty(&decision)?;
        fs::write(&path, content)?;
        Ok(())
    }

    /// Get the decision for a review
    pub fn get_decision(&self, id: &str) -> Result<Option<ReviewDecision>> {
        let path = self.decision_path(id);

        if !path.exists() {
            return Ok(None);
        }

        let content = fs::read_to_string(&path)?;
        let decision: ReviewDecision = serde_json::from_str(&content)?;
        Ok(Some(decision))
    }
}

/// Git utilities for review
pub struct GitInfo;

impl GitInfo {
    /// Check if we're in a git repository
    pub fn is_git_repo() -> bool {
        Command::new("git")
            .args(["rev-parse", "--git-dir"])
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
    }

    /// Get the current branch name
    pub fn current_branch() -> Result<String> {
        let output = Command::new("git")
            .args(["branch", "--show-current"])
            .output()
            .context("Failed to run git branch")?;

        if !output.status.success() {
            bail!("Failed to get current branch");
        }

        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    }

    /// Check if there are staged changes
    pub fn has_staged_changes() -> bool {
        Command::new("git")
            .args(["diff", "--cached", "--quiet"])
            .status()
            .map(|s| !s.success())
            .unwrap_or(false)
    }

    /// Get diff stats for a ref
    pub fn diff_stats(git_ref: &str) -> Result<(usize, String)> {
        let (file_count_output, stat_output) = if git_ref == "--cached" {
            let files = Command::new("git")
                .args(["diff", "--cached", "--name-only"])
                .output()
                .context("Failed to get diff files")?;

            let stats = Command::new("git")
                .args(["diff", "--cached", "--stat"])
                .output()
                .context("Failed to get diff stats")?;

            (files, stats)
        } else {
            let files = Command::new("git")
                .args(["diff", git_ref, "--name-only"])
                .output()
                .context("Failed to get diff files")?;

            let stats = Command::new("git")
                .args(["diff", git_ref, "--stat"])
                .output()
                .context("Failed to get diff stats")?;

            (files, stats)
        };

        let file_count = String::from_utf8_lossy(&file_count_output.stdout)
            .lines()
            .filter(|l| !l.is_empty())
            .count();

        let stats = String::from_utf8_lossy(&stat_output.stdout)
            .lines()
            .last()
            .unwrap_or("")
            .trim()
            .to_string();

        Ok((file_count, stats))
    }

    /// Show diff for a ref (returns the diff output)
    pub fn show_diff(git_ref: &str, stat_only: bool) -> Result<String> {
        let mut args = vec!["diff"];

        if git_ref == "--cached" {
            args.push("--cached");
        } else {
            args.push(git_ref);
        }

        if stat_only {
            args.push("--stat");
        }

        let output = Command::new("git")
            .args(&args)
            .output()
            .context("Failed to run git diff")?;

        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    }
}

/// Get current username
pub fn current_user() -> String {
    std::env::var("USER")
        .or_else(|_| std::env::var("USERNAME"))
        .unwrap_or_else(|_| "unknown".to_string())
}

/// Simple random u32 generator (no external deps)
fn rand_u32() -> u32 {
    use std::time::{SystemTime, UNIX_EPOCH};
    let duration = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    // Mix nanoseconds and seconds for pseudo-randomness
    ((duration.as_nanos() as u64 ^ duration.as_secs()) & 0xFFFFFFFF) as u32
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;

    #[test]
    fn test_review_status_roundtrip() {
        for status in [ReviewStatus::Pending, ReviewStatus::Approved, ReviewStatus::Rejected] {
            let s = status.as_str();
            let parsed = ReviewStatus::from_str(s).unwrap();
            assert_eq!(status, parsed);
        }
    }

    #[test]
    fn test_review_status_invalid() {
        assert!(ReviewStatus::from_str("invalid").is_none());
    }

    #[test]
    fn test_generate_id_format() {
        let id = ReviewStore::generate_id();
        assert!(id.starts_with("review-"));
        assert!(id.len() > 20); // Timestamp + random
    }

    #[test]
    fn test_store_creation() {
        let temp_dir = env::temp_dir().join("review_test_store");
        let _ = fs::remove_dir_all(&temp_dir);

        let _store = ReviewStore::new(&temp_dir).unwrap();
        assert!(temp_dir.exists());

        // Cleanup
        let _ = fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_store_save_load() {
        let temp_dir = env::temp_dir().join("review_test_save_load");
        let _ = fs::remove_dir_all(&temp_dir);

        let store = ReviewStore::new(&temp_dir).unwrap();

        let review = ReviewRequest {
            id: "test-review-123".to_string(),
            status: ReviewStatus::Pending,
            git_ref: "HEAD^..HEAD".to_string(),
            message: Some("Test review".to_string()),
            requested_by: "tester".to_string(),
            requested_at: Utc::now(),
            project: PathBuf::from("/tmp/test"),
            branch: "main".to_string(),
            file_count: 5,
            stats: "5 files changed".to_string(),
        };

        store.save(&review).unwrap();
        assert!(store.exists("test-review-123"));

        let loaded = store.load("test-review-123").unwrap();
        assert_eq!(loaded.id, review.id);
        assert_eq!(loaded.status, ReviewStatus::Pending);

        // Cleanup
        let _ = fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_list_empty() {
        let temp_dir = env::temp_dir().join("review_test_list_empty");
        let _ = fs::remove_dir_all(&temp_dir);

        let store = ReviewStore::new(&temp_dir).unwrap();
        let reviews = store.list(None).unwrap();
        assert!(reviews.is_empty());

        // Cleanup
        let _ = fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_update_status() {
        let temp_dir = env::temp_dir().join("review_test_update_status");
        let _ = fs::remove_dir_all(&temp_dir);

        let store = ReviewStore::new(&temp_dir).unwrap();

        let review = ReviewRequest {
            id: "test-status-update".to_string(),
            status: ReviewStatus::Pending,
            git_ref: "HEAD".to_string(),
            message: None,
            requested_by: "tester".to_string(),
            requested_at: Utc::now(),
            project: PathBuf::from("/tmp/test"),
            branch: "main".to_string(),
            file_count: 1,
            stats: "1 file changed".to_string(),
        };

        store.save(&review).unwrap();
        store.update_status("test-status-update", ReviewStatus::Approved).unwrap();

        let loaded = store.load("test-status-update").unwrap();
        assert_eq!(loaded.status, ReviewStatus::Approved);

        // Cleanup
        let _ = fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_comments() {
        let temp_dir = env::temp_dir().join("review_test_comments");
        let _ = fs::remove_dir_all(&temp_dir);

        let store = ReviewStore::new(&temp_dir).unwrap();

        let comment = ReviewComment {
            by: "reviewer".to_string(),
            at: Utc::now(),
            comment: "Looks good!".to_string(),
        };

        store.add_comment("test-review", comment).unwrap();

        let comments = store.get_comments("test-review").unwrap();
        assert_eq!(comments.len(), 1);
        assert_eq!(comments[0].comment, "Looks good!");

        // Cleanup
        let _ = fs::remove_dir_all(&temp_dir);
    }
}
