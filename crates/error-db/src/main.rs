//! error-db - Error Pattern Database
//!
//! "Agents solve the same errors repeatedly. Every session starts fresh.
//!  Error-DB is institutional memory for debugging."
//!
//! Commands:
//! - match: Search for matching patterns (alias: search)
//! - add: Add a new error pattern
//! - solution: Add solution to existing pattern
//! - confirm: Mark solution as successful
//! - report: Mark solution as failed
//! - list: List all patterns
//! - show: Show pattern details
//! - stats: Show database statistics

mod db;
mod matcher;

use anyhow::{bail, Context, Result};
use clap::{Parser, Subcommand};
use serde_json::json;
use std::io::{self, Read};

use crate::db::ErrorDatabase;
use crate::matcher::{format_match, PatternMatcher};

#[derive(Parser)]
#[command(name = "error-db")]
#[command(about = "Error Pattern Database - institutional memory for debugging")]
#[command(version)]
#[command(after_help = r#"TRIGGER:
    Use error-db when you encounter ANY error message. Check if it has been
    solved before instead of debugging from scratch every time.

EXAMPLES:
    # Search for an error
    error-db match "Cannot find module 'express'"

    # Pipe error from command
    npm test 2>&1 | error-db match --stdin

    # Add new pattern
    error-db add "TypeError: Cannot read property 'X' of undefined"

    # Add solution to pattern
    error-db solution <pattern-id> "Check if object is null before accessing"

    # Confirm solution worked
    error-db confirm <solution-id>

COMMANDS:
    match     Search for matching error patterns (alias: search)
    add       Add a new error pattern to the database
    solution  Add a solution to an existing pattern
    confirm   Mark a solution as successful (increases confidence)
    report    Mark a solution as failed (decreases confidence)
    list      List all patterns (filter by --language, --scope)
    show      Show pattern details with all solutions
    stats     Show database statistics
"#)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// Search for matching error patterns (alias: search)
    #[command(alias = "search")]
    Match {
        /// Error text to search for
        error: Option<String>,

        /// Read error from stdin
        #[arg(long)]
        stdin: bool,

        /// Output as JSON
        #[arg(long)]
        json: bool,

        /// Show all matches, not just the best one
        #[arg(long)]
        all: bool,
    },

    /// Add a new error pattern
    Add {
        /// Error pattern text
        pattern: String,

        /// Initial solution (optional)
        solution: Option<String>,

        /// Pattern scope: global, language, framework, project
        #[arg(long, default_value = "global")]
        scope: String,

        /// Programming language (for language-scoped patterns)
        #[arg(long)]
        language: Option<String>,

        /// Framework (for framework-scoped patterns)
        #[arg(long)]
        framework: Option<String>,

        /// Auto-fix command
        #[arg(long)]
        command: Option<String>,

        /// Category/tags (comma-separated)
        #[arg(long)]
        category: Option<String>,
    },

    /// Add solution to an existing pattern
    Solution {
        /// Pattern ID
        pattern_id: String,

        /// Solution text
        solution: String,

        /// Auto-fix command (optional)
        #[arg(long)]
        command: Option<String>,
    },

    /// Confirm a solution worked (increases confidence)
    Confirm {
        /// Solution ID
        solution_id: String,
    },

    /// Report that a solution didn't work (decreases confidence)
    Report {
        /// Solution ID
        solution_id: String,
    },

    /// List all patterns
    List {
        /// Filter by language
        #[arg(long)]
        language: Option<String>,

        /// Filter by scope
        #[arg(long)]
        scope: Option<String>,

        /// Output as JSON
        #[arg(long)]
        json: bool,
    },

    /// Show details of a specific pattern
    Show {
        /// Pattern ID
        pattern_id: String,

        /// Output as JSON
        #[arg(long)]
        json: bool,
    },

    /// Show database statistics
    Stats {
        /// Output as JSON
        #[arg(long)]
        json: bool,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    let mut db = ErrorDatabase::open(None).context("Failed to open error database")?;

    match cli.command {
        Some(Commands::Match {
            error,
            stdin,
            json,
            all,
        }) => cmd_match(&db, error, stdin, json, all),
        Some(Commands::Add {
            pattern,
            solution,
            scope,
            language,
            framework,
            command,
            category,
        }) => cmd_add(
            &mut db, &pattern, solution, &scope, language, framework, command, category,
        ),
        Some(Commands::Solution {
            pattern_id,
            solution,
            command,
        }) => cmd_solution(&mut db, &pattern_id, &solution, command),
        Some(Commands::Confirm { solution_id }) => cmd_confirm(&mut db, &solution_id),
        Some(Commands::Report { solution_id }) => cmd_report(&mut db, &solution_id),
        Some(Commands::List {
            language,
            scope,
            json,
        }) => cmd_list(&db, language, scope, json),
        Some(Commands::Show { pattern_id, json }) => cmd_show(&db, &pattern_id, json),
        Some(Commands::Stats { json }) => cmd_stats(&db, json),
        None => {
            // Default: show help
            println!("error-db - Error Pattern Database");
            println!();
            println!("Use 'error-db --help' for usage information");
            println!("Use 'error-db match <error>' to search for solutions");
            Ok(())
        }
    }
}

