//! journal - Narrative reconstruction for Daedalos
//!
//! "What happened?" is the most important debugging question.
//!
//! Journal aggregates events from ALL Daedalos tools and synthesizes them
//! into human-readable narratives. It's the historian of your AI activity.
//!
//! Commands:
//! - what: What happened? (default command)
//! - events: List raw events
//! - summary: Show event summary
//! - log: Log a custom event

mod collector;
mod db;
mod narrative;

use anyhow::Result;
use chrono::Utc;
use clap::{Parser, Subcommand};

use crate::collector::{log_event_to_file, EventCollector};
use crate::narrative::{build_summary, format_summary, format_time, what_happened};

#[derive(Parser)]
#[command(name = "journal")]
#[command(about = "Activity log aggregating events from all Daedalos tools")]
#[command(version)]
#[command(after_help = r#"WHEN TO USE:
    Use journal to understand what happened during an AI session.
    Essential for debugging, auditing, and reviewing AI activity.

TRIGGER:
    - After something unexpected happened
    - When debugging failures or weird behavior
    - To audit what an AI agent did while you were away
    - Before committing to verify recent changes
    - When handing off work to another person/agent

EXAMPLES:
    journal                       # What happened in the last hour?
    journal what -H 8             # What happened in the last 8 hours?
    journal what --days 1         # What happened today?
    journal what -s gates         # Focus on permission checks
    journal events --source loop  # Raw loop iteration events
    journal summary               # Quick summary stats
    journal log "Started deploy"  # Log a custom event

SOURCES:
    gates        Gate checks and approvals
    loop         Iteration loop events
    agent        Agent lifecycle events
    undo         File changes and checkpoints
    mcp-hub      MCP server events
    user         Custom user-logged events

NARRATIVE OUTPUT:
    The default 'what' command produces human-readable narratives
    like "In the last hour: 3 loop iterations, 12 file changes,
    2 denied gate checks." Use --json for machine-readable output."#)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// Generate human-readable narrative of what happened (default)
    #[command(after_help = "Example: journal what -H 4 -s loop --verbose")]
    What {
        /// Hours to look back
        #[arg(short = 'H', long, default_value = "1")]
        hours: f64,

        /// Days to look back (overrides hours)
        #[arg(long)]
        days: Option<f64>,

        /// Filter by source: gates, loop, agent, undo, mcp-hub, user
        #[arg(short, long)]
        source: Option<String>,

        /// Include more context in narrative
        #[arg(short, long)]
        verbose: bool,

        /// Output as JSON for scripting
        #[arg(long)]
        json: bool,
    },

    /// List raw events in table format
    #[command(after_help = "Example: journal events --source loop --type iteration_complete")]
    Events {
        /// Hours to look back
        #[arg(short = 'H', long, default_value = "24")]
        hours: f64,

        /// Days to look back (overrides hours)
        #[arg(long)]
        days: Option<f64>,

        /// Filter by source: gates, loop, agent, undo, mcp-hub, user
        #[arg(short, long)]
        source: Option<String>,

        /// Filter by event type
        #[arg(short = 't', long = "type")]
        event_type: Option<String>,

        /// Maximum events to show
        #[arg(short, long, default_value = "100")]
        limit: usize,

        /// Output as JSON for scripting
        #[arg(long)]
        json: bool,
    },

    /// Show statistics: event counts by source and type
    #[command(after_help = "Example: journal summary --days 7")]
    Summary {
        /// Hours to look back
        #[arg(short = 'H', long, default_value = "24")]
        hours: f64,

        /// Days to look back (overrides hours)
        #[arg(long)]
        days: Option<f64>,

        /// Output as JSON for scripting
        #[arg(long)]
        json: bool,
    },

    /// Add a custom event to the journal
    #[command(after_help = "Example: journal log \"Starting deployment\" deploy custom")]
    Log {
        /// Event message/description
        summary: String,

        /// Source identifier for grouping
        #[arg(default_value = "user")]
        source: String,

        /// Event type for filtering
        #[arg(default_value = "custom")]
        event_type: String,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Some(Commands::What {
            hours,
            days,
            source,
            verbose,
            json,
        }) => cmd_what(hours, days, source, verbose, json),

        Some(Commands::Events {
            hours,
            days,
            source,
            event_type,
            limit,
            json,
        }) => cmd_events(hours, days, source, event_type, limit, json),

        Some(Commands::Summary { hours, days, json }) => cmd_summary(hours, days, json),

        Some(Commands::Log {
            summary,
            source,
            event_type,
        }) => cmd_log(&summary, &source, &event_type),

        None => {
            // Default: run "what" command with default args
            cmd_what(1.0, None, None, false, false)
        }
    }
}

