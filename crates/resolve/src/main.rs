//! resolve - Resolve uncertainty through context gathering
//!
//! Instead of asking the user, gather context from specs, codebase,
//! and research to make confident decisions.
//!
//! Two types of uncertainty:
//! 1. Intent: "What problem are we solving?" - may warrant asking
//! 2. Implementation: "How should we solve it?" - always resolve through context
//!
//! # Examples
//!
//! ```bash
//! resolve "should timezone be user-level or slot-level?"
//! resolve intent "user asked about reducing asking behavior"
//! resolve gather "how should we handle authentication?"
//! resolve log "Chose JWT for auth" --reasoning "API-first architecture"
//! ```

mod context;
mod decisions;
mod intent;

use anyhow::Result;
use clap::{Parser, Subcommand};
use colored::Colorize;

use crate::context::{gather_context, ConfidenceLevel};
use crate::decisions::log_decision;
use crate::intent::{analyze_intent, IntentClarity, QuestionType};

#[derive(Parser)]
#[command(name = "resolve")]
#[command(about = "Resolve uncertainty through context gathering instead of asking users")]
#[command(version)]
#[command(after_help = "\
TRIGGER:
    Use resolve when facing implementation uncertainty. Before asking the user
    \"how should I...?\", run resolve to gather context from specs, codebase,
    and decision history.

PHILOSOPHY:
    Intent questions (WHAT) may warrant asking - targeted, binary/small-set
    Implementation questions (HOW) should NEVER be asked - resolve through context

EXAMPLES:
    resolve \"should timezone be user-level or slot-level?\"
    resolve \"how should we handle authentication?\"
    resolve intent \"what problem are we solving?\"
    resolve gather \"error handling patterns\"
    resolve log \"Chose JWT\" --reasoning \"API-first architecture\"

CONFIDENCE LEVELS:
    high     Multiple consistent sources - proceed silently
    medium   Some gaps but reasonable inference - proceed, state assumption
    low      Conflicting signals or missing critical info - may need to ask

CONTEXT SOURCES:
    - Specs (spec query)           Prior decisions and principles
    - Codebase patterns (codex)    Existing patterns in the codebase
    - Project info (project)       Conventions and structure
    - Decision history             Past decisions in DECISIONS.md")]
struct Cli {
    /// Output as JSON
    #[arg(long, global = true)]
    json: bool,

    #[command(subcommand)]
    command: Option<Commands>,

    /// The question or task to resolve (shortcut for 'resolve <question>')
    #[arg(trailing_var_arg = true)]
    question: Vec<String>,
}

#[derive(Subcommand)]
enum Commands {
    /// Analyze the intent of a question (WHAT vs HOW)
    Intent {
        /// The question to analyze
        question: String,
    },

    /// Gather context for a question from all sources
    Gather {
        /// The question to gather context for
        question: String,
    },

    /// Log a decision to DECISIONS.md for future reference
    Log {
        /// The decision that was made
        decision: String,

        /// Why this decision was made
        #[arg(short, long)]
        reasoning: Option<String>,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Some(Commands::Intent { question }) => cmd_intent(&question, cli.json),
        Some(Commands::Gather { question }) => cmd_gather(&question, cli.json),
        Some(Commands::Log { decision, reasoning }) => {
            cmd_log(&decision, reasoning.as_deref(), cli.json)
        }
        None => {
            // Check if question was provided directly
            if cli.question.is_empty() {
                cmd_help()
            } else {
                let question = cli.question.join(" ");
                cmd_resolve(&question, cli.json)
            }
        }
    }
}

/// Full resolve flow: intent analysis + context gathering
fn cmd_resolve(question: &str, json: bool) -> Result<()> {
    if json {
        resolve_json(question)
    } else {
        resolve_human(question)
    }
}

