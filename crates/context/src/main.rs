//! Context - Context window management for Claude Code
//!
//! Monitor and manage Claude's context usage to optimize long sessions.
//! Agents lose track of their context window and die mid-task.
//! This tool provides visibility into what's consuming the token budget.

mod estimator;
mod tracker;
mod visualizer;

use anyhow::Result;
use clap::{Parser, Subcommand};

use tracker::ContextTracker;
use visualizer::{format_breakdown, format_status, format_checkpoints, format_files, format_suggestions};

/// Format a number with thousands separators
fn format_number(n: usize) -> String {
    let s = n.to_string();
    let mut result = String::new();
    for (i, c) in s.chars().rev().enumerate() {
        if i > 0 && i % 3 == 0 {
            result.push(',');
        }
        result.push(c);
    }
    result.chars().rev().collect()
}

#[derive(Parser)]
#[command(name = "context")]
#[command(version = "1.0.0")]
#[command(about = "Context window management - know your token budget")]
#[command(after_help = "\
TRIGGER:
    Use context when in a long conversation and worried about limits.
    Visibility into token usage enables intelligent decisions about
    what to summarize, skip, or defer.

EXAMPLES:
    context                    Show current token usage (default)
    context status             Same as above
    context breakdown          Detailed breakdown by category
    context files              Show files in context with token counts
    context compact            Get suggestions for reducing usage
    context checkpoint work    Save current state for reference
    context list-checkpoints   Show saved checkpoints
    context full               Complete context report

PHILOSOPHY:
    Agents lose track of their context window and die mid-task.
    Visibility prevents this. Check early, check often.")]
struct Cli {
    /// Disable colored output
    #[arg(long, global = true)]
    no_color: bool,

    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// Show current context budget (alias: estimate)
    #[command(alias = "estimate")]
    Status {
        /// Output as JSON
        #[arg(long)]
        json: bool,
    },

    /// Show detailed context breakdown
    Breakdown {
        /// Output as JSON
        #[arg(long)]
        json: bool,
    },

    /// Show files currently in context
    Files {
        /// Output as JSON
        #[arg(long)]
        json: bool,

        /// Maximum files to show
        #[arg(short = 'n', long, default_value = "10")]
        limit: usize,
    },

    /// Suggest context compaction strategies
    Compact {
        /// Output as JSON
        #[arg(long)]
        json: bool,
    },

    /// Save context checkpoint for later reference
    Checkpoint {
        /// Name for the checkpoint
        name: String,
    },

    /// List saved checkpoints
    ListCheckpoints {
        /// Output as JSON
        #[arg(long)]
        json: bool,
    },

    /// Show checkpoint contents
    Restore {
        /// Name of checkpoint to restore
        name: String,

        /// Output as JSON
        #[arg(long)]
        json: bool,
    },

    /// Show complete context report
    Full,
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let use_color = !cli.no_color && atty::is(atty::Stream::Stdout);

    match cli.command {
        Some(Commands::Status { json }) => cmd_status(json, use_color),
        Some(Commands::Breakdown { json }) => cmd_breakdown(json, use_color),
        Some(Commands::Files { json, limit }) => cmd_files(json, limit, use_color),
        Some(Commands::Compact { json }) => cmd_compact(json, use_color),
        Some(Commands::Checkpoint { name }) => cmd_checkpoint(&name),
        Some(Commands::ListCheckpoints { json }) => cmd_list_checkpoints(json, use_color),
        Some(Commands::Restore { name, json }) => cmd_restore(&name, json, use_color),
        Some(Commands::Full) => cmd_full(use_color),
        None => cmd_status(false, use_color), // Default to status
    }
}

fn cmd_status(json_output: bool, use_color: bool) -> Result<()> {
    let tracker = ContextTracker::new(None)?;
    let status = tracker.get_status()?;

    if json_output {
        println!("{}", serde_json::to_string_pretty(&status)?);
    } else {
        println!("{}", format_status(&status, use_color));
    }

    Ok(())
}