/// What happened? command
fn cmd_what(
    hours: f64,
    days: Option<f64>,
    source: Option<String>,
    verbose: bool,
    json: bool,
) -> Result<()> {
    let hours = days.map(|d| d * 24.0).unwrap_or(hours);
    let since = (Utc::now() - chrono::Duration::seconds((hours * 3600.0) as i64)).timestamp() as f64;

    let collector = EventCollector::new();
    let sources: Option<Vec<&str>> = source.as_ref().map(|s| vec![s.as_str()]);

    let events = collector.collect_all(
        since,
        sources.as_deref(),
        None,
        1000,
    );

    if json {
        let summary = build_summary(&events);
        let result = serde_json::json!({
            "events": events,
            "summary": summary,
        });
        println!("{}", serde_json::to_string_pretty(&result)?);
    } else {
        println!("{}", what_happened(&events, hours, verbose));
    }

    Ok(())
}

/// Events listing command
fn cmd_events(
    hours: f64,
    days: Option<f64>,
    source: Option<String>,
    event_type: Option<String>,
    limit: usize,
    json: bool,
) -> Result<()> {
    let hours = days.map(|d| d * 24.0).unwrap_or(hours);
    let since = (Utc::now() - chrono::Duration::seconds((hours * 3600.0) as i64)).timestamp() as f64;

    let collector = EventCollector::new();
    let sources: Option<Vec<&str>> = source.as_ref().map(|s| vec![s.as_str()]);
    let types: Option<Vec<&str>> = event_type.as_ref().map(|t| vec![t.as_str()]);

    let events = collector.collect_all(
        since,
        sources.as_deref(),
        types.as_deref(),
        limit,
    );

    if json {
        println!("{}", serde_json::to_string_pretty(&events)?);
    } else {
        if events.is_empty() {
            println!("No events found in the specified time range.");
            return Ok(());
        }

        // Print header
        println!("{:<20} {:<12} {:<16} {}", "TIME", "SOURCE", "TYPE", "SUMMARY");
        println!("{}", "-".repeat(80));

        for event in events {
            let time_str = format_time(event.timestamp);
            // Truncate summary to fit
            let summary = if event.summary.len() > 40 {
                format!("{}...", &event.summary[..37])
            } else {
                event.summary.clone()
            };
            println!(
                "{:<20} {:<12} {:<16} {}",
                time_str, event.source, event.event_type, summary
            );
        }
    }

    Ok(())
}

/// Summary command
fn cmd_summary(hours: f64, days: Option<f64>, json: bool) -> Result<()> {
    let hours = days.map(|d| d * 24.0).unwrap_or(hours);
    let since = (Utc::now() - chrono::Duration::seconds((hours * 3600.0) as i64)).timestamp() as f64;

    let collector = EventCollector::new();
    let events = collector.collect_all(since, None, None, 10000);
    let summary = build_summary(&events);

    if json {
        println!("{}", serde_json::to_string_pretty(&summary)?);
    } else {
        println!("{}", format_summary(&summary));
    }

    Ok(())
}

/// Log custom event command
fn cmd_log(summary: &str, source: &str, event_type: &str) -> Result<()> {
    log_event_to_file(source, event_type, summary)?;
    println!("Event logged");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cli_parsing() {
        // Just verify the CLI parses without panic
        let cli = Cli::try_parse_from(["journal", "--help"]);
        // --help returns an error (it exits), so we just check it doesn't panic
        assert!(cli.is_err());
    }

    #[test]
    fn test_what_command_parsing() {
        let cli = Cli::try_parse_from(["journal", "what", "-H", "2"]).unwrap();
        match cli.command {
            Some(Commands::What { hours, .. }) => {
                assert!((hours - 2.0).abs() < 0.001);
            }
            _ => panic!("Expected What command"),
        }
    }

    #[test]
    fn test_events_command_parsing() {
        let cli = Cli::try_parse_from(["journal", "events", "--source", "loop", "--limit", "50"]).unwrap();
        match cli.command {
            Some(Commands::Events { source, limit, .. }) => {
                assert_eq!(source, Some("loop".to_string()));
                assert_eq!(limit, 50);
            }
            _ => panic!("Expected Events command"),
        }
    }

    #[test]
    fn test_log_command_parsing() {
        let cli = Cli::try_parse_from(["journal", "log", "Test message", "user", "custom"]).unwrap();
        match cli.command {
            Some(Commands::Log { summary, source, event_type }) => {
                assert_eq!(summary, "Test message");
                assert_eq!(source, "user");
                assert_eq!(event_type, "custom");
            }
            _ => panic!("Expected Log command"),
        }
    }
}
