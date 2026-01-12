//! focus - Pomodoro timer and distraction blocking for deep work
//!
//! "Protect the sacred space of deep work."
//!
//! Usage:
//!   focus start [MINS]          Start a focus session (default: 25 mins)
//!   focus start --deep          Start a 90-minute deep work session
//!   focus stop                  End current focus session
//!   focus status                Show focus session status
//!   focus break [MINS]          Take a break (default: 5 mins)
//!   focus stats [DAYS]          Show focus statistics
//!   focus block                 Enable distraction blocking indicator
//!   focus unblock               Disable distraction blocking indicator

use anyhow::{bail, Result};
use chrono::{DateTime, Local};
use clap::{Parser, Subcommand};
use daedalos_core::Paths;

use focus::session::{CompletedSession, FocusSession, SessionType};
use focus::stats::FocusStats;
use focus::store::FocusStore;

/// Focus - Pomodoro timer and distraction blocking for deep work
#[derive(Parser)]
#[command(name = "focus")]
#[command(about = "Pomodoro timer and distraction blocking for deep work sessions")]
#[command(version)]
#[command(after_help = r#"WHEN TO USE:
    Before starting focused work. Tracks time spent and completion rate.
    Integrates with 'metrics' for productivity statistics.

PRESETS:
    --pomodoro    25 min focus, 5 min break (default)
    --deep        90 min focus, 20 min break
    --quick       15 min focus, 3 min break

EXAMPLES:
    focus start                 # Start 25-minute pomodoro
    focus start --deep          # Start 90-minute deep work session
    focus start 45 --task "API" # Custom duration with task name
    focus status                # Check remaining time
    focus stop                  # End session early
    focus break                 # Take a 5-minute break
    focus stats 30              # Show 30-day statistics

INTEGRATION:
    Focus sessions are tracked and visible in 'metrics focus'.
    Use --block to enable distraction blocking indicator.

ALIASES:
    focus s     # start
    focus st    # status
    focus b     # break
"#)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// Start a focus session
    #[command(alias = "s")]
    Start {
        /// Duration in minutes (default: 25, or preset default)
        #[arg(value_name = "MINS")]
        duration: Option<u32>,

        /// Use pomodoro preset (25 min focus, 5 min break)
        #[arg(long)]
        pomodoro: bool,

        /// Use deep work preset (90 min focus, 20 min break)
        #[arg(long)]
        deep: bool,

        /// Use quick preset (15 min focus, 3 min break)
        #[arg(long)]
        quick: bool,

        /// What you're working on
        #[arg(long)]
        task: Option<String>,

        /// Enable distraction blocking during session
        #[arg(long)]
        block: bool,
    },

    /// Stop the current focus session
    #[command(alias = "end")]
    Stop,

    /// Show current focus session status
    #[command(alias = "st")]
    Status,

    /// Take a break
    #[command(alias = "b")]
    Break {
        /// Break duration in minutes (default: 5)
        #[arg(default_value = "5")]
        duration: u32,
    },

    /// Show focus statistics
    #[command(alias = "statistics")]
    Stats {
        /// Number of days to show (default: 7)
        #[arg(default_value = "7")]
        days: u32,
    },

    /// Enable distraction blocking indicator
    Block {
        /// Don't print output
        #[arg(long)]
        quiet: bool,
    },

    /// Disable distraction blocking indicator
    Unblock {
        /// Don't print output
        #[arg(long)]
        quiet: bool,
    },
}

// ANSI color codes
const GREEN: &str = "\x1b[0;32m";
const CYAN: &str = "\x1b[0;36m";
const MAGENTA: &str = "\x1b[0;35m";
const BOLD: &str = "\x1b[1m";
const NC: &str = "\x1b[0m";

/// Check if stdout is a TTY and colors should be used
fn use_colors() -> bool {
    std::io::IsTerminal::is_terminal(&std::io::stdout())
}

/// Conditionally apply color
fn color(code: &str, text: &str) -> String {
    if use_colors() {
        format!("{}{}{}", code, text, NC)
    } else {
        text.to_string()
    }
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    let paths = Paths::new();
    let focus_dir = paths.data.join("focus");
    let store = FocusStore::new(&focus_dir)?;

    match cli.command {
        Some(Commands::Start {
            duration,
            pomodoro,
            deep,
            quick,
            task,
            block,
        }) => cmd_start(&store, duration, pomodoro, deep, quick, task, block),
        Some(Commands::Stop) => cmd_stop(&store),
        Some(Commands::Status) => cmd_status(&store),
        Some(Commands::Break { duration }) => cmd_break(duration),
        Some(Commands::Stats { days }) => cmd_stats(&store, days),
        Some(Commands::Block { quiet }) => cmd_block(&store, quiet),
        Some(Commands::Unblock { quiet }) => cmd_unblock(&store, quiet),
        None => cmd_status(&store),
    }
}

