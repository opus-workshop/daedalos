//! Output formatting for evolve results

use anyhow::Result;
use colored::Colorize;
use std::path::Path;

use crate::analyze::Analysis;
use crate::gaps::Gaps;
use crate::intent::Intent;
use crate::path::EvolutionPath;

/// Print full evolution analysis with colors
pub fn print_full_evolution(
    path: &Path,
    intent: &Intent,
    analysis: &Analysis,
    gaps: &Gaps,
    evolution: &EvolutionPath,
) -> Result<()> {
    println!();
    println!("{}", "══════════════════════════════════════════════════════════════".bold());
    println!("{}", "                        EVOLVE                                 ".bold());
    println!("{}", "══════════════════════════════════════════════════════════════".bold());
    println!();
    println!("{} {}", "Target:".bold(), path.display());
    println!();
    println!("{}", "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━".dimmed());
    println!();

    print_intent_phase(intent)?;

    println!();
    println!("{}", "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━".dimmed());
    println!();

    print_analyze_phase(analysis)?;

    println!();
    println!("{}", "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━".dimmed());
    println!();

    print_gaps_phase(gaps)?;

    println!();
    println!("{}", "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━".dimmed());
    println!();

    print_path_phase(evolution)?;

    println!();

    Ok(())
}

/// Print intent phase
pub fn print_intent_phase(intent: &Intent) -> Result<()> {
    println!("{}", "PHASE 1: UNDERSTAND INTENT".bold());
    println!("{}", "\"What is this code trying to become?\"".dimmed());
    println!();

    // Spec info
    if let Some(ref spec_file) = intent.spec_file {
        println!("{} {}", "From spec:".green(), spec_file.display());
        if let Some(ref spec_intent) = intent.spec_intent {
            for line in spec_intent.lines().take(5) {
                println!("  {}", line);
            }
        }
        println!();

        if !intent.success_criteria.is_empty() {
            println!("{}", "Success criteria:".dimmed());
            for criterion in intent.success_criteria.iter().take(3) {
                println!("  {} {}", "•".dimmed(), criterion);
            }
            println!();
        }
    }

    // Git info
    if let Some(ref first) = intent.first_commit {
        println!("{} {}", "First commit:".green(), first);
    }
    if !intent.recent_commits.is_empty() {
        println!("{}", "Recent changes:".green());
        for commit in intent.recent_commits.iter().take(3) {
            println!("  {}", commit);
        }
        println!();
    }

    // Doc info
    if let Some(ref doc) = intent.doc_content {
        println!("{}", "From docs:".green());
        for line in doc.lines().take(3) {
            println!("  {}", line);
        }
        println!();
    }

    // Test info
    if !intent.test_descriptions.is_empty() {
        println!("{}", "From tests:".green());
        for desc in intent.test_descriptions.iter().take(3) {
            println!("  {}", desc);
        }
        println!();
    }

    // Code comments
    if let Some(ref comments) = intent.code_comments {
        println!("{}", "From code comments:".green());
        for line in comments.lines().take(3) {
            println!("  {}", line);
        }
        println!();
    }

    if intent.sources_count == 0 {
        println!("{}", "No intent sources found.".yellow());
        println!("{}", "Consider adding a .spec.yaml or README.".dimmed());
    } else {
        println!("{}", format!("Intent sources found: {}", intent.sources_count).dimmed());
    }

    Ok(())
}

/// Print analyze phase
pub fn print_analyze_phase(analysis: &Analysis) -> Result<()> {
    println!("{}", "PHASE 2: ANALYZE CURRENT STATE".bold());
    println!("{}", "\"How well does code serve the intent?\"".dimmed());
    println!();

    println!("{} {}", "Files in scope:".cyan(), analysis.file_count);
    println!("{} {}", "Lines of code:".cyan(), analysis.line_count);
    println!();

    if !analysis.todos.is_empty() {
        println!("{}", format!("Found {} TODO/FIXME comments", analysis.todos.len()).yellow());
        for todo in analysis.todos.iter().take(3) {
            println!("  {} {}", format!("{}:{}", todo.file, todo.line).dimmed(), todo.content);
        }
        println!();
    }

    if !analysis.large_files.is_empty() {
        for large in &analysis.large_files {
            println!("{}", format!("Large file ({} lines) - consider splitting", large.lines).yellow());
            println!("  {}", large.path);
        }
        println!();
    }

    println!(
        "{}",
        format!("Issues: {}, Warnings: {}", analysis.issues, analysis.warnings).dimmed()
    );

    Ok(())
}

