//! undo - File-level undo with timeline
//!
//! "Users should experiment fearlessly. Every change is reversible."
//!
//! Commands:
//! - checkpoint [name]: Create a named restore point
//! - last [n]: Revert last n changes
//! - timeline: Show chronological list of changes
//! - restore <reference>: Restore to a reference point

mod db;

use anyhow::{bail, Context, Result};
use chrono::{Local, Utc};
use clap::{Parser, Subcommand};
use daedalos_core::Paths;
use std::path::PathBuf;

use crate::db::{ChangeType, UndoDatabase, UndoEntry};

#[derive(Parser)]
#[command(name = "undo")]
#[command(about = "File-level undo with timeline - every change is reversible")]
#[command(version)]
#[command(after_help = "\
TRIGGER:
    Use undo checkpoint BEFORE any multi-file change, deletion, or refactor.
    If something breaks, undo restore recovers instantly.

EXAMPLES:
    undo checkpoint \"before refactor\"   Create named restore point
    undo last                            Undo the most recent change
    undo last 3                          Undo the last 3 changes
    undo timeline                        Show chronological list of changes
    undo restore before-refactor         Restore to named checkpoint
    undo restore 14:30:00                Restore to time (today)
    undo restore abc123                  Restore to specific entry ID

WORKFLOW:
    1. undo checkpoint \"before risky work\"
    2. Make your changes
    3. If things go wrong: undo restore \"before risky work\"
    4. If things work: continue (checkpoint remains for safety)")]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// Create a named checkpoint
    Checkpoint {
        /// Name for the checkpoint (auto-generated if not provided)
        name: Option<String>,

        /// Description of the checkpoint
        #[arg(short, long)]
        description: Option<String>,
    },

    /// Revert last n changes
    Last {
        /// Number of changes to undo (default: 1)
        #[arg(default_value = "1")]
        count: u32,

        /// Only show what would be undone, don't actually undo
        #[arg(long)]
        dry_run: bool,

        /// Filter to specific file
        #[arg(long)]
        file: Option<PathBuf>,
    },

    /// Show chronological list of changes
    Timeline {
        /// Number of entries to show
        #[arg(short = 'n', long, default_value = "20")]
        count: u32,

        /// Filter to specific file
        #[arg(long)]
        file: Option<PathBuf>,

        /// Output as JSON
        #[arg(long)]
        json: bool,
    },

    /// Restore to a reference point (checkpoint name, ID, or time)
    Restore {
        /// Reference: checkpoint name, entry ID, or time (HH:MM:SS)
        reference: String,

        /// Only show what would be restored, don't actually restore
        #[arg(long)]
        dry_run: bool,

        /// Filter to specific file
        #[arg(long)]
        file: Option<PathBuf>,
    },

    /// Record a file change (for use by other tools)
    Record {
        /// File path to record
        file: PathBuf,

        /// Change type: edit, create, delete, rename
        #[arg(short = 't', long, default_value = "edit")]
        change_type: String,

        /// Description of the change
        #[arg(short, long, default_value = "File changed")]
        description: String,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    let paths = Paths::new();
    let data_dir = paths.data.join("undo");
    let mut db = UndoDatabase::open(&data_dir)
        .context("Failed to open undo database")?;

    match cli.command {
        Some(Commands::Checkpoint { name, description }) => {
            cmd_checkpoint(&mut db, name, description)
        }
        Some(Commands::Last { count, dry_run, file }) => {
            cmd_last(&mut db, count, dry_run, file)
        }
        Some(Commands::Timeline { count, file, json }) => {
            cmd_timeline(&db, count, file, json)
        }
        Some(Commands::Restore { reference, dry_run, file }) => {
            cmd_restore(&mut db, &reference, dry_run, file)
        }
        Some(Commands::Record { file, change_type, description }) => {
            cmd_record(&mut db, file, &change_type, &description)
        }
        None => {
            // Default: show recent timeline
            cmd_timeline(&db, 10, None, false)
        }
    }
}

/// Create a named checkpoint
fn cmd_checkpoint(db: &mut UndoDatabase, name: Option<String>, description: Option<String>) -> Result<()> {
    let name = name.unwrap_or_else(|| {
        format!("checkpoint-{}", Local::now().format("%Y%m%d-%H%M%S"))
    });

    let desc = description.unwrap_or_default();
    let id = db.create_checkpoint(&name, &desc)?;

    println!("Created checkpoint: {} ({})", name, id);
    Ok(())
}

