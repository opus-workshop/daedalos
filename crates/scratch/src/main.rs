//! scratch - Project-scoped ephemeral experiment environments
//!
//! "Fear of breaking things prevents bold experiments."
//!
//! Scratch environments are disposable project copies where anything goes.
//! Create, experiment, promote if it works, abandon if it doesn't.
//!
//! Commands:
//! - new <name>: Create a new scratch environment
//! - list: List all scratch environments
//! - destroy <name>: Delete a scratch environment

mod store;

use anyhow::{bail, Context, Result};
use chrono::{DateTime, Utc};
use clap::{Parser, Subcommand};
use daedalos_core::Paths;
use std::path::PathBuf;

use crate::store::{ScratchMode, ScratchStore};

#[derive(Parser)]
#[command(name = "scratch")]
#[command(about = "Project-scoped ephemeral experiment environments - experiment fearlessly")]
#[command(version)]
#[command(after_help = "\
TRIGGER:
    Use scratch when experimenting with something risky, or when you want
    to try an approach without affecting the real codebase.

EXAMPLES:
    scratch new experiment       Create scratch from current directory
    scratch new test --from .    Explicit source directory
    scratch list                 Show all scratch environments
    scratch enter experiment     cd into the scratch copy
    scratch diff experiment      See changes made in scratch
    scratch promote experiment   Copy changes back to original
    scratch destroy experiment   Delete scratch (with --force)

MODES:
    git   - Uses git worktree (if in a git repo)
    copy  - Full directory copy
    btrfs - Btrfs snapshot (Linux only, fastest)

PHILOSOPHY:
    Fear of breaking things prevents bold experiments.
    Scratches let you try anything. Promote what works, destroy what doesn't.")]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// Create a new scratch environment
    New {
        /// Name for the scratch environment
        name: String,

        /// Source directory to copy (default: current directory)
        #[arg(short, long)]
        from: Option<PathBuf>,

        /// Mode: git, copy, or btrfs (auto-detected if not specified)
        #[arg(short, long)]
        mode: Option<String>,
    },

    /// List all scratch environments
    List {
        /// Output as JSON
        #[arg(long)]
        json: bool,
    },

    /// Delete a scratch environment
    Destroy {
        /// Name of the scratch environment to delete
        name: String,

        /// Don't ask for confirmation
        #[arg(short, long)]
        force: bool,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    let paths = Paths::new();
    let scratch_dir = paths.data.join("scratch");
    let store = ScratchStore::new(&scratch_dir)?;

    match cli.command {
        Some(Commands::New { name, from, mode }) => cmd_new(&store, &name, from, mode),
        Some(Commands::List { json }) => cmd_list(&store, json),
        Some(Commands::Destroy { name, force }) => cmd_destroy(&store, &name, force),
        None => cmd_list(&store, false),
    }
}

/// Create a new scratch environment
fn cmd_new(store: &ScratchStore, name: &str, from: Option<PathBuf>, mode: Option<String>) -> Result<()> {
    // Validate name
    if name.is_empty() {
        bail!("Scratch name cannot be empty");
    }

    if name.contains('/') || name.contains('\\') {
        bail!("Scratch name cannot contain path separators");
    }

    // Check if already exists
    if store.exists(name) {
        bail!("Scratch environment '{}' already exists", name);
    }

    // Resolve source directory
    let source = if let Some(ref from_path) = from {
        from_path
            .canonicalize()
            .with_context(|| format!("Source directory not found: {}", from_path.display()))?
    } else {
        std::env::current_dir().context("Could not determine current directory")?
    };

    if !source.is_dir() {
        bail!("Source is not a directory: {}", source.display());
    }

    // Detect or parse mode
    let scratch_mode = if let Some(mode_str) = mode {
        ScratchMode::from_str(&mode_str)
            .ok_or_else(|| anyhow::anyhow!("Invalid mode '{}'. Valid modes: git, copy, btrfs", mode_str))?
    } else {
        store.detect_mode(&source)
    };

    println!("info: Creating scratch '{}' from {}", name, source.display());
    println!("info: Using mode: {}", scratch_mode.as_str());

    // Create the scratch environment
    let scratch_path = store.create(name, &source, scratch_mode)?;

    println!("success: Created scratch environment: {}", name);
    println!();
    println!("Enter with:   scratch enter {}", name);
    println!("Path:         {}", scratch_path.display());
    println!("See changes:  scratch diff {}", name);
    println!("Keep changes: scratch promote {}", name);
    println!("Discard:      scratch destroy {}", name);

    Ok(())
}

