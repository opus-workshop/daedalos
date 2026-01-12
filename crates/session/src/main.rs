//! session - Save/restore terminal sessions for Daedalos
//!
//! "Capture and restore your complete terminal state."
//!
//! Commands:
//! - save [NAME]: Save current session state
//! - restore [NAME]: Restore a saved session
//! - list: List saved sessions
//! - show [NAME]: Show session details
//! - delete [NAME]: Delete a saved session
//! - export [NAME]: Export session to shareable format
//! - import <FILE>: Import session from file
//! - auto: Toggle auto-save on exit

use anyhow::{bail, Result};
use chrono::{DateTime, Utc};
use clap::{Parser, Subcommand};
use session::{capture_session, generate_restore_script, time_ago, SessionStore};
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "session")]
#[command(about = "Save/restore terminal sessions - capture and restore your complete terminal state")]
#[command(version)]
#[command(long_about = r#"
session - Save/restore terminal sessions for Daedalos

Capture and restore your complete terminal state.

WHAT'S SAVED:
    - Current working directory
    - Environment variables (filtered)
    - Shell history (recent)
    - Git branch and status
    - Tmux/screen layout (if applicable)
    - Daedalos project context

INTEGRATION:
    Sessions integrate with:
    - env: Restores project environment
    - agent: Restores agent context
    - journal: Logs session switches
"#)]
#[command(after_help = r#"WHEN TO USE:
    - End of work session -> resume later exactly where you were
    - Before risky experiments -> save state to restore if needed
    - Sharing work context -> export session for teammates

EXAMPLES:
    session save                    # Auto-named timestamp session
    session save "debugging-auth"   # Named session
    session restore debugging-auth  # Restore session
    session list                    # Show all saved sessions
    session show debugging-auth     # View session details
    session export debugging-auth   # Create shareable archive
"#)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// Save current terminal state (cwd, env, history, git, tmux)
    Save {
        /// Name for the session (timestamp if not specified)
        name: Option<String>,
    },

    /// Restore a saved session (shows commands to run)
    Restore {
        /// Name of the session to restore (lists recent if not provided)
        name: Option<String>,

        /// Show what would be restored without doing it
        #[arg(short = 'n', long)]
        dry_run: bool,
    },

    /// List all saved sessions with timestamps
    #[command(visible_aliases = &["ls", "l"])]
    List {
        /// Output as JSON for scripting
        #[arg(long)]
        json: bool,
    },

    /// Show detailed session contents (commands, env, git state)
    Show {
        /// Name of the session to show
        name: String,

        /// Output as JSON for scripting
        #[arg(long)]
        json: bool,
    },

    /// Delete a saved session permanently
    #[command(visible_aliases = &["rm", "remove"])]
    Delete {
        /// Name of the session to delete
        name: String,
    },

    /// Export session to shareable .tar.gz archive
    Export {
        /// Name of the session to export (most recent if not specified)
        name: Option<String>,

        /// Output file path (defaults to <name>.session.tar.gz)
        #[arg(short, long)]
        output: Option<PathBuf>,
    },

    /// Import session from exported archive
    Import {
        /// Session archive file to import
        file: PathBuf,
    },

    /// Toggle auto-save on shell exit
    Auto,
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let store = SessionStore::from_paths()?;

    match cli.command {
        Some(Commands::Save { name }) => cmd_save(&store, name),
        Some(Commands::Restore { name, dry_run }) => cmd_restore(&store, name, dry_run),
        Some(Commands::List { json }) => cmd_list(&store, json),
        Some(Commands::Show { name, json }) => cmd_show(&store, &name, json),
        Some(Commands::Delete { name }) => cmd_delete(&store, &name),
        Some(Commands::Export { name, output }) => cmd_export(&store, name, output),
        Some(Commands::Import { file }) => cmd_import(&store, &file),
        Some(Commands::Auto) => cmd_auto(),
        None => cmd_list(&store, false),
    }
}

/// Generate a session name from timestamp
fn generate_name() -> String {
    Utc::now().format("%Y%m%d_%H%M%S").to_string()
}

/// Save current session
fn cmd_save(store: &SessionStore, name: Option<String>) -> Result<()> {
    let name = name.unwrap_or_else(generate_name);

    if store.exists(&name) {
        println!("info: Overwriting existing session: {}", name);
    }

    let session = capture_session(&name)?;
    store.save(&name, &session)?;

    println!("success: Session saved: {}", name);
    println!("dim: Restore with: session restore {}", name);

    Ok(())
}

/// Restore a saved session
fn cmd_restore(store: &SessionStore, name: Option<String>, dry_run: bool) -> Result<()> {
    let name = match name {
        Some(n) => n,
        None => {
            // Show recent sessions if no name provided
            let sessions = store.list()?;
            if sessions.is_empty() {
                bail!("No saved sessions");
            }

            println!("bold: Recent sessions:");
            for session in sessions.iter().take(5) {
                let cwd = session.cwd.to_string_lossy();
                println!("  {} - {}", session.name, cwd);
            }
            println!();
            bail!("Session name required");
        }
    };

    if !store.exists(&name) {
        bail!("Session not found: {}\nUse 'session list' to see available sessions", name);
    }

    let session = store.load(&name)?;

    if dry_run {
        println!("bold: Would restore:");
        println!("  cyan: Directory: {}", session.info.cwd.display());

        if let Some(ref git) = session.git {
            println!("  cyan: Git branch: {}", git.branch);
        }

        if !session.daedalos_env.is_empty() {
            println!("  cyan: Daedalos env:");
            for (key, value) in &session.daedalos_env {
                println!("    {}={}", key, value);
            }
        }
        return Ok(());
    }

    // Generate and display restore script
    let script = generate_restore_script(&session);

    println!("bold: To restore this session, run:");
    println!();
    println!("  {}", script.trim().replace('\n', " && "));
    println!();

    // Show git info if available
    if let Some(ref git) = session.git {
        println!("dim: Git branch was: {}", git.branch);
        println!("dim: To switch: git checkout {}", git.branch);
    }

    Ok(())
}