/// Undo last n changes
fn cmd_last(db: &mut UndoDatabase, count: u32, dry_run: bool, file: Option<PathBuf>) -> Result<()> {
    let file_filter = file.as_ref().map(|p| p.to_string_lossy().to_string());
    let entries = db.get_entries(count, file_filter.as_deref())?;

    if entries.is_empty() {
        println!("Nothing to undo");
        return Ok(());
    }

    // Filter out checkpoints - we only undo actual file changes
    let file_entries: Vec<_> = entries
        .into_iter()
        .filter(|e| e.change_type != ChangeType::Checkpoint && e.backup_hash.is_some())
        .collect();

    if file_entries.is_empty() {
        println!("No file changes to undo");
        return Ok(());
    }

    if dry_run {
        println!("Would undo {} changes:", file_entries.len());
        for entry in &file_entries {
            println!("  {} - {} ({})",
                format_time(&entry.timestamp),
                entry.file_path,
                entry.change_type.as_str()
            );
        }
        return Ok(());
    }

    // Create pre-restore checkpoint (undo the undo safety)
    let checkpoint_name = format!("pre-restore-{}", Local::now().format("%H%M%S"));
    db.create_checkpoint(&checkpoint_name, "Auto-checkpoint before undo")?;

    let mut restored = 0;
    for entry in &file_entries {
        if db.restore_file(entry)? {
            restored += 1;
            println!("Restored: {}", entry.file_path);
        } else {
            eprintln!("Warning: Could not restore {}", entry.file_path);
        }
    }

    println!("\nUndid {} of {} changes", restored, file_entries.len());
    println!("(checkpoint '{}' created - use 'undo restore {}' to redo)",
        checkpoint_name, checkpoint_name);

    Ok(())
}

/// Show timeline of changes
fn cmd_timeline(db: &UndoDatabase, count: u32, file: Option<PathBuf>, json: bool) -> Result<()> {
    let file_filter = file.as_ref().map(|p| p.to_string_lossy().to_string());
    let entries = db.get_entries(count, file_filter.as_deref())?;

    if json {
        let json_entries: Vec<_> = entries.iter().map(|e| {
            serde_json::json!({
                "id": e.id,
                "timestamp": e.timestamp.to_rfc3339(),
                "type": e.change_type.as_str(),
                "file": e.file_path,
                "description": e.description,
                "size": e.file_size,
            })
        }).collect();
        println!("{}", serde_json::to_string_pretty(&json_entries)?);
        return Ok(());
    }

    if entries.is_empty() {
        println!("No entries in timeline");
        return Ok(());
    }

    println!("{:<12} {:<10} {:<8} {}", "TIME", "ID", "TYPE", "FILE");
    println!("{}", "-".repeat(70));

    for entry in &entries {
        let file_display = if entry.file_path.is_empty() {
            entry.description.clone()
        } else {
            // Show just filename for brevity
            std::path::Path::new(&entry.file_path)
                .file_name()
                .map(|f| f.to_string_lossy().to_string())
                .unwrap_or(entry.file_path.clone())
        };

        let type_display = match entry.change_type {
            ChangeType::Edit => "edit",
            ChangeType::Create => "create",
            ChangeType::Delete => "delete",
            ChangeType::Rename => "rename",
            ChangeType::Checkpoint => "checkpoint",
        };

        println!("{:<12} {:<10} {:<8} {}",
            format_time(&entry.timestamp),
            &entry.id[..entry.id.len().min(10)],
            type_display,
            file_display
        );
    }

    Ok(())
}

