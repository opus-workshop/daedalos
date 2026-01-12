//! trust - Context-aware permission system for Daedalos
//!
//! "Permissions without friction. Safety without interruption."

use anyhow::Result;
use chrono::{Duration, Utc};
use clap::{Parser, Subcommand};
use tracing_subscriber::EnvFilter;

use trust::{
    audit::{AuditLog, AuditQuery},
    config::TrustConfig,
    evaluator::{Action, EvalRequest, TrustEvaluator},
    pattern::{PatternDecision, PatternStore},
    session::SessionManager,
};

/// trust - Context-aware permission system for Daedalos
#[derive(Parser)]
#[command(name = "trust")]
#[command(version = "1.0.0")]
#[command(about = "Context-aware permission system for Daedalos")]
#[command(long_about = "Context-aware permission system for Daedalos.\n\n\
    Traditional permission systems are OPERATION-CENTRIC: \"May I run `rm`?\"\n\
    Daedalos trust is CONTEXT-CENTRIC: \"May I run `rm *.pyc` in `~/Daedalos`\n\
    during a `loop` session that you started?\"\n\n\
    Context changes everything.")]
pub struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Show trust status for current context
    #[command(about = "Show trust status for current directory or session")]
    Status {
        /// Session ID to show status for
        #[arg(long)]
        session: Option<String>,
    },

    /// Evaluate if an operation would be allowed
    #[command(about = "Evaluate if an operation would be allowed")]
    Eval {
        /// The tool/command to evaluate
        tool: String,

        /// Arguments to the tool
        args: Vec<String>,

        /// Domain for network operations
        #[arg(long)]
        domain: Option<String>,

        /// Git branch for git operations
        #[arg(long)]
        branch: Option<String>,

        /// Session ID
        #[arg(long)]
        session: Option<String>,

        /// Output as JSON
        #[arg(long)]
        json: bool,
    },

    /// Add a pattern to always allow
    #[command(about = "Add a pattern to the allow list")]
    Allow {
        /// Pattern to allow (e.g., "rm *.pyc")
        pattern: String,

        /// Scope for the pattern (e.g., "~/projects/*")
        #[arg(long, default_value = "*")]
        scope: String,
    },

    /// Add a pattern to always deny
    #[command(about = "Add a pattern to the deny list")]
    Deny {
        /// Pattern to deny
        pattern: String,

        /// Scope for the pattern
        #[arg(long, default_value = "*")]
        scope: String,
    },

    /// List learned patterns
    #[command(about = "List learned patterns")]
    Patterns {
        /// Filter by scope
        #[arg(long)]
        scope: Option<String>,

        /// Filter pattern string
        filter: Option<String>,
    },

    /// Query the audit log
    #[command(about = "Query the trust audit log")]
    Audit {
        /// Filter by session ID
        #[arg(long)]
        session: Option<String>,

        /// Show only denied operations
        #[arg(long)]
        denied: bool,

        /// Show entries since (e.g., "1h", "1d", "1w")
        #[arg(long)]
        since: Option<String>,

        /// Limit number of results
        #[arg(long, default_value = "20")]
        limit: usize,

        /// Output as JSON
        #[arg(long)]
        json: bool,
    },

    /// Show trust statistics
    #[command(about = "Show trust statistics")]
    Stats {
        /// Show stats since (e.g., "1h", "1d", "1w")
        #[arg(long)]
        since: Option<String>,
    },

    /// List active sessions
    #[command(about = "List active trust sessions")]
    Sessions {
        /// Output as JSON
        #[arg(long)]
        json: bool,
    },

    /// Reset trust data
    #[command(about = "Reset trust data")]
    Reset {
        /// Clear learned patterns
        #[arg(long)]
        patterns: bool,

        /// Clear all sessions
        #[arg(long)]
        sessions: bool,

        /// Clear everything
        #[arg(long)]
        all: bool,
    },

    /// Initialize default configuration
    #[command(about = "Initialize default trust configuration")]
    Init {
        /// Force overwrite existing config
        #[arg(long)]
        force: bool,
    },
}

