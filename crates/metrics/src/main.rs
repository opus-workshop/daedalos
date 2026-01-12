//! metrics - Productivity statistics for Daedalos
//!
//! What gets measured gets managed. What gets visualized gets understood.
//!
//! Aggregates signals from multiple sources - git commits, focus sessions,
//! journal events - to build a picture of actual work.

use anyhow::Result;
use clap::{Parser, Subcommand};

use metrics::db::{date_ago, day_name, today, DailyMetrics, MetricsDatabase};
use metrics::display::{
    draw_bar, format_duration, format_today, format_trend, section_header, title, Colors,
    ProductivityScore,
};
use metrics::focus::{get_daily_focus, get_focus_stats};
use metrics::git::{get_daily_commits, get_git_stats, get_stats_for_days, get_today_stats};

#[derive(Parser)]
#[command(name = "metrics")]
#[command(about = "Productivity statistics for Daedalos - what gets measured gets managed")]
#[command(version)]
#[command(after_help = r#"DATA SOURCES:
    - git: Commits, lines changed, files touched
    - focus: Focus sessions, breaks, completion rate
    - journal: Events, tool usage, errors

PHILOSOPHY:
    Metrics exists because developers have terrible intuition about their
    own productivity. Without data, you can't tell the difference between
    8 hours of focused coding and 6 hours of context-switching.

    This is YOUR data about YOUR work for YOUR improvement.
    All data is local. No dashboards for supervisors.

EXAMPLES:
    metrics                      # Today's summary (default)
    metrics week                 # This week
    metrics commits --days 30    # Commits over 30 days
    metrics focus                # Focus session stats
    metrics trends --days 90     # 90-day trends
    metrics export --csv         # Export data
"#)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// Today's activity summary (default)
    Today {
        /// Output as JSON
        #[arg(long)]
        json: bool,

        /// Project directory (default: current)
        #[arg(long)]
        project: Option<String>,
    },

    /// This week's summary
    Week {
        /// Output as JSON
        #[arg(long)]
        json: bool,

        /// Project directory
        #[arg(long)]
        project: Option<String>,
    },

    /// This month's summary
    Month {
        /// Output as JSON
        #[arg(long)]
        json: bool,

        /// Project directory
        #[arg(long)]
        project: Option<String>,
    },

    /// Git commit statistics
    Commits {
        /// Days to look back (default: 7)
        #[arg(long, default_value = "7")]
        days: u32,

        /// Project directory
        #[arg(long)]
        project: Option<String>,

        /// Output as JSON
        #[arg(long)]
        json: bool,
    },

    /// Focus session statistics
    Focus {
        /// Days to look back (default: 7)
        #[arg(long, default_value = "7")]
        days: u32,

        /// Output as JSON
        #[arg(long)]
        json: bool,
    },

    /// Show trends over time
    Trends {
        /// Days to look back (default: 30)
        #[arg(long, default_value = "30")]
        days: u32,

        /// Project directory
        #[arg(long)]
        project: Option<String>,

        /// Output as JSON
        #[arg(long)]
        json: bool,
    },

    /// Export metrics data
    Export {
        /// Days to export (default: 30)
        #[arg(long, default_value = "30")]
        days: u32,

        /// Output as CSV instead of JSON
        #[arg(long)]
        csv: bool,

        /// Project directory
        #[arg(long)]
        project: Option<String>,
    },

    /// Record an event (for manual logging)
    Record {
        /// Event type (commit, task, session, etc.)
        event_type: String,

        /// Event description
        description: Option<String>,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Some(Commands::Today { json, project }) => cmd_today(json, project.as_deref()),
        Some(Commands::Week { json, project }) => cmd_week(json, project.as_deref()),
        Some(Commands::Month { json, project }) => cmd_month(json, project.as_deref()),
        Some(Commands::Commits { days, project, json }) => {
            cmd_commits(days, project.as_deref(), json)
        }
        Some(Commands::Focus { days, json }) => cmd_focus(days, json),
        Some(Commands::Trends { days, project, json }) => cmd_trends(days, project.as_deref(), json),
        Some(Commands::Export { days, csv, project }) => cmd_export(days, csv, project.as_deref()),
        Some(Commands::Record {
            event_type,
            description,
        }) => cmd_record(&event_type, description.as_deref()),
        None => cmd_today(false, None),
    }
}

