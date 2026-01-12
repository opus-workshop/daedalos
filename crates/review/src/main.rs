//! review - Human code review workflow for Daedalos
//!
//! "Trust but verify. Especially when AI is writing code."
//!
//! Commands:
//! - request [REF]: Request review for changes
//! - list: List pending reviews
//! - start [ID]: Start reviewing a request
//! - approve [ID]: Approve changes
//! - reject [ID]: Reject with comments
//! - comment [ID]: Add comment without decision
//! - show <ID>: Show review details

use anyhow::{bail, Result};
use chrono::Utc;
use clap::{Parser, Subcommand};
use daedalos_core::Paths;
use review::{
    current_user, GitInfo, ReviewComment, ReviewDecision, ReviewRequest,
    ReviewStatus, ReviewStore,
};
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "review")]
#[command(about = "Human code review workflow for AI-generated changes")]
#[command(version)]
#[command(after_help = r#"WHEN TO USE:
    After AI generates code changes that need human verification.
    Creates a structured review queue so nothing slips through.

WORKFLOW:
    1. AI runs: review request -m "Added auth feature"
    2. Human runs: review start (shows diff)
    3. Human runs: review approve -m "LGTM" OR review reject -m "needs work"

EXAMPLES:
    review request              # Request review for staged changes
    review request HEAD~3..HEAD # Request review for last 3 commits
    review list --pending       # Show reviews awaiting action
    review start                # Begin reviewing most recent
    review approve -m "LGTM"    # Approve with comment
    review reject -m "Fix X"    # Reject with required changes
    review show abc123          # Show details of specific review

ALIASES:
    review req, review r        # request
    review ls                   # list
    review lgtm, review ok      # approve
    review deny                 # reject
"#)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// Request review for changes
    #[command(visible_alias = "req", visible_alias = "r")]
    Request {
        /// Git ref for changes (default: staged changes or HEAD)
        #[arg(name = "REF")]
        git_ref: Option<String>,

        /// Description of the changes
        #[arg(short, long)]
        message: Option<String>,
    },

    /// List review requests
    #[command(visible_alias = "ls")]
    List {
        /// Show only pending reviews
        #[arg(long)]
        pending: bool,

        /// Show only approved reviews
        #[arg(long)]
        approved: bool,

        /// Show only rejected reviews
        #[arg(long)]
        rejected: bool,

        /// Output as JSON
        #[arg(long)]
        json: bool,
    },

    /// Start reviewing a request (shows diff)
    #[command(visible_alias = "begin")]
    Start {
        /// Review ID (default: most recent pending)
        #[arg(name = "ID")]
        id: Option<String>,
    },

    /// Approve changes
    #[command(visible_alias = "lgtm", visible_alias = "ok")]
    Approve {
        /// Review ID (default: most recent pending)
        #[arg(name = "ID")]
        id: Option<String>,

        /// Approval comment
        #[arg(short, long)]
        message: Option<String>,
    },

    /// Reject changes (requires message)
    #[command(visible_alias = "deny")]
    Reject {
        /// Review ID (default: most recent pending)
        #[arg(name = "ID")]
        id: Option<String>,

        /// Rejection reason (required)
        #[arg(short, long)]
        message: Option<String>,
    },

    /// Add a comment without decision
    #[command(visible_alias = "c")]
    Comment {
        /// Review ID (default: most recent pending)
        #[arg(name = "ID")]
        id: Option<String>,

        /// Comment text (required)
        #[arg(short, long)]
        message: Option<String>,
    },

    /// Show review details
    Show {
        /// Review ID
        #[arg(name = "ID")]
        id: String,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    let paths = Paths::new();
    let review_dir = paths.data.join("review");
    let store = ReviewStore::new(&review_dir)?;

    match cli.command {
        Some(Commands::Request { git_ref, message }) => cmd_request(&store, git_ref, message),
        Some(Commands::List { pending, approved, rejected, json }) => {
            let filter = if pending {
                Some(ReviewStatus::Pending)
            } else if approved {
                Some(ReviewStatus::Approved)
            } else if rejected {
                Some(ReviewStatus::Rejected)
            } else {
                None
            };
            cmd_list(&store, filter, json)
        }
        Some(Commands::Start { id }) => cmd_start(&store, id),
        Some(Commands::Approve { id, message }) => cmd_approve(&store, id, message),
        Some(Commands::Reject { id, message }) => cmd_reject(&store, id, message),
        Some(Commands::Comment { id, message }) => cmd_comment(&store, id, message),
        Some(Commands::Show { id }) => cmd_show(&store, &id),
        None => cmd_list(&store, None, false),
    }
}