fn main() -> Result<()> {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .init();

    let cli = Cli::parse();

    match cli.command {
        Commands::Status { session } => cmd_status(session),
        Commands::Eval {
            tool,
            args,
            domain,
            branch,
            session,
            json,
        } => cmd_eval(tool, args, domain, branch, session, json),
        Commands::Allow { pattern, scope } => cmd_allow(pattern, scope),
        Commands::Deny { pattern, scope } => cmd_deny(pattern, scope),
        Commands::Patterns { scope, filter } => cmd_patterns(scope, filter),
        Commands::Audit {
            session,
            denied,
            since,
            limit,
            json,
        } => cmd_audit(session, denied, since, limit, json),
        Commands::Stats { since } => cmd_stats(since),
        Commands::Sessions { json } => cmd_sessions(json),
        Commands::Reset {
            patterns,
            sessions,
            all,
        } => cmd_reset(patterns, sessions, all),
        Commands::Init { force } => cmd_init(force),
    }
}

fn cmd_status(session_id: Option<String>) -> Result<()> {
    let config = TrustConfig::load()?;
    let cwd = std::env::current_dir()?;

    // Get trust level for current directory
    let level = config.trust_level_for_path(&cwd);

    println!("Trust Status");
    println!("{}", "=".repeat(50));
    println!();
    println!("Working directory: {}", cwd.display());
    println!("Trust level: {}", level);
    println!();

    // Show session info if provided
    if let Some(id) = session_id {
        let manager = SessionManager::new();
        if let Some(session) = manager.load_session(&id)? {
            println!("Session: {}", session.id);
            println!("  Type: {:?}", session.session_type);
            println!("  Base level: {}", session.base_level);
            if let Some(escalated) = session.escalated_level {
                println!("  Escalated to: {}", escalated);
            }
            println!("  Approved patterns: {}", session.approved_patterns.len());
            println!("  Approved domains: {}", session.approved_domains.len());
            println!("  Started: {}", session.started_at);
            println!("  Last activity: {}", session.last_activity);
        } else {
            println!("Session not found: {}", id);
        }
    }

    // Show recent audit stats
    let audit = AuditLog::new();
    let since = Utc::now() - Duration::hours(24);
    if let Ok(stats) = audit.stats(Some(since)) {
        println!();
        println!("Last 24 hours:");
        println!("  Total decisions: {}", stats.total);
        println!("  Allowed: {} ({:.1}%)", stats.allowed,
            if stats.total > 0 { stats.allowed as f64 / stats.total as f64 * 100.0 } else { 0.0 });
        println!("  Denied: {}", stats.denied);
        println!("  Asked: {}", stats.asked);
        println!("  Auto-allow rate: {:.1}%", stats.auto_allow_rate());
    }

    Ok(())
}

fn cmd_eval(
    tool: String,
    args: Vec<String>,
    domain: Option<String>,
    branch: Option<String>,
    session_id: Option<String>,
    json_output: bool,
) -> Result<()> {
    let evaluator = TrustEvaluator::new()?;
    let cwd = std::env::current_dir()?;

    // Load session if provided
    let session = if let Some(id) = &session_id {
        let manager = SessionManager::new();
        manager.load_session(id)?
    } else {
        None
    };

    let args_refs: Vec<&str> = args.iter().map(|s| s.as_str()).collect();

    let request = EvalRequest {
        tool: &tool,
        args: &args_refs,
        working_dir: &cwd,
        domain: domain.as_deref(),
        branch: branch.as_deref(),
    };

    let decision = evaluator.evaluate(&request, session.as_ref());

    if json_output {
        println!("{}", serde_json::to_string_pretty(&decision)?);
    } else {
        let action_str = match decision.action {
            Action::Allow => "ALLOW",
            Action::Deny => "DENY",
            Action::Ask => "ASK",
        };

        println!("{}: {} {:?}", action_str, tool, args);
        println!("  Reason: {:?}", decision.reason);
        println!("  Details: {}", decision.details);
        if let Some(pattern) = &decision.matched_pattern {
            println!("  Matched pattern: {}", pattern);
        }
    }

    // Exit with appropriate code
    match decision.action {
        Action::Allow => std::process::exit(0),
        Action::Deny => std::process::exit(1),
        Action::Ask => std::process::exit(2),
    }
}