/// Search for matching error patterns
fn cmd_match(
    db: &ErrorDatabase,
    error: Option<String>,
    stdin: bool,
    as_json: bool,
    all: bool,
) -> Result<()> {
    let error_text = if stdin {
        let mut buffer = String::new();
        io::stdin()
            .read_to_string(&mut buffer)
            .context("Failed to read from stdin")?;
        buffer
    } else {
        error.ok_or_else(|| anyhow::anyhow!("Error text required (or use --stdin)"))?
    };

    let matcher = PatternMatcher::new(db);

    if all {
        let matches = matcher.find_matches(&error_text, 0.4)?;

        if matches.is_empty() {
            if as_json {
                println!("{}", json!({"error": "No matching patterns found"}));
            } else {
                println!("No matching patterns found.");
                println!("\nAdd this error with: error-db add '<error pattern>'");
            }
            std::process::exit(2);
        }

        if as_json {
            let output: Vec<_> = matches
                .iter()
                .map(|m| {
                    json!({
                        "pattern": {
                            "id": m.pattern.id,
                            "pattern": m.pattern.pattern,
                            "language": m.pattern.language,
                            "scope": m.pattern.scope,
                        },
                        "score": m.score,
                        "solutions": m.solutions.iter().map(|s| json!({
                            "id": s.id,
                            "solution": s.solution,
                            "command": s.command,
                            "confidence": s.confidence,
                        })).collect::<Vec<_>>(),
                    })
                })
                .collect();
            println!("{}", serde_json::to_string_pretty(&output)?);
        } else {
            for (i, m) in matches.iter().enumerate() {
                if i > 0 {
                    println!("\n{}", "=".repeat(60));
                }
                println!("{}", format_match(m, true));
            }
        }
    } else {
        let result = matcher.search(&error_text)?;

        match result {
            Some(m) => {
                if as_json {
                    let output = json!({
                        "pattern": {
                            "id": m.pattern.id,
                            "pattern": m.pattern.pattern,
                            "language": m.pattern.language,
                            "scope": m.pattern.scope,
                        },
                        "score": m.score,
                        "solutions": m.solutions.iter().map(|s| json!({
                            "id": s.id,
                            "solution": s.solution,
                            "command": s.command,
                            "confidence": s.confidence,
                        })).collect::<Vec<_>>(),
                    });
                    println!("{}", serde_json::to_string_pretty(&output)?);
                } else {
                    println!("{}", format_match(&m, false));
                }
            }
            None => {
                if as_json {
                    println!("{}", json!({"error": "No matching patterns found"}));
                } else {
                    println!("No matching patterns found.");
                    println!("\nAdd this error with: error-db add '<error pattern>'");
                }
                std::process::exit(2);
            }
        }
    }

    Ok(())
}

/// Add a new error pattern
fn cmd_add(
    db: &mut ErrorDatabase,
    pattern: &str,
    solution: Option<String>,
    scope: &str,
    language: Option<String>,
    framework: Option<String>,
    command: Option<String>,
    category: Option<String>,
) -> Result<()> {
    let tags: Option<Vec<&str>> = category.as_ref().map(|c| c.split(',').collect());

    let has_solution = solution.is_some();
    let solution_data = solution.map(|s| (s, command));

    let pattern_id = db.add_pattern(
        pattern,
        scope,
        language.as_deref(),
        framework.as_deref(),
        tags,
        solution_data,
    )?;

    println!("Pattern added: {}", pattern_id);

    if !has_solution {
        println!("Add a solution with: error-db solution {} '<solution>'", pattern_id);
    }

    Ok(())
}

/// Add solution to existing pattern
fn cmd_solution(
    db: &mut ErrorDatabase,
    pattern_id: &str,
    solution: &str,
    command: Option<String>,
) -> Result<()> {
    // Verify pattern exists
    if db.get_pattern(pattern_id)?.is_none() {
        bail!("Pattern not found: {}", pattern_id);
    }

    let solution_id = db.add_solution(pattern_id, solution, command.as_deref())?;
    println!("Solution added: {}", solution_id);

    Ok(())
}

/// Confirm a solution worked
fn cmd_confirm(db: &mut ErrorDatabase, solution_id: &str) -> Result<()> {
    // Verify solution exists
    if db.get_solution(solution_id)?.is_none() {
        bail!("Solution not found: {}", solution_id);
    }

    db.confirm_solution(solution_id)?;
    println!("Solution confirmed! Confidence increased.");

    Ok(())
}

/// Report that a solution didn't work
fn cmd_report(db: &mut ErrorDatabase, solution_id: &str) -> Result<()> {
    // Verify solution exists
    if db.get_solution(solution_id)?.is_none() {
        bail!("Solution not found: {}", solution_id);
    }

    db.report_failure(solution_id)?;
    println!("Failure reported. Confidence decreased.");

    Ok(())
}