/// Start a new focus session
fn cmd_start(
    store: &FocusStore,
    duration: Option<u32>,
    pomodoro: bool,
    deep: bool,
    quick: bool,
    task: Option<String>,
    block: bool,
) -> Result<()> {
    // Check for existing session
    if let Some(_session) = store.get_current_session()? {
        bail!("Focus session already active. Use 'focus stop' to end it first.");
    }

    // Determine session type and duration
    let session_type = if deep {
        SessionType::Deep
    } else if quick {
        SessionType::Quick
    } else if pomodoro || duration.is_none() {
        SessionType::Pomodoro
    } else {
        SessionType::Custom
    };

    let final_duration = duration.unwrap_or_else(|| session_type.default_duration());

    // Create and save session
    let session = FocusSession::new(final_duration, task.clone(), session_type, block);
    store.save_current_session(&session)?;

    // Enable blocking if requested
    if block {
        store.enable_blocking()?;
    }

    // Calculate end time
    let end_time: DateTime<Local> = session.end_time().into();

    println!("{} Focus session started", color(GREEN, "[ok]"));
    println!();
    println!("{}", color(&format!("{}{}", BOLD, MAGENTA), "FOCUS MODE"));
    println!();
    println!("  {}  {} minutes", color(CYAN, "Duration:"), final_duration);
    println!("  {}      {}", color(CYAN, "Type:"), session_type.as_str());
    if let Some(ref t) = task {
        println!("  {}      {}", color(CYAN, "Task:"), t);
    }
    println!("  {}   {}", color(CYAN, "Ends at:"), end_time.format("%H:%M"));
    println!();
    println!("Run 'focus status' to check progress");
    println!("Run 'focus stop' to end early");

    Ok(())
}

/// Stop the current focus session
fn cmd_stop(store: &FocusStore) -> Result<()> {
    let session = match store.get_current_session()? {
        Some(s) => s,
        None => {
            println!("{} No active focus session", color(CYAN, "[info]"));
            return Ok(());
        }
    };

    let elapsed = session.elapsed_minutes();

    // Record the partial session
    let completed = CompletedSession::from_session(&session, false);
    store.record_session(&completed)?;
    store.clear_current_session()?;

    // Disable blocking if it was enabled
    if session.blocking {
        store.disable_blocking()?;
    }

    println!("{} Focus session ended", color(GREEN, "[ok]"));
    println!("Completed: {} of {} minutes", elapsed, session.duration);

    Ok(())
}

/// Show current session status
fn cmd_status(store: &FocusStore) -> Result<()> {
    let session = match store.get_current_session()? {
        Some(s) => s,
        None => {
            println!("No active focus session");
            println!();
            println!("Start one with: focus start");
            return Ok(());
        }
    };

    let elapsed = session.elapsed_minutes();
    let remaining = session.remaining_minutes();
    let progress = session.progress_percent();

    // Build progress bar
    let bar_width: usize = 30;
    let filled = (bar_width * (progress as usize)) / 100;
    let empty = bar_width - filled;
    let bar: String = format!(
        "{}{}",
        "\u{2588}".repeat(filled),  // filled blocks
        "\u{2591}".repeat(empty)     // empty blocks
    );

    println!("{}", color(&format!("{}{}", BOLD, MAGENTA), "FOCUS MODE ACTIVE"));
    println!();
    println!("  {} [{}] {}%", color(CYAN, "Progress:"), bar, progress);
    println!("  {}  {} minutes", color(CYAN, "Elapsed:"), elapsed);
    println!("  {} {} minutes", color(CYAN, "Remaining:"), remaining);
    if let Some(ref task) = session.task {
        println!("  {}      {}", color(CYAN, "Task:"), task);
    }
    println!();

    if remaining == 0 {
        println!("{}", color(GREEN, "Session complete! Take a break."));
    }

    Ok(())
}

/// Start a break
fn cmd_break(duration: u32) -> Result<()> {
    println!("{}", color(&format!("{}{}", BOLD, GREEN), "BREAK TIME"));
    println!();
    println!("Take a {} minute break!", duration);
    println!();
    println!("Suggestions:");
    println!("  - Stretch or walk around");
    println!("  - Get water or a snack");
    println!("  - Look away from screen");
    println!("  - Take deep breaths");
    println!();
    println!("Break ends in {} minutes", duration);

    Ok(())
}

/// Show focus statistics
fn cmd_stats(store: &FocusStore, days: u32) -> Result<()> {
    let sessions = store.get_sessions_for_days(days)?;
    let stats = FocusStats::from_sessions(&sessions);

    let (hours, mins) = stats.total_time();

    println!("{}Focus Statistics (Last {} days){}", BOLD, days, NC);
    println!();
    println!("  {}    {}", color(CYAN, "Total Sessions:"), stats.total_sessions);
    println!(
        "  {}         {} ({}%)",
        color(CYAN, "Completed:"),
        stats.completed_sessions,
        stats.completion_rate
    );
    println!(
        "  {}  {}h {}m",
        color(CYAN, "Total Focus Time:"),
        hours,
        mins
    );

    if stats.total_sessions > 0 {
        println!();
        println!(
            "  {}   {} minutes",
            color(CYAN, "Average Session:"),
            stats.average_duration
        );
    }

    Ok(())
}

/// Enable distraction blocking indicator
fn cmd_block(store: &FocusStore, quiet: bool) -> Result<()> {
    store.enable_blocking()?;

    if !quiet {
        println!("{} Distraction blocking enabled", color(GREEN, "[ok]"));
        println!();
        println!("Note: Full blocking requires additional setup:");
        println!("  macOS: Use Screen Time or edit /etc/hosts");
        println!("  Linux: Use /etc/hosts or firewall rules");
        println!();
        println!("The 'focus' tool tracks that blocking should be active.");
    }

    Ok(())
}

/// Disable distraction blocking indicator
fn cmd_unblock(store: &FocusStore, quiet: bool) -> Result<()> {
    store.disable_blocking()?;

    if !quiet {
        println!("{} Distraction blocking disabled", color(GREEN, "[ok]"));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_color_with_tty() {
        // Just verify the color function doesn't panic
        let result = color(RED, "test");
        assert!(result.contains("test"));
    }
}