/// Today's activity summary
fn cmd_today(json: bool, project: Option<&str>) -> Result<()> {
    let colors = Colors::auto();
    let paths = daedalos_core::Paths::new();
    let project_path = project.map(std::path::Path::new);

    // Get git stats for today
    let git_stats = get_today_stats(project_path)?;

    // Get focus stats for today
    let focus_stats = get_focus_stats(1, &paths.data)?;

    // Build metrics struct
    let metrics = DailyMetrics {
        date: today(),
        commits: git_stats.commits,
        insertions: git_stats.insertions,
        deletions: git_stats.deletions,
        files_touched: git_stats.files_touched,
        focus_sessions: focus_stats.total_sessions,
        focus_minutes: focus_stats.total_minutes,
        focus_completed: focus_stats.completed,
        ..Default::default()
    };

    if json {
        let score = ProductivityScore::from_metrics(&metrics);
        let output = serde_json::json!({
            "date": metrics.date,
            "git": {
                "commits": metrics.commits,
                "insertions": metrics.insertions,
                "deletions": metrics.deletions,
                "files_touched": metrics.files_touched,
            },
            "focus": {
                "sessions": metrics.focus_sessions,
                "minutes": metrics.focus_minutes,
                "completed": metrics.focus_completed,
            },
            "productivity_score": score.score,
        });
        println!("{}", serde_json::to_string_pretty(&output)?);
        return Ok(());
    }

    // Display
    println!("{}", title("Today's Activity", &colors));
    println!("{}{}{}", colors.dim, format_today(), colors.reset);
    println!();

    println!("{}", section_header("Git Activity", &colors));
    println!("  Commits:       {}", metrics.commits);
    println!(
        "  Lines added:   {}{}{}",
        colors.green, metrics.insertions, colors.reset
    );
    println!(
        "  Lines removed: {}{}{}",
        colors.red, metrics.deletions, colors.reset
    );
    println!("  Files touched: {}", metrics.files_touched);
    println!();

    println!("{}", section_header("Focus", &colors));
    let (hours, mins) = focus_stats.hours_minutes();
    println!(
        "  Sessions: {} ({} completed)",
        metrics.focus_sessions, metrics.focus_completed
    );
    println!("  Time: {}h {}m", hours, mins);
    println!();

    // Productivity score
    let score = ProductivityScore::from_metrics(&metrics);
    println!(
        "{}Productivity Score: {}{}",
        colors.bold,
        score.format(&colors),
        colors.reset
    );

    Ok(())
}

/// This week's summary
fn cmd_week(json: bool, project: Option<&str>) -> Result<()> {
    let colors = Colors::auto();
    let paths = daedalos_core::Paths::new();
    let project_path = project.map(std::path::Path::new);

    // Get daily commits for 7 days
    let daily_commits = get_daily_commits(7, project_path)?;
    let max_commits = daily_commits.iter().map(|(_, c)| *c).max().unwrap_or(1);

    // Get git stats for the week
    let git_stats = get_stats_for_days(7, project_path)?;

    // Get focus stats for the week
    let focus_stats = get_focus_stats(7, &paths.data)?;

    if json {
        let output = serde_json::json!({
            "period": "week",
            "daily": daily_commits.iter().map(|(date, commits)| {
                serde_json::json!({
                    "date": date,
                    "commits": commits,
                })
            }).collect::<Vec<_>>(),
            "totals": {
                "commits": git_stats.commits,
                "insertions": git_stats.insertions,
                "deletions": git_stats.deletions,
                "net_lines": git_stats.net_lines(),
            },
            "focus": {
                "sessions": focus_stats.total_sessions,
                "minutes": focus_stats.total_minutes,
                "completion_rate": focus_stats.completion_rate(),
            }
        });
        println!("{}", serde_json::to_string_pretty(&output)?);
        return Ok(());
    }

    println!("{}", title("This Week's Activity", &colors));
    println!();

    // Daily commit chart
    println!("{}", section_header("Daily Commits", &colors));
    for (date, commits) in &daily_commits {
        let day = day_name(date);
        let bar = draw_bar(*commits, max_commits, 20);
        println!(
            "  {}: {}{}{} {}",
            day, colors.green, bar, colors.reset, commits
        );
    }
    println!();

    // Week totals
    println!("{}", section_header("Week Totals", &colors));
    println!("  Total commits:  {}", git_stats.commits);
    println!(
        "  Lines changed:  {}",
        git_stats.insertions + git_stats.deletions
    );
    println!("  Net lines:      {}", git_stats.net_lines());
    println!();

    // Focus stats
    println!("{}", section_header("Focus Stats", &colors));
    println!("  Sessions:        {}", focus_stats.total_sessions);
    println!("  Completion rate: {}%", focus_stats.completion_rate());
    println!(
        "  Total focus:     {}",
        format_duration(focus_stats.total_minutes)
    );

    Ok(())
}