/// List all scratch environments
fn cmd_list(store: &ScratchStore, json: bool) -> Result<()> {
    let scratches = store.list()?;

    if json {
        let json_output: Vec<_> = scratches
            .iter()
            .map(|s| {
                serde_json::json!({
                    "name": s.name,
                    "original": s.original.to_string_lossy(),
                    "path": s.path.to_string_lossy(),
                    "mode": s.mode.as_str(),
                    "created": s.created.to_rfc3339(),
                    "expires": s.expires.map(|e| e.to_rfc3339()),
                })
            })
            .collect();
        println!("{}", serde_json::to_string_pretty(&json_output)?);
        return Ok(());
    }

    if scratches.is_empty() {
        println!("No scratch environments found. Create one with: scratch new <name>");
        return Ok(());
    }

    // Table header
    println!(
        "{:<20} {:<30} {:<12} {:<10}",
        "NAME", "ORIGINAL", "CREATED", "MODE"
    );
    println!("{}", "-".repeat(75));

    for scratch in &scratches {
        // Truncate original path if too long
        let original_str = scratch.original.to_string_lossy();
        let original_display = if original_str.len() > 28 {
            format!("...{}", &original_str[original_str.len() - 25..])
        } else {
            original_str.to_string()
        };

        let created_display = time_ago(&scratch.created);

        // Check if expired
        let expired = scratch.expires.map(|e| Utc::now() > e).unwrap_or(false);
        let suffix = if expired { " (expired)" } else { "" };

        println!(
            "{:<20} {:<30} {:<12} {:<10}{}",
            scratch.name,
            original_display,
            created_display,
            scratch.mode.as_str(),
            suffix
        );
    }

    Ok(())
}

/// Delete a scratch environment
fn cmd_destroy(store: &ScratchStore, name: &str, force: bool) -> Result<()> {
    if !store.exists(name) {
        bail!("Scratch environment not found: {}", name);
    }

    if !force {
        // In a non-interactive context, just warn and proceed
        // Real CLI would prompt here, but for now we require --force
        println!("warning: Use --force to confirm deletion of scratch '{}'", name);
        println!("         This cannot be undone.");
        return Ok(());
    }

    println!("info: Destroying scratch '{}'", name);

    store.destroy(name)?;

    println!("success: Destroyed scratch environment: {}", name);

    Ok(())
}

/// Format a timestamp as relative time
fn time_ago(dt: &DateTime<Utc>) -> String {
    let now = Utc::now();
    let duration = now.signed_duration_since(*dt);

    let seconds = duration.num_seconds();
    if seconds < 60 {
        return "just now".to_string();
    }

    let minutes = duration.num_minutes();
    if minutes < 60 {
        return format!("{} min ago", minutes);
    }

    let hours = duration.num_hours();
    if hours < 24 {
        return format!("{} hours ago", hours);
    }

    let days = duration.num_days();
    format!("{} days ago", days)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_time_ago() {
        let now = Utc::now();
        assert_eq!(time_ago(&now), "just now");

        let one_hour_ago = now - chrono::Duration::hours(1);
        assert_eq!(time_ago(&one_hour_ago), "1 hours ago");

        let two_days_ago = now - chrono::Duration::days(2);
        assert_eq!(time_ago(&two_days_ago), "2 days ago");
    }
}