/// List all patterns
fn cmd_list(
    db: &ErrorDatabase,
    language: Option<String>,
    scope: Option<String>,
    as_json: bool,
) -> Result<()> {
    let mut patterns = db.get_all_patterns()?;

    // Filter by language if specified
    if let Some(ref lang) = language {
        patterns.retain(|p| p.language.as_ref() == Some(lang));
    }

    // Filter by scope if specified
    if let Some(ref s) = scope {
        patterns.retain(|p| &p.scope == s);
    }

    if as_json {
        let output: Vec<_> = patterns
            .iter()
            .map(|p| {
                json!({
                    "id": p.id,
                    "pattern": p.pattern,
                    "scope": p.scope,
                    "language": p.language,
                })
            })
            .collect();
        println!("{}", serde_json::to_string_pretty(&output)?);
    } else {
        println!(
            "{:<50} {:<12} {:<10}",
            "PATTERN", "LANG", "SCOPE"
        );
        println!("{}", "-".repeat(75));

        for p in &patterns {
            let pattern_display = if p.pattern.len() > 47 {
                format!("{}...", &p.pattern[..47])
            } else {
                p.pattern.clone()
            };
            let lang = p.language.as_deref().unwrap_or("-");
            println!("{:<50} {:<12} {:<10}", pattern_display, lang, p.scope);
        }

        println!("\nTotal: {} patterns", patterns.len());
    }

    Ok(())
}

/// Show pattern details
fn cmd_show(db: &ErrorDatabase, pattern_id: &str, as_json: bool) -> Result<()> {
    let pattern = db
        .get_pattern(pattern_id)?
        .ok_or_else(|| anyhow::anyhow!("Pattern not found: {}", pattern_id))?;

    let solutions = db.get_solutions(pattern_id)?;

    if as_json {
        let output = json!({
            "pattern": {
                "id": pattern.id,
                "pattern": pattern.pattern,
                "scope": pattern.scope,
                "language": pattern.language,
                "framework": pattern.framework,
                "tags": pattern.tags,
                "created_at": pattern.created_at,
            },
            "solutions": solutions.iter().map(|s| json!({
                "id": s.id,
                "solution": s.solution,
                "command": s.command,
                "confidence": s.confidence,
                "success_count": s.success_count,
                "failure_count": s.failure_count,
                "last_confirmed": s.last_confirmed,
            })).collect::<Vec<_>>(),
        });
        println!("{}", serde_json::to_string_pretty(&output)?);
    } else {
        println!("Pattern: {}", pattern.pattern);
        println!("ID: {}", pattern.id);
        println!("Scope: {}", pattern.scope);
        if let Some(ref lang) = pattern.language {
            println!("Language: {}", lang);
        }
        if let Some(ref fw) = pattern.framework {
            println!("Framework: {}", fw);
        }
        if !pattern.tags.is_empty() {
            println!("Tags: {}", pattern.tags.join(", "));
        }
        println!("Created: {}", pattern.created_at);

        println!("\nSolutions ({}):", solutions.len());
        for (i, s) in solutions.iter().enumerate() {
            println!("\n  [{}] Confidence: {:.0}%", i + 1, s.confidence * 100.0);
            println!("  ID: {}", s.id);
            for line in s.solution.lines() {
                println!("    {}", line);
            }
            if let Some(ref cmd) = s.command {
                println!("  Command: {}", cmd);
            }
            println!(
                "  Success: {}, Failures: {}",
                s.success_count, s.failure_count
            );
            if let Some(ref confirmed) = s.last_confirmed {
                println!("  Last confirmed: {}", confirmed);
            }
        }
    }

    Ok(())
}

/// Show database statistics
fn cmd_stats(db: &ErrorDatabase, as_json: bool) -> Result<()> {
    let stats = db.stats()?;

    if as_json {
        println!("{}", serde_json::to_string_pretty(&stats)?);
    } else {
        println!("ERROR-DB Statistics");
        println!("{}", "=".repeat(40));
        println!("Total patterns: {}", stats.total_patterns);
        println!("Total solutions: {}", stats.total_solutions);

        if !stats.by_scope.is_empty() {
            println!("\nBy scope:");
            for (scope, count) in &stats.by_scope {
                println!("  {}: {}", scope, count);
            }
        }

        if !stats.by_language.is_empty() {
            println!("\nBy language:");
            for (lang, count) in &stats.by_language {
                println!("  {}: {}", lang, count);
            }
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_cli_help() {
        // Just ensure CLI parses correctly
        let cli = Cli::try_parse_from(["error-db", "--help"]);
        assert!(cli.is_err()); // --help causes early exit
    }

    #[test]
    fn test_database_seeds() -> Result<()> {
        let tmp = TempDir::new()?;
        let db_path = tmp.path().join("test.db");
        let db = ErrorDatabase::open(Some(&db_path))?;

        let patterns = db.get_all_patterns()?;
        assert!(!patterns.is_empty(), "Database should be seeded with patterns");

        Ok(())
    }
}