/// Resolve with JSON output
fn resolve_json(question: &str) -> Result<()> {
    let intent = analyze_intent(question)?;
    let context = gather_context(question)?;

    let recommendation = if context.confidence >= ConfidenceLevel::Medium {
        "proceed_with_context"
    } else if intent.clarity == IntentClarity::Unclear {
        "clarify_intent"
    } else {
        "gather_more_context"
    };

    let output = serde_json::json!({
        "question": question,
        "intent": {
            "type": intent.question_type.as_str(),
            "clarity": intent.clarity.as_str(),
            "surface": intent.surface,
            "root": intent.root,
        },
        "context": {
            "sources_checked": context.sources_checked,
            "sources_with_results": context.sources_with_results,
            "spec_context": context.spec_context,
            "codex_context": context.codex_context,
            "project_context": context.project_context,
            "decisions_context": context.decisions_context,
        },
        "confidence": context.confidence.as_str(),
        "recommendation": recommendation,
    });

    println!("{}", serde_json::to_string_pretty(&output)?);
    Ok(())
}

/// Resolve with human-readable output
fn resolve_human(question: &str) -> Result<()> {
    println!();
    println!("{}", "RESOLVE".bold());
    println!("{}", "=".repeat(64).dimmed());
    println!();
    println!("{} {}", "Question:".bold(), question);
    println!();
    println!("{}", "-".repeat(64).dimmed());
    println!();

    // Intent phase
    let intent = analyze_intent(question)?;
    print_intent(&intent);

    println!();
    println!("{}", "-".repeat(64).dimmed());
    println!();

    // Context gathering phase
    let context = gather_context(question)?;
    print_context(&context);

    println!();
    println!("{}", "-".repeat(64).dimmed());
    println!();

    // Recommendation
    println!("{}", "RECOMMENDATION".bold());
    println!("{}", "-".repeat(14).dimmed());
    println!();

    if context.confidence >= ConfidenceLevel::Medium {
        println!("Based on gathered context, proceed with implementation.");
    } else if intent.clarity == IntentClarity::Unclear {
        println!("Intent unclear. Consider asking a {} question about", "targeted".cyan());
        println!("{} (what outcome is desired), not implementation details.", "intent".cyan());
    } else {
        println!("Limited context found. May need to ask about {}, or", "intent".cyan());
        println!("proceed with {} stated.", "assumptions".yellow());
    }
    println!();

    Ok(())
}

/// Print intent analysis in human-readable format
fn print_intent(intent: &intent::IntentAnalysis) {
    println!("{}", "INTENT ANALYSIS".bold());
    println!("{}", "-".repeat(15).dimmed());
    println!();
    println!("{} {}", "Surface:".cyan(), intent.surface);

    if let Some(root) = &intent.root {
        println!("{} {}", "Root:".cyan(), root);
    }

    let type_str = match intent.question_type {
        QuestionType::Implementation => "Implementation decision",
        QuestionType::Intent => "Intent/goal question",
        QuestionType::Task => "Task/feature request",
        QuestionType::General => "General question",
    };
    println!("{} {}", "Type:".cyan(), type_str);

    let clarity_str = match intent.clarity {
        IntentClarity::Clear => "Clear".green().to_string(),
        IntentClarity::LikelyClear => "Likely clear".green().to_string(),
        IntentClarity::NeedsDetail => "May need details".yellow().to_string(),
        IntentClarity::Unclear => "May need clarification".yellow().to_string(),
    };
    println!("{} {}", "Intent:".cyan(), clarity_str);

    if let Some(hint) = &intent.hint {
        println!();
        println!("{}", hint.dimmed());
    }
}