/// Print gaps phase
pub fn print_gaps_phase(gaps: &Gaps) -> Result<()> {
    println!("{}", "PHASE 3: IDENTIFY MISSING PIECES".bold());
    println!("{}", "\"What's not there that should be?\"".dimmed());
    println!();

    if !gaps.spec_interface.is_empty() {
        println!("{}", "From spec interface (verify implementation):".cyan());
        for cmd in gaps.spec_interface.iter().take(5) {
            println!("  {}", cmd);
        }
        println!();
    }

    if !gaps.integration_points.is_empty() {
        println!("{}", "Integration points (from spec):".cyan());
        for point in &gaps.integration_points {
            println!("  {} {}", "•".dimmed(), point);
        }
        println!();
    }

    if gaps.missing_tests {
        println!("{}", "Missing: Tests".yellow());
    }
    if gaps.missing_docs {
        println!("{}", "Missing: README documentation".yellow());
    }
    if gaps.missing_spec {
        println!("{}", "Missing: Spec file (.spec.yaml)".yellow());
    }

    println!();
    println!("{}", format!("Gaps identified: {}", gaps.count).dimmed());

    Ok(())
}

/// Print evolution path phase
pub fn print_path_phase(evolution: &EvolutionPath) -> Result<()> {
    println!("{}", "PHASE 4: EVOLUTION PATH".bold());
    println!("{}", "\"How to grow toward full intent\"".dimmed());
    println!();

    if !evolution.priority_fix.is_empty() {
        println!("{}", "Priority 1: Fix before extend".magenta());
        for suggestion in &evolution.priority_fix {
            println!("  {} {}", "•".dimmed(), suggestion.action);
            if !suggestion.reason.is_empty() {
                println!("    {}", suggestion.reason.dimmed());
            }
        }
        println!();
    }

    if !evolution.priority_fundamentals.is_empty() {
        println!("{}", "Priority 2: Missing fundamentals".magenta());
        for suggestion in &evolution.priority_fundamentals {
            println!("  {} {}", "•".dimmed(), suggestion.action);
            if !suggestion.reason.is_empty() {
                println!("    {}", suggestion.reason.dimmed());
            }
        }
        println!();
    }

    if !evolution.priority_extend.is_empty() {
        println!("{}", "Priority 3: Extend toward intent".magenta());
        for suggestion in &evolution.priority_extend {
            println!("  {} {}", "•".dimmed(), suggestion.action);
            if !suggestion.reason.is_empty() {
                println!("    {}", suggestion.reason.dimmed());
            }
        }
        println!();
    }

    println!("{}", "Use 'loop' to implement each suggestion iteratively".dimmed());

    Ok(())
}

/// Print JSON output for full evolution
pub fn print_json_evolution(
    path: &Path,
    intent: &Intent,
    analysis: &Analysis,
    gaps: &Gaps,
    evolution: &EvolutionPath,
) -> Result<()> {
    let output = serde_json::json!({
        "path": path.to_string_lossy(),
        "name": path.file_name().map(|n| n.to_string_lossy()).unwrap_or_default(),
        "intent": intent,
        "analysis": analysis,
        "gaps": gaps,
        "evolution_path": evolution,
        "evolution_needed": gaps.count > 0 || analysis.issues > 0 || analysis.warnings > 0
    });

    println!("{}", serde_json::to_string_pretty(&output)?);

    Ok(())
}