/// Restore to a reference point
fn cmd_restore(db: &mut UndoDatabase, reference: &str, dry_run: bool, file: Option<PathBuf>) -> Result<()> {
    // Try to find reference as:
    // 1. Checkpoint name
    // 2. Entry ID
    // 3. Time (HH:MM:SS or HH:MM)

    // Check if it's a checkpoint
    if let Some(checkpoint) = db.get_checkpoint(reference)? {
        let entries = db.get_entries_since_checkpoint(&checkpoint.name)?;

        let file_filter = file.as_ref().map(|p| p.to_string_lossy().to_string());
        let entries: Vec<_> = if let Some(ref filter) = file_filter {
            entries.into_iter().filter(|e| e.file_path == *filter).collect()
        } else {
            entries
        };

        if entries.is_empty() {
            println!("No changes since checkpoint '{}'", checkpoint.name);
            return Ok(());
        }

        if dry_run {
            println!("Would restore {} files to checkpoint '{}':", entries.len(), checkpoint.name);
            for entry in &entries {
                println!("  {}", entry.file_path);
            }
            return Ok(());
        }

        // Create pre-restore checkpoint
        let pre_name = format!("pre-restore-{}", Local::now().format("%H%M%S"));
        db.create_checkpoint(&pre_name, &format!("Before restore to {}", reference))?;

        let mut restored = 0;
        for entry in &entries {
            if db.restore_file(entry)? {
                restored += 1;
                println!("Restored: {}", entry.file_path);
            }
        }

        println!("\nRestored {} files to checkpoint '{}'", restored, checkpoint.name);
        return Ok(());
    }

    // Check if it's an entry ID
    if let Some(entry) = db.get_entry(reference)? {
        if entry.backup_hash.is_none() {
            bail!("Entry {} has no backup to restore", reference);
        }

        if dry_run {
            println!("Would restore: {}", entry.file_path);
            return Ok(());
        }

        // Create pre-restore checkpoint
        let pre_name = format!("pre-restore-{}", Local::now().format("%H%M%S"));
        db.create_checkpoint(&pre_name, &format!("Before restore entry {}", reference))?;

        if db.restore_file(&entry)? {
            println!("Restored: {}", entry.file_path);
        } else {
            bail!("Failed to restore file");
        }

        return Ok(());
    }

    // Try parsing as time
    if let Some(time_entries) = try_parse_time_reference(db, reference)? {
        if time_entries.is_empty() {
            println!("No changes found around time {}", reference);
            return Ok(());
        }

        let file_filter = file.as_ref().map(|p| p.to_string_lossy().to_string());
        let entries: Vec<_> = if let Some(ref filter) = file_filter {
            time_entries.into_iter().filter(|e| e.file_path == *filter).collect()
        } else {
            time_entries
        };

        if dry_run {
            println!("Would restore {} files to time {}:", entries.len(), reference);
            for entry in &entries {
                println!("  {}", entry.file_path);
            }
            return Ok(());
        }

        // Create pre-restore checkpoint
        let pre_name = format!("pre-restore-{}", Local::now().format("%H%M%S"));
        db.create_checkpoint(&pre_name, &format!("Before restore to time {}", reference))?;

        let mut restored = 0;
        for entry in &entries {
            if db.restore_file(entry)? {
                restored += 1;
                println!("Restored: {}", entry.file_path);
            }
        }

        println!("\nRestored {} files", restored);
        return Ok(());
    }

    bail!("Could not find reference '{}' (not a checkpoint, entry ID, or valid time)", reference);
}

/// Record a file change
fn cmd_record(db: &mut UndoDatabase, file: PathBuf, change_type: &str, description: &str) -> Result<()> {
    let ct = ChangeType::from_str(change_type)
        .ok_or_else(|| anyhow::anyhow!("Invalid change type: {}", change_type))?;

    let file_path = file.canonicalize().unwrap_or(file);
    let id = db.record_change(&file_path, ct, description)?;

    println!("Recorded change: {} ({})", file_path.display(), id);
    Ok(())
}

/// Format timestamp for display
fn format_time(dt: &chrono::DateTime<Utc>) -> String {
    dt.with_timezone(&Local).format("%H:%M:%S").to_string()
}

/// Try to parse a time reference and find entries around that time
fn try_parse_time_reference(db: &UndoDatabase, reference: &str) -> Result<Option<Vec<UndoEntry>>> {
    // Parse HH:MM:SS or HH:MM
    let parts: Vec<&str> = reference.split(':').collect();
    if parts.len() < 2 || parts.len() > 3 {
        return Ok(None);
    }

    let hour: u32 = parts[0].parse().ok().filter(|&h| h < 24).ok_or_else(|| anyhow::anyhow!("Invalid hour"))?;
    let minute: u32 = parts[1].parse().ok().filter(|&m| m < 60).ok_or_else(|| anyhow::anyhow!("Invalid minute"))?;
    let second: u32 = if parts.len() == 3 {
        parts[2].parse().ok().filter(|&s| s < 60).ok_or_else(|| anyhow::anyhow!("Invalid second"))?
    } else {
        0
    };

    // Build target time (today with given time)
    let today = Local::now().date_naive();
    let target_time = today.and_hms_opt(hour, minute, second)
        .ok_or_else(|| anyhow::anyhow!("Invalid time"))?;
    let target_utc = target_time.and_local_timezone(Local)
        .single()
        .map(|t| t.with_timezone(&Utc))
        .ok_or_else(|| anyhow::anyhow!("Ambiguous time"))?;

    // Get all entries and find ones around this time
    // This is a simple approach - we get entries and find the closest one before target time
    let entries = db.get_entries(1000, None)?;

    let before_entries: Vec<_> = entries
        .into_iter()
        .filter(|e| e.timestamp <= target_utc && e.change_type != ChangeType::Checkpoint)
        .collect();

    Ok(Some(before_entries))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_time() {
        let dt = Utc::now();
        let formatted = format_time(&dt);
        assert!(formatted.contains(':'));
    }
}