/// This month's summary
fn cmd_month(json: bool, project: Option<&str>) -> Result<()> {
    let colors = Colors::auto();
    let paths = daedalos_core::Paths::new();
    let project_path = project.map(std::path::Path::new);

    // Get weekly commits (4 weeks)
    let mut weekly_commits = Vec::new();
    for week in 0..4 {
        let start = format!("{} days ago", week * 7 + 7);
        let end = format!("{} days ago", week * 7);
        let stats = get_git_stats(&start, &end, project_path)?;
        weekly_commits.push((format!("Week {}", 4 - week), stats.commits));
    }
    weekly_commits.reverse();

    let max_commits = weekly_commits.iter().map(|(_, c)| *c).max().unwrap_or(1);

    // Get git stats for the month
    let git_stats = get_stats_for_days(30, project_path)?;

    // Get focus stats for the month
    let focus_stats = get_focus_stats(30, &paths.data)?;

    if json {
        let output = serde_json::json!({
            "period": "month",
            "weekly": weekly_commits.iter().map(|(week, commits)| {
                serde_json::json!({
                    "week": week,
                    "commits": commits,
                })
            }).collect::<Vec<_>>(),
            "totals": {
                "commits": git_stats.commits,
                "insertions": git_stats.insertions,
                "deletions": git_stats.deletions,
                "files_touched": git_stats.files_touched,
            },
            "focus": {
                "sessions": focus_stats.total_sessions,
                "total_hours": focus_stats.total_minutes / 60,
                "avg_per_day": focus_stats.avg_per_day(30),
            }
        });
        println!("{}", serde_json::to_string_pretty(&output)?);
        return Ok(());
    }

    println!("{}", title("This Month's Activity", &colors));
    println!();

    // Weekly commit chart
    println!("{}", section_header("Weekly Commits", &colors));
    for (week, commits) in &weekly_commits {
        let bar = draw_bar(*commits, max_commits, 25);
        println!(
            "  {}: {}{}{} {}",
            week, colors.green, bar, colors.reset, commits
        );
    }
    println!();

    // Month totals
    println!("{}", section_header("Month Totals", &colors));
    println!("  Total commits:  {}", git_stats.commits);
    println!(
        "  Lines added:    {}{}{}",
        colors.green, git_stats.insertions, colors.reset
    );
    println!(
        "  Lines removed:  {}{}{}",
        colors.red, git_stats.deletions, colors.reset
    );
    println!("  Files touched:  {}", git_stats.files_touched);
    println!();

    // Focus stats
    println!("{}", section_header("Focus Stats", &colors));
    println!("  Sessions:       {}", focus_stats.total_sessions);
    println!(
        "  Total focus:    {}",
        format_duration(focus_stats.total_minutes)
    );
    println!("  Avg per day:    {}m", focus_stats.avg_per_day(30));

    Ok(())
}

/// Git commit statistics
fn cmd_commits(days: u32, project: Option<&str>, json: bool) -> Result<()> {
    let colors = Colors::auto();
    let project_path = project.map(std::path::Path::new);

    let git_stats = get_stats_for_days(days, project_path)?;

    if json {
        let output = serde_json::json!({
            "days": days,
            "commits": git_stats.commits,
            "insertions": git_stats.insertions,
            "deletions": git_stats.deletions,
            "net_lines": git_stats.net_lines(),
            "files_touched": git_stats.files_touched,
            "by_author": git_stats.by_author,
            "peak_hour": git_stats.peak_hour(),
        });
        println!("{}", serde_json::to_string_pretty(&output)?);
        return Ok(());
    }

    println!(
        "{}",
        title(&format!("Commit Statistics ({} days)", days), &colors)
    );
    println!();

    println!("{}", section_header("Overview", &colors));
    println!("  Total commits:  {}", git_stats.commits);
    println!(
        "  Lines added:    {}+{}{}",
        colors.green, git_stats.insertions, colors.reset
    );
    println!(
        "  Lines removed:  {}-{}{}",
        colors.red, git_stats.deletions, colors.reset
    );
    println!("  Net change:     {}", git_stats.net_lines());
    println!("  Files modified: {}", git_stats.files_touched);
    println!();

    // Peak hour
    if let Some((hour, count)) = git_stats.peak_hour() {
        println!("{}", section_header("Commits by Hour", &colors));
        println!("  Peak hour: {}:00 ({} commits)", hour, count);
        println!();
    }

    // Top contributors
    if !git_stats.by_author.is_empty() {
        println!("{}", section_header("Contributors", &colors));
        let mut authors: Vec<_> = git_stats.by_author.iter().collect();
        authors.sort_by(|a, b| b.1.cmp(a.1));
        for (author, count) in authors.iter().take(5) {
            println!("  {}: {} commits", author, count);
        }
    }

    Ok(())
}

