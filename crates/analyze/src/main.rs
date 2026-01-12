//! analyze - Analyze for gaps and fill them
//!
//! "Find what's missing. Write what's needed. Verify it works."
//!
//! The analyze tool orchestrates the full gap-filling workflow:
//! 1. Run `evolve gaps` to identify missing pieces
//! 2. Spawn an agent with gap context to write tests/code
//! 3. Loop until `verify` passes
//!
//! This is the "analyze for gaps and write missing tests around them" workflow
//! that transforms gap detection into gap resolution.

mod orchestrator;

use anyhow::Result;
use clap::{Parser, Subcommand};
use colored::Colorize;

use orchestrator::AnalyzeOrchestrator;

/// analyze - Analyze for gaps and fill them
#[derive(Parser)]
#[command(name = "analyze")]
#[command(version)]
#[command(about = "Analyze for gaps and fill them - find missing tests, write them, verify")]
#[command(after_help = "\
TRIGGER:
    Use analyze when you want to automatically improve test coverage or
    fill implementation gaps. It orchestrates: evolve gaps → agent → loop.

EXAMPLES:
    analyze tests                    Analyze and fill test gaps
    analyze tests src/auth/          Focus on specific directory
    analyze tests --dry-run          Show gaps without filling them
    analyze gaps                     Just show gaps (alias for evolve gaps)
    analyze status                   Show active analysis runs

WORKFLOW:
    1. Runs `evolve gaps` to identify untested code paths
    2. Generates a targeted prompt from gap analysis
    3. Spawns agent to write tests for those specific gaps
    4. Loops until `verify` passes

PHILOSOPHY:
    Gap detection without resolution is incomplete.
    Daedalos orchestrates, AI generates.
    Loop until verified.")]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// Analyze and fill test gaps
    Tests {
        /// Path to analyze (default: current directory)
        path: Option<String>,

        /// Just show gaps without filling them
        #[arg(long)]
        dry_run: bool,

        /// Maximum iterations for the loop
        #[arg(short = 'n', long, default_value = "10")]
        max_iterations: u32,

        /// Verification command (default: auto-detect via verify)
        #[arg(long)]
        promise: Option<String>,

        /// Output as JSON
        #[arg(long)]
        json: bool,
    },

    /// Show gaps without filling (alias for evolve gaps)
    Gaps {
        /// Path to analyze
        path: Option<String>,

        /// Output as JSON
        #[arg(long)]
        json: bool,
    },

    /// Show active analysis runs
    Status {
        /// Output as JSON
        #[arg(long)]
        json: bool,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Some(Commands::Tests {
            path,
            dry_run,
            max_iterations,
            promise,
            json,
        }) => {
            cmd_tests(path, dry_run, max_iterations, promise, json).await
        }
        Some(Commands::Gaps { path, json }) => cmd_gaps(path, json).await,
        Some(Commands::Status { json }) => cmd_status(json),
        None => {
            // Default: show help
            println!("{}", "analyze - Analyze for gaps and fill them".bold());
            println!();
            println!("Usage: analyze <command>");
            println!();
            println!("Commands:");
            println!("    {}     Analyze and fill test gaps", "tests".cyan());
            println!("    {}      Show gaps without filling", "gaps".cyan());
            println!("    {}    Show active analysis runs", "status".cyan());
            println!();
            println!("Run {} for more information", "analyze --help".bold());
            Ok(())
        }
    }
}

async fn cmd_tests(
    path: Option<String>,
    dry_run: bool,
    max_iterations: u32,
    promise: Option<String>,
    json: bool,
) -> Result<()> {
    let path = path.unwrap_or_else(|| ".".to_string());
    let orchestrator = AnalyzeOrchestrator::new(&path)?;

    // Step 1: Get gaps
    if !json {
        println!("{}", "Step 1: Analyzing gaps...".bold());
    }

    let gaps = orchestrator.get_gaps()?;

    if json {
        println!("{}", serde_json::to_string_pretty(&gaps)?);
        if dry_run {
            return Ok(());
        }
    } else {
        println!();
        print_gaps_summary(&gaps);
    }

    if gaps.test_gaps.is_empty() && gaps.coverage_gaps.is_empty() {
        if !json {
            println!();
            println!("{} No test gaps found!", "ok".green());
        }
        return Ok(());
    }

    if dry_run {
        if !json {
            println!();
            println!(
                "{} Dry run - would fill {} test gaps",
                "info:".blue(),
                gaps.test_gaps.len() + gaps.coverage_gaps.len()
            );
        }
        return Ok(());
    }

    // Step 2: Generate prompt
    if !json {
        println!();
        println!("{}", "Step 2: Generating prompt from gaps...".bold());
    }

    let prompt = orchestrator.generate_prompt(&gaps)?;

    if !json {
        println!("Prompt preview:");
        let preview: String = prompt.chars().take(200).collect();
        println!("{}", format!("  {}...", preview).dimmed());
    }

    // Step 3: Start loop with agent
    if !json {
        println!();
        println!("{}", "Step 3: Starting loop...".bold());
    }

    let promise = promise.unwrap_or_else(|| "verify --quick".to_string());

    let result = orchestrator
        .run_loop(&prompt, &promise, max_iterations)
        .await?;

    if json {
        println!("{}", serde_json::to_string_pretty(&result)?);
    } else {
        println!();
        if result.success {
            println!(
                "{} Test gaps filled in {} iterations!",
                "ok".green(),
                result.iterations
            );
        } else {
            println!(
                "{}  Loop completed after {} iterations (max: {})",
                "!".yellow(),
                result.iterations,
                max_iterations
            );
            if let Some(ref err) = result.error {
                println!("Last error: {}", err);
            }
        }
    }

    Ok(())
}

async fn cmd_gaps(path: Option<String>, json: bool) -> Result<()> {
    let path = path.unwrap_or_else(|| ".".to_string());
    let orchestrator = AnalyzeOrchestrator::new(&path)?;

    let gaps = orchestrator.get_gaps()?;

    if json {
        println!("{}", serde_json::to_string_pretty(&gaps)?);
    } else {
        print_gaps_summary(&gaps);
    }

    Ok(())
}

fn cmd_status(_json: bool) -> Result<()> {
    // TODO: Track active analysis runs
    println!("{}", "No active analysis runs".dimmed());
    Ok(())
}

fn print_gaps_summary(gaps: &orchestrator::GapAnalysis) {
    if gaps.test_gaps.is_empty() && gaps.coverage_gaps.is_empty() {
        println!("{} No test gaps found!", "ok".green());
        return;
    }

    println!("{}", "Test Gaps Found:".bold());
    println!();

    for gap in &gaps.test_gaps {
        println!(
            "  {} {} ({})",
            "!".yellow(),
            gap.file,
            gap.reason.dimmed()
        );
    }

    if !gaps.coverage_gaps.is_empty() {
        println!();
        println!("{}", "Coverage Gaps:".bold());
        for gap in &gaps.coverage_gaps {
            println!("  {} {} - {}", "!".yellow(), gap.file, gap.description);
        }
    }

    println!();
    println!(
        "Total: {} gaps",
        gaps.test_gaps.len() + gaps.coverage_gaps.len()
    );
}