/// Request review for changes
fn cmd_request(store: &ReviewStore, git_ref: Option<String>, message: Option<String>) -> Result<()> {
    // Must be in a git repository
    if !GitInfo::is_git_repo() {
        bail!("Not in a git repository");
    }

    // Determine what to review
    let effective_ref = if let Some(ref r) = git_ref {
        r.clone()
    } else if GitInfo::has_staged_changes() {
        "--cached".to_string()
    } else {
        "HEAD^..HEAD".to_string()
    };

    // Get stats
    let (file_count, stats) = GitInfo::diff_stats(&effective_ref)?;

    if file_count == 0 {
        bail!("No changes to review");
    }

    // Get branch and project info
    let branch = GitInfo::current_branch().unwrap_or_else(|_| "unknown".to_string());
    let project = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));

    // Create review request
    let review_id = ReviewStore::generate_id();
    let review = ReviewRequest {
        id: review_id.clone(),
        status: ReviewStatus::Pending,
        git_ref: effective_ref,
        message: message.clone(),
        requested_by: current_user(),
        requested_at: Utc::now(),
        project,
        branch,
        file_count,
        stats: stats.clone(),
    };

    store.save(&review)?;

    // Output
    println!("success: Review requested: {}", review_id);
    println!();
    println!("Review Request");
    println!("  ID:      {}", review_id);
    println!("  Changes: {} files", file_count);
    println!("  Stats:   {}", stats);
    if let Some(msg) = &message {
        println!("  Message: {}", msg);
    }
    println!();
    println!("Reviewer: run 'review start' to begin review");

    Ok(())
}

/// List review requests
fn cmd_list(store: &ReviewStore, filter: Option<ReviewStatus>, json: bool) -> Result<()> {
    let reviews = store.list(filter)?;

    if json {
        let json_output: Vec<_> = reviews
            .iter()
            .map(|r| {
                serde_json::json!({
                    "id": r.id,
                    "status": r.status.as_str(),
                    "git_ref": r.git_ref,
                    "message": r.message,
                    "requested_by": r.requested_by,
                    "requested_at": r.requested_at.to_rfc3339(),
                    "project": r.project.to_string_lossy(),
                    "branch": r.branch,
                    "file_count": r.file_count,
                    "stats": r.stats,
                })
            })
            .collect();
        println!("{}", serde_json::to_string_pretty(&json_output)?);
        return Ok(());
    }

    if reviews.is_empty() {
        println!("No review requests found.");
        return Ok(());
    }

    println!("Review Requests");
    println!();

    for review in &reviews {
        let status_icon = match review.status {
            ReviewStatus::Pending => "o",   // Yellow circle (pending)
            ReviewStatus::Approved => "+",  // Green check (approved)
            ReviewStatus::Rejected => "x",  // Red X (rejected)
        };

        println!("  {} {}", status_icon, review.id);
        println!(
            "    Status: {} | Files: {} | By: {}",
            review.status.as_str(),
            review.file_count,
            review.requested_by
        );
        if let Some(msg) = &review.message {
            println!("    {}", msg);
        }
        println!();
    }

    Ok(())
}