/// Focus session statistics
fn cmd_focus(days: u32, json: bool) -> Result<()> {
    let colors = Colors::auto();
    let paths = daedalos_core::Paths::new();

    let focus_stats = get_focus_stats(days, &paths.data)?;
    let daily_focus = get_daily_focus(days.min(7), &paths.data)?;
    let max_minutes = daily_focus
        .iter()
        .map(|(_, s)| s.total_minutes)
        .max()
        .unwrap_or(1);

    if json {
        let output = serde_json::json!({
            "days": days,
            "total_sessions": focus_stats.total_sessions,
            "total_minutes": focus_stats.total_minutes,
            "completed": focus_stats.completed,
            "completion_rate": focus_stats.completion_rate(),
            "avg_per_day": focus_stats.avg_per_day(days),
        });
        println!("{}", serde_json::to_string_pretty(&output)?);
        return Ok(());
    }

    println!(
        "{}",
        title(&format!("Focus Statistics ({} days)", days), &colors)
    );
    println!();

    println!("{}", section_header("Overview", &colors));
    println!("  Total sessions: {}", focus_stats.total_sessions);
    println!(
        "  Completed:      {} ({}%)",
        focus_stats.completed,
        focus_stats.completion_rate()
    );
    println!(
        "  Total time:     {}",
        format_duration(focus_stats.total_minutes)
    );
    println!("  Avg per day:    {}m", focus_stats.avg_per_day(days));
    println!();

    // Completion rate bar
    println!("{}", section_header("Completion Rate", &colors));
    let rate = focus_stats.completion_rate();
    let bar = draw_bar(rate, 100, 30);
    println!(
        "  [{}{}{}] {}%",
        colors.green, bar, colors.reset, rate
    );
    println!();

    // Daily focus chart
    if !daily_focus.is_empty() {
        println!("{}", section_header("Daily Focus Time", &colors));
        for (date, stats) in &daily_focus {
            let day = day_name(date);
            let bar = draw_bar(stats.total_minutes, max_minutes, 20);
            println!(
                "  {}: {}{}{} {}m",
                day, colors.magenta, bar, colors.reset, stats.total_minutes
            );
        }
    }

    Ok(())
}

/// Show trends over time
fn cmd_trends(days: u32, project: Option<&str>, json: bool) -> Result<()> {
    let colors = Colors::auto();
    let paths = daedalos_core::Paths::new();
    let project_path = project.map(std::path::Path::new);

    // Current period
    let current_git = get_stats_for_days(days, project_path)?;
    let current_focus = get_focus_stats(days, &paths.data)?;

    // Previous period
    let prev_since = format!("{} days ago", days * 2);
    let prev_until = format!("{} days ago", days);
    let prev_git = get_git_stats(&prev_since, &prev_until, project_path)?;
    let _prev_focus = get_focus_stats(days, &paths.data)?; // Note: this would need date range support

    if json {
        let output = serde_json::json!({
            "period_days": days,
            "current": {
                "commits": current_git.commits,
                "focus_minutes": current_focus.total_minutes,
            },
            "previous": {
                "commits": prev_git.commits,
            },
            "trends": {
                "commits": format_trend(current_git.commits, prev_git.commits, &Colors::new(false)),
            }
        });
        println!("{}", serde_json::to_string_pretty(&output)?);
        return Ok(());
    }

    println!("{}", title(&format!("Trends ({} days)", days), &colors));
    println!();

    // Commit trend
    println!("{}", section_header("Commit Trend", &colors));
    println!("  Current period:  {} commits", current_git.commits);
    println!("  Previous period: {} commits", prev_git.commits);
    println!(
        "  Change:          {}",
        format_trend(current_git.commits, prev_git.commits, &colors)
    );
    println!();

    // Focus trend
    println!("{}", section_header("Focus Trend", &colors));
    println!("  Focus sessions: {}", current_focus.total_sessions);
    println!(
        "  Focus time:     {}",
        format_duration(current_focus.total_minutes)
    );

    Ok(())
}