fn cmd_allow(pattern: String, scope: String) -> Result<()> {
    let mut store = PatternStore::load()?;
    store.record(&pattern, &scope, PatternDecision::Allow);
    store.confirm(&pattern, &scope);
    store.save()?;

    println!("Added allow pattern: {} (scope: {})", pattern, scope);
    Ok(())
}

fn cmd_deny(pattern: String, scope: String) -> Result<()> {
    let mut store = PatternStore::load()?;
    store.record(&pattern, &scope, PatternDecision::Deny);
    store.confirm(&pattern, &scope);
    store.save()?;

    println!("Added deny pattern: {} (scope: {})", pattern, scope);
    Ok(())
}

fn cmd_patterns(scope: Option<String>, filter: Option<String>) -> Result<()> {
    let store = PatternStore::load()?;

    println!("{:<30} {:<20} {:<10} {:<6} {}", "PATTERN", "SCOPE", "DECISION", "COUNT", "CONFIRMED");
    println!("{}", "-".repeat(80));

    for pattern in store.list() {
        // Apply filters
        if let Some(ref s) = scope {
            if !pattern.scope.contains(s) {
                continue;
            }
        }
        if let Some(ref f) = filter {
            if !pattern.pattern.contains(f) {
                continue;
            }
        }

        let decision_str = match pattern.decision {
            PatternDecision::Allow => "allow",
            PatternDecision::Deny => "deny",
            PatternDecision::Ask => "ask",
        };

        let confirmed_str = if pattern.confirmed { "yes" } else { "no" };

        let pattern_display = if pattern.pattern.len() > 28 {
            format!("{}...", &pattern.pattern[..28])
        } else {
            pattern.pattern.clone()
        };

        let scope_display = if pattern.scope.len() > 18 {
            format!("{}...", &pattern.scope[..18])
        } else {
            pattern.scope.clone()
        };

        println!(
            "{:<30} {:<20} {:<10} {:<6} {}",
            pattern_display, scope_display, decision_str, pattern.count, confirmed_str
        );
    }

    Ok(())
}

fn cmd_audit(
    session: Option<String>,
    denied: bool,
    since: Option<String>,
    limit: usize,
    json_output: bool,
) -> Result<()> {
    let audit = AuditLog::new();

    let mut query = AuditQuery::default().limit(limit);

    if let Some(id) = session {
        query = query.session(&id);
    }

    if denied {
        query = query.action(Action::Deny);
    }

    if let Some(since_str) = since {
        let since_time = parse_duration(&since_str)?;
        query = query.since(since_time);
    }

    let entries = audit.query(query)?;

    if json_output {
        println!("{}", serde_json::to_string_pretty(&entries)?);
    } else {
        println!(
            "{:<20} {:<12} {:<8} {:<15} {}",
            "TIMESTAMP", "SESSION", "ACTION", "OPERATION", "DETAILS"
        );
        println!("{}", "-".repeat(80));

        for entry in entries {
            let action_str = match entry.decision {
                Action::Allow => "ALLOW",
                Action::Deny => "DENY",
                Action::Ask => "ASK",
            };

            let time_str = entry.timestamp.format("%Y-%m-%d %H:%M").to_string();
            let session_short = if entry.session_id.len() > 10 {
                format!("{}...", &entry.session_id[..10])
            } else {
                entry.session_id.clone()
            };

            let details_short = if entry.details.len() > 30 {
                format!("{}...", &entry.details[..30])
            } else {
                entry.details.clone()
            };

            println!(
                "{:<20} {:<12} {:<8} {:<15} {}",
                time_str, session_short, action_str, entry.operation, details_short
            );
        }
    }

    Ok(())
}