/// List saved sessions
fn cmd_list(store: &SessionStore, json: bool) -> Result<()> {
    let sessions = store.list()?;

    if json {
        let json_output: Vec<_> = sessions
            .iter()
            .map(|s| {
                serde_json::json!({
                    "name": s.name,
                    "cwd": s.cwd.to_string_lossy(),
                    "created": s.created.timestamp(),
                    "git_branch": s.git_branch,
                })
            })
            .collect();
        println!("{}", serde_json::to_string_pretty(&json_output)?);
        return Ok(());
    }

    if sessions.is_empty() {
        println!("No saved sessions");
        return Ok(());
    }

    println!("bold: Saved Sessions");
    println!();

    for session in &sessions {
        // Truncate path if too long
        let cwd_str = session.cwd.to_string_lossy();
        let cwd_display = if cwd_str.len() > 50 {
            format!("...{}", &cwd_str[cwd_str.len() - 47..])
        } else {
            cwd_str.to_string()
        };

        let created_display = time_ago(&session.created);

        println!("  cyan: {}", session.name);
        println!("    dim: Path:  {}", cwd_display);
        println!("    dim: Saved: {}", created_display);

        if let Some(ref branch) = session.git_branch {
            println!("    dim: Branch: {}", branch);
        }
        println!();
    }

    Ok(())
}

/// Show session details
fn cmd_show(store: &SessionStore, name: &str, json: bool) -> Result<()> {
    let session = store.load(name)?;

    if json {
        println!("{}", serde_json::to_string_pretty(&session)?);
        return Ok(());
    }

    println!("bold: Session: {}", name);
    println!();

    // Basic info
    println!("cyan: Info:");
    println!("  Created: {}", DateTime::from_timestamp(session.info.created, 0)
        .map(|dt| dt.format("%Y-%m-%d %H:%M:%S").to_string())
        .unwrap_or_else(|| "unknown".to_string()));
    println!("  Directory: {}", session.info.cwd.display());
    println!("  User: {}", session.info.user);
    println!("  Hostname: {}", session.info.hostname);
    println!("  Shell: {}", session.info.shell);
    println!();

    // Git state
    if let Some(ref git) = session.git {
        println!("cyan: Git:");
        println!("  Root: {}", git.root.display());
        println!("  Branch: {}", git.branch);
        println!("  Commit: {}", &git.commit[..git.commit.len().min(12)]);
        println!("  Modified: {} files", git.modified_count);
        println!();
    }

    // Daedalos environment
    if !session.daedalos_env.is_empty() {
        println!("cyan: Daedalos Environment:");
        for (key, value) in &session.daedalos_env {
            println!("  {}={}", key, value);
        }
        println!();
    }

    // Tmux state
    if !session.tmux_windows.is_empty() {
        println!("cyan: Tmux Windows:");
        for window in &session.tmux_windows {
            println!("  {}: {} ({})", window.index, window.name, window.path.display());
        }
        println!();
    }

    // Recent history
    if !session.history.is_empty() {
        println!("cyan: Recent Commands (last 10):");
        for cmd in session.history.iter().rev().take(10).rev() {
            // Skip zsh history metadata
            let cmd = if cmd.starts_with(':') {
                cmd.split(';').nth(1).unwrap_or(cmd)
            } else {
                cmd
            };
            println!("  {}", cmd);
        }
        println!();
    }

    Ok(())
}

/// Delete a session
fn cmd_delete(store: &SessionStore, name: &str) -> Result<()> {
    if !store.exists(name) {
        bail!("Session not found: {}", name);
    }

    store.delete(name)?;
    println!("success: Session deleted: {}", name);

    Ok(())
}

/// Export a session
fn cmd_export(store: &SessionStore, name: Option<String>, output: Option<PathBuf>) -> Result<()> {
    let name = match name {
        Some(n) => n,
        None => {
            // Use most recent session
            let sessions = store.list()?;
            if sessions.is_empty() {
                bail!("No sessions to export");
            }
            sessions[0].name.clone()
        }
    };

    if !store.exists(&name) {
        bail!("Session not found: {}", name);
    }

    let output = output.unwrap_or_else(|| PathBuf::from(format!("{}.session.tar.gz", name)));

    store.export(&name, &output)?;
    println!("success: Exported to: {}", output.display());

    Ok(())
}

/// Import a session
fn cmd_import(store: &SessionStore, file: &PathBuf) -> Result<()> {
    let name = store.import(file)?;
    println!("success: Session imported: {}", name);

    Ok(())
}

/// Toggle auto-save
fn cmd_auto() -> Result<()> {
    let paths = daedalos_core::Paths::new();
    let auto_file = paths.data.join("session").join("auto_save");

    if auto_file.exists() {
        std::fs::remove_file(&auto_file)?;
        println!("info: Auto-save disabled");
        println!("Sessions will not be saved automatically on exit");
    } else {
        std::fs::create_dir_all(auto_file.parent().unwrap())?;
        std::fs::write(&auto_file, "")?;
        println!("success: Auto-save enabled");
        println!("Add this to your shell config to enable:");
        println!();
        println!("  trap 'session save \"auto_$(date +%Y%m%d_%H%M%S)\"' EXIT");
    }

    Ok(())
}