/// Export metrics data
fn cmd_export(days: u32, csv: bool, project: Option<&str>) -> Result<()> {
    let paths = daedalos_core::Paths::new();
    let project_path = project.map(std::path::Path::new);

    if csv {
        println!("date,commits,insertions,deletions,focus_sessions,focus_minutes");

        for i in (0..days).rev() {
            let date = date_ago(i as i64);

            // Git stats for the day
            let since = format!("{} 00:00", date);
            let until = format!("{} 23:59", date);
            let git_stats = get_git_stats(&since, &until, project_path)?;

            // Focus stats for the day
            let focus_stats =
                metrics::focus::get_focus_stats_for_date(&date, &paths.data)?;

            println!(
                "{},{},{},{},{},{}",
                date,
                git_stats.commits,
                git_stats.insertions,
                git_stats.deletions,
                focus_stats.total_sessions,
                focus_stats.total_minutes
            );
        }
    } else {
        let mut daily_data = Vec::new();

        for i in (0..days).rev() {
            let date = date_ago(i as i64);

            let since = format!("{} 00:00", date);
            let until = format!("{} 23:59", date);
            let git_stats = get_git_stats(&since, &until, project_path)?;
            let focus_stats =
                metrics::focus::get_focus_stats_for_date(&date, &paths.data)?;

            daily_data.push(serde_json::json!({
                "date": date,
                "commits": git_stats.commits,
                "insertions": git_stats.insertions,
                "deletions": git_stats.deletions,
                "focus_sessions": focus_stats.total_sessions,
                "focus_minutes": focus_stats.total_minutes,
            }));
        }

        let output = serde_json::json!({
            "period": format!("{} days", days),
            "generated": chrono::Utc::now().to_rfc3339(),
            "daily": daily_data,
        });

        println!("{}", serde_json::to_string_pretty(&output)?);
    }

    Ok(())
}

/// Record a manual event
fn cmd_record(event_type: &str, _description: Option<&str>) -> Result<()> {
    let paths = daedalos_core::Paths::new();
    let db = MetricsDatabase::open(&paths.data.join("metrics"))?;

    let today = today();
    let mut metrics = db.get_daily(&today)?.unwrap_or_else(|| DailyMetrics::new(&today));

    match event_type.to_lowercase().as_str() {
        "commit" | "commits" => {
            metrics.commits += 1;
            println!("Recorded commit (total today: {})", metrics.commits);
        }
        "task" | "tasks" => {
            // Tasks aren't directly tracked, but we could log to journal
            println!("Task recorded");
        }
        "session" | "focus" => {
            metrics.focus_sessions += 1;
            metrics.focus_completed += 1;
            metrics.focus_minutes += 25; // Default pomodoro
            println!(
                "Recorded focus session (total today: {})",
                metrics.focus_sessions
            );
        }
        "error" | "errors" => {
            metrics.errors += 1;
            println!("Recorded error (total today: {})", metrics.errors);
        }
        _ => {
            println!(
                "Unknown event type: {}. Valid types: commit, task, session, error",
                event_type
            );
            return Ok(());
        }
    }

    db.upsert_daily(&metrics)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cli_parsing() {
        let cli = Cli::try_parse_from(["metrics", "--help"]);
        // --help returns an error (it exits), but shouldn't panic
        assert!(cli.is_err());
    }

    #[test]
    fn test_today_command() {
        let cli = Cli::try_parse_from(["metrics", "today", "--json"]).unwrap();
        match cli.command {
            Some(Commands::Today { json, .. }) => assert!(json),
            _ => panic!("Expected Today command"),
        }
    }

    #[test]
    fn test_commits_command() {
        let cli = Cli::try_parse_from(["metrics", "commits", "--days", "30"]).unwrap();
        match cli.command {
            Some(Commands::Commits { days, .. }) => assert_eq!(days, 30),
            _ => panic!("Expected Commits command"),
        }
    }

    #[test]
    fn test_export_csv() {
        let cli = Cli::try_parse_from(["metrics", "export", "--csv", "--days", "7"]).unwrap();
        match cli.command {
            Some(Commands::Export { csv, days, .. }) => {
                assert!(csv);
                assert_eq!(days, 7);
            }
            _ => panic!("Expected Export command"),
        }
    }
}