/// Start reviewing a request
fn cmd_start(store: &ReviewStore, id: Option<String>) -> Result<()> {
    let review = if let Some(review_id) = id {
        store.load(&review_id)?
    } else {
        store
            .most_recent_pending()?
            .ok_or_else(|| anyhow::anyhow!("No pending reviews to start"))?
    };

    println!("Starting Review: {}", review.id);
    if let Some(msg) = &review.message {
        println!("{}", msg);
    }
    println!();

    // Show diff stats
    let diff_stat = GitInfo::show_diff(&review.git_ref, true)?;
    println!("{}", diff_stat);

    // Show full diff
    println!();
    println!("Full diff:");
    println!("---");
    let diff = GitInfo::show_diff(&review.git_ref, false)?;
    println!("{}", diff);
    println!("---");

    println!();
    println!("Review Actions:");
    println!("  review approve -m 'comment'   # Approve");
    println!("  review reject -m 'comment'    # Reject");
    println!("  review comment -m 'comment'   # Comment only");

    Ok(())
}

/// Approve a review
fn cmd_approve(store: &ReviewStore, id: Option<String>, message: Option<String>) -> Result<()> {
    let review = if let Some(review_id) = id {
        store.load(&review_id)?
    } else {
        store
            .most_recent_pending()?
            .ok_or_else(|| anyhow::anyhow!("No pending reviews to approve"))?
    };

    // Update status
    store.update_status(&review.id, ReviewStatus::Approved)?;

    // Save decision
    let decision = ReviewDecision {
        by: current_user(),
        at: Utc::now(),
        comment: message.clone(),
        decision: ReviewStatus::Approved,
    };
    store.save_decision(&review.id, decision)?;

    println!("success: Review approved: {}", review.id);
    if let Some(msg) = &message {
        println!("Comment: {}", msg);
    }

    Ok(())
}

/// Reject a review
fn cmd_reject(store: &ReviewStore, id: Option<String>, message: Option<String>) -> Result<()> {
    let message = message.ok_or_else(|| anyhow::anyhow!("Rejection requires a message (-m)"))?;

    let review = if let Some(review_id) = id {
        store.load(&review_id)?
    } else {
        store
            .most_recent_pending()?
            .ok_or_else(|| anyhow::anyhow!("No pending reviews to reject"))?
    };

    // Update status
    store.update_status(&review.id, ReviewStatus::Rejected)?;

    // Save decision
    let decision = ReviewDecision {
        by: current_user(),
        at: Utc::now(),
        comment: Some(message.clone()),
        decision: ReviewStatus::Rejected,
    };
    store.save_decision(&review.id, decision)?;

    println!("error: Review rejected: {}", review.id);
    println!("Reason: {}", message);

    Ok(())
}

/// Add a comment to a review
fn cmd_comment(store: &ReviewStore, id: Option<String>, message: Option<String>) -> Result<()> {
    let message = message.ok_or_else(|| anyhow::anyhow!("Comment required (-m)"))?;

    let review = if let Some(review_id) = id {
        store.load(&review_id)?
    } else {
        store
            .most_recent_pending()?
            .ok_or_else(|| anyhow::anyhow!("No pending reviews to comment on"))?
    };

    let comment = ReviewComment {
        by: current_user(),
        at: Utc::now(),
        comment: message,
    };

    store.add_comment(&review.id, comment)?;

    println!("success: Comment added to: {}", review.id);

    Ok(())
}

/// Show review details
fn cmd_show(store: &ReviewStore, id: &str) -> Result<()> {
    let review = store.load(id)?;

    // Pretty print the review
    println!("{}", serde_json::to_string_pretty(&review)?);

    // Show decision if exists
    if let Some(decision) = store.get_decision(id)? {
        println!();
        println!("Decision:");
        println!("{}", serde_json::to_string_pretty(&decision)?);
    }

    // Show comments if any
    let comments = store.get_comments(id)?;
    if !comments.is_empty() {
        println!();
        println!("Comments:");
        for comment in &comments {
            println!("  {} ({}): {}", comment.by, comment.at.format("%Y-%m-%d %H:%M"), comment.comment);
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cli_parses() {
        // Just verify the CLI definition is valid
        use clap::CommandFactory;
        Cli::command().debug_assert();
    }
}