fn cmd_stats(since: Option<String>) -> Result<()> {
    let audit = AuditLog::new();

    let since_time = if let Some(s) = since {
        Some(parse_duration(&s)?)
    } else {
        None
    };

    let stats = audit.stats(since_time)?;

    println!("Trust Statistics");
    println!("{}", "=".repeat(40));
    println!();
    println!("Total decisions: {}", stats.total);
    println!("  Allowed: {}", stats.allowed);
    println!("  Denied: {}", stats.denied);
    println!("  Asked: {}", stats.asked);
    println!();
    println!("User prompted: {}", stats.user_prompted);
    println!("Pattern matches: {}", stats.pattern_matches);
    println!();
    println!("Auto-allow rate: {:.1}%", stats.auto_allow_rate());

    if stats.total > 0 {
        let effectiveness = ((stats.total - stats.asked) as f64 / stats.total as f64) * 100.0;
        println!("Trust effectiveness: {:.1}%", effectiveness);
        println!("  (Operations that didn't require user prompting)");
    }

    Ok(())
}

fn cmd_sessions(json_output: bool) -> Result<()> {
    let manager = SessionManager::new();
    let sessions = manager.list_sessions()?;

    if json_output {
        println!("{}", serde_json::to_string_pretty(&sessions)?);
    } else {
        if sessions.is_empty() {
            println!("No active sessions");
            return Ok(());
        }

        println!(
            "{:<20} {:<10} {:<12} {:<12} {}",
            "ID", "TYPE", "BASE", "EFFECTIVE", "LAST ACTIVITY"
        );
        println!("{}", "-".repeat(70));

        for session in sessions {
            let id_short = if session.id.len() > 18 {
                format!("{}...", &session.id[..18])
            } else {
                session.id.clone()
            };

            let type_str = format!("{:?}", session.session_type).to_lowercase();
            let effective = session.effective_level();

            let time_str = session.last_activity.format("%Y-%m-%d %H:%M").to_string();

            println!(
                "{:<20} {:<10} {:<12} {:<12} {}",
                id_short, type_str, session.base_level, effective, time_str
            );
        }
    }

    Ok(())
}

fn cmd_reset(patterns: bool, sessions: bool, all: bool) -> Result<()> {
    if all || patterns {
        let mut store = PatternStore::load()?;
        let count = store.list().count();
        store = PatternStore::default();
        store.save()?;
        println!("Cleared {} patterns", count);
    }

    if all || sessions {
        let manager = SessionManager::new();
        let sessions = manager.list_sessions()?;
        let count = sessions.len();
        for session in sessions {
            manager.end_session(&session.id)?;
        }
        println!("Cleared {} sessions", count);
    }

    if !patterns && !sessions && !all {
        println!("Specify --patterns, --sessions, or --all");
    }

    Ok(())
}

fn cmd_init(force: bool) -> Result<()> {
    let config = TrustConfig::default();
    let paths = daedalos_core::Paths::new();
    let config_path = paths.config.join("trust.yaml");

    if config_path.exists() && !force {
        println!("Config already exists at {:?}", config_path);
        println!("Use --force to overwrite");
        return Ok(());
    }

    config.save()?;
    println!("Created default trust config at {:?}", config_path);

    // Also create empty patterns file
    let patterns = PatternStore::default();
    patterns.save()?;
    println!("Created empty patterns store");

    Ok(())
}

/// Parse a duration string like "1h", "1d", "1w" into a DateTime
fn parse_duration(s: &str) -> Result<chrono::DateTime<Utc>> {
    let s = s.trim();
    let (num, unit) = s.split_at(s.len() - 1);
    let num: i64 = num.parse()?;

    let duration = match unit {
        "h" => Duration::hours(num),
        "d" => Duration::days(num),
        "w" => Duration::weeks(num),
        "m" => Duration::minutes(num),
        _ => anyhow::bail!("Unknown duration unit: {}", unit),
    };

    Ok(Utc::now() - duration)
}