/// Print context gathering results in human-readable format
fn print_context(context: &context::ContextResult) {
    println!("{}", "CONTEXT GATHERING".bold());
    println!("{}", "-".repeat(17).dimmed());
    println!();

    // Specs
    println!("{}", "Checking specs...".cyan());
    if let Some(spec_ctx) = &context.spec_context {
        println!("{}", "Found spec context:".green());
        for line in spec_ctx.lines().take(15) {
            println!("  {}", line);
        }
    } else {
        println!("{}", "  No relevant specs found".dimmed());
    }
    println!();

    // Codebase patterns
    println!("{}", "Checking codebase patterns...".cyan());
    if let Some(codex_ctx) = &context.codex_context {
        println!("{}", "Found codebase patterns:".green());
        for line in codex_ctx.lines().take(10) {
            println!("  {}", line);
        }
    } else {
        println!("{}", "  No relevant patterns found".dimmed());
    }
    println!();

    // Project conventions
    println!("{}", "Checking project conventions...".cyan());
    if let Some(proj_ctx) = &context.project_context {
        println!("{}", "Project context:".green());
        for line in proj_ctx.lines().take(8) {
            println!("  {}", line);
        }
    } else {
        println!("{}", "  No project info available".dimmed());
    }
    println!();

    // Decision history
    println!("{}", "Checking decision history...".cyan());
    if let Some(dec_ctx) = &context.decisions_context {
        println!("{}", "Found past decisions:".green());
        for line in dec_ctx.lines().take(10) {
            println!("  {}", line);
        }
    } else {
        println!("{}", "  No relevant past decisions".dimmed());
    }
    println!();

    // Confidence summary
    print!("{} ", "CONFIDENCE:".bold());
    match context.confidence {
        ConfidenceLevel::High => println!("{} - Multiple consistent sources", "HIGH".green()),
        ConfidenceLevel::Medium => {
            println!("{} - Some context found, may need inference", "MEDIUM".yellow())
        }
        ConfidenceLevel::Low => {
            println!(
                "{} - Limited context, proceed with stated assumptions",
                "LOW".yellow()
            )
        }
        ConfidenceLevel::None => println!("{} - No context found, may need to ask", "NONE".red()),
    }
    println!();
    println!(
        "{}",
        format!(
            "Sources checked: {}, with results: {}",
            context.sources_checked, context.sources_with_results
        )
        .dimmed()
    );
}

/// Intent analysis command
fn cmd_intent(question: &str, json: bool) -> Result<()> {
    let intent = analyze_intent(question)?;

    if json {
        let output = serde_json::json!({
            "question": question,
            "type": intent.question_type.as_str(),
            "clarity": intent.clarity.as_str(),
            "surface": intent.surface,
            "root": intent.root,
            "hint": intent.hint,
        });
        println!("{}", serde_json::to_string_pretty(&output)?);
    } else {
        println!();
        print_intent(&intent);
        println!();
    }

    Ok(())
}

/// Context gathering command
fn cmd_gather(question: &str, json: bool) -> Result<()> {
    let context = gather_context(question)?;

    if json {
        let output = serde_json::json!({
            "question": question,
            "sources_checked": context.sources_checked,
            "sources_with_results": context.sources_with_results,
            "confidence": context.confidence.as_str(),
            "spec_context": context.spec_context,
            "codex_context": context.codex_context,
            "project_context": context.project_context,
            "decisions_context": context.decisions_context,
        });
        println!("{}", serde_json::to_string_pretty(&output)?);
    } else {
        println!();
        print_context(&context);
        println!();
    }

    Ok(())
}

/// Log decision command
fn cmd_log(decision: &str, reasoning: Option<&str>, json: bool) -> Result<()> {
    let path = log_decision(decision, reasoning)?;

    if json {
        let output = serde_json::json!({
            "logged": true,
            "decision": decision,
            "reasoning": reasoning,
            "file": path.display().to_string(),
        });
        println!("{}", serde_json::to_string_pretty(&output)?);
    } else {
        println!(
            "{} Decision logged to {}",
            "Success:".green(),
            path.display()
        );
    }

    Ok(())
}

/// Show help message
fn cmd_help() -> Result<()> {
    println!("resolve - Resolve uncertainty through context gathering");
    println!();
    println!("USAGE:");
    println!("    resolve \"<question or task>\"");
    println!("    resolve intent \"<question>\"      Intent analysis only");
    println!("    resolve gather \"<question>\"      Context gathering only");
    println!("    resolve log \"<decision>\"         Log decision to DECISIONS.md");
    println!("    resolve --json                   Output as JSON");
    println!();
    println!("EXAMPLES:");
    println!("    resolve \"should timezone be user-level or slot-level?\"");
    println!("    resolve \"how should we handle authentication?\"");
    println!("    resolve log \"Chose JWT for auth\" --reasoning \"API-first\"");
    println!();
    println!("Run 'resolve --help' for more information");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::CommandFactory;

    #[test]
    fn test_cli_parse() {
        Cli::command().debug_assert();
    }
}