fn cmd_breakdown(json_output: bool, use_color: bool) -> Result<()> {
    let tracker = ContextTracker::new(None)?;
    let status = tracker.get_status()?;

    if json_output {
        println!("{}", serde_json::to_string_pretty(&status.breakdown)?);
    } else {
        println!("{}", format_breakdown(&status, use_color));
    }

    Ok(())
}

fn cmd_files(json_output: bool, limit: usize, use_color: bool) -> Result<()> {
    let tracker = ContextTracker::new(None)?;
    let files = tracker.get_files_in_context()?;

    if json_output {
        let limited: Vec<_> = files.into_iter().take(limit).collect();
        println!("{}", serde_json::to_string_pretty(&limited)?);
    } else {
        println!("{}", format_files(&files, limit, use_color));
    }

    Ok(())
}

fn cmd_compact(json_output: bool, use_color: bool) -> Result<()> {
    let tracker = ContextTracker::new(None)?;
    let suggestions = tracker.get_compaction_suggestions()?;

    if json_output {
        println!("{}", serde_json::to_string_pretty(&suggestions)?);
    } else {
        println!("{}", format_suggestions(&suggestions, use_color));
    }

    Ok(())
}

fn cmd_checkpoint(name: &str) -> Result<()> {
    let tracker = ContextTracker::new(None)?;
    let data = tracker.checkpoint(name)?;

    println!("Checkpoint '{}' created", name);
    println!("  Tokens: {}", format_number(data.status.used));
    println!("  Files: {}", data.files.len());

    Ok(())
}

fn cmd_list_checkpoints(json_output: bool, use_color: bool) -> Result<()> {
    let tracker = ContextTracker::new(None)?;
    let checkpoints = tracker.list_checkpoints()?;

    if json_output {
        println!("{}", serde_json::to_string_pretty(&checkpoints)?);
    } else {
        println!("{}", format_checkpoints(&checkpoints, use_color));
    }

    Ok(())
}

fn cmd_restore(name: &str, json_output: bool, _use_color: bool) -> Result<()> {
    let tracker = ContextTracker::new(None)?;

    match tracker.restore_checkpoint(name)? {
        Some(data) => {
            if json_output {
                println!("{}", serde_json::to_string_pretty(&data)?);
            } else {
                println!("Checkpoint: {}", data.name);
                println!("Created: {}", data.created);
                if let Some(ref project) = data.project {
                    println!("Project: {}", project);
                }
                println!();
                println!("Status at checkpoint:");
                println!("  Used: {} tokens", format_number(data.status.used));
                println!("  Percentage: {:.1}%", data.status.percentage);
                println!();
                if !data.files.is_empty() {
                    println!("Files in context:");
                    for f in data.files.iter().take(10) {
                        println!("  {} ({} tokens)", f.path, format_number(f.tokens));
                    }
                }
            }
        }
        None => {
            eprintln!("Checkpoint '{}' not found", name);
            std::process::exit(2);
        }
    }

    Ok(())
}

fn cmd_full(use_color: bool) -> Result<()> {
    let tracker = ContextTracker::new(None)?;
    let status = tracker.get_status()?;
    let files = tracker.get_files_in_context()?;
    let suggestions = tracker.get_compaction_suggestions()?;

    println!("{}", format_status(&status, use_color));
    println!();
    println!("{}", format_breakdown(&status, use_color));
    println!();
    println!("{}", format_files(&files, 5, use_color));

    if !suggestions.is_empty() {
        println!();
        println!("{}", format_suggestions(&suggestions, use_color));
    }

    Ok(())
}

// Add atty for TTY detection
mod atty {
    pub enum Stream {
        Stdout,
    }

    pub fn is(_stream: Stream) -> bool {
        // Simple check using libc
        #[cfg(unix)]
        unsafe {
            libc::isatty(libc::STDOUT_FILENO) != 0
        }
        #[cfg(not(unix))]
        true
    }
}

#[cfg(unix)]
extern crate libc;
