//! CLI command definitions and handlers

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use colored::*;

use crate::find::{find_all_specs, find_project_root, find_spec};
use crate::output::{output_spec, output_spec_section};
use crate::spec::Spec;
use crate::template::create_spec_from_template;
use crate::validate::{validate_all_specs, ValidationResult};

/// spec - Rich specification management for Daedalos
///
/// Manages structured specs that capture not just WHAT but WHY.
#[derive(Parser)]
#[command(name = "spec")]
#[command(version = "1.0.0")]
#[command(about = "Rich specification management - capture intent, decisions, and anti-patterns")]
#[command(after_help = "\
TRIGGER:
    Use spec before starting work on ANY component. Load intent and anti-patterns
    first to avoid rediscovering decisions or repeating known mistakes.

EXAMPLES:
    spec show loop                 Show full spec for loop tool
    spec show undo -s intent       Show just the intent section
    spec query \"why sqlite\"        Search for design decisions
    spec context \"fix auth bug\"    Get relevant context for a task
    spec list --missing            Show components without specs
    spec validate                  Check all specs for errors
    spec new my-tool               Create new spec from template

SECTIONS:
    intent        WHY this exists - the problem being solved
    constraints   Hard limits (performance, compatibility, etc.)
    interface     Commands, arguments, outputs
    examples      Usage examples and expected behavior
    decisions     Design choices and WHY (not just WHAT)
    anti_patterns Things to AVOID and why they're bad")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Display spec for a component
    #[command(about = "Display spec for a component")]
    Show {
        /// Component name
        component: String,

        /// Show only a specific section (intent, constraints, interface, examples, decisions, anti_patterns)
        #[arg(short, long)]
        section: Option<String>,

        /// Output as JSON
        #[arg(long, default_value = "false")]
        json: bool,
    },

    /// Search across all specs
    #[command(about = "Search across all specs semantically")]
    Query {
        /// Search query (e.g., "why sqlite")
        query: String,
    },

    /// List all specs in project
    #[command(about = "List all specs in the project")]
    List {
        /// Show components without specs
        #[arg(long, default_value = "false")]
        missing: bool,

        /// Show specs older than implementation
        #[arg(long, default_value = "false")]
        stale: bool,
    },

    /// Get relevant specs for a task
    #[command(about = "Get relevant spec sections for a task")]
    Context {
        /// Task description (e.g., "fix undo restore")
        task: String,
    },

    /// Validate spec format
    #[command(about = "Validate spec format and completeness")]
    Validate {
        /// Path to validate (default: current directory)
        path: Option<String>,
    },

    /// Create new spec from template
    #[command(about = "Create a new spec from template")]
    New {
        /// Component name
        name: String,

        /// Type: tool, library, service, doc
        #[arg(short = 't', long = "type", default_value = "tool")]
        type_: String,
    },
}

/// Show a spec or section of a spec
pub fn cmd_show(component: &str, section: Option<&str>, json: bool) -> Result<()> {
    let project_root = find_project_root()?;
    let spec_path = find_spec(&project_root, component)?
        .with_context(|| format!("Spec not found: {}", component))?;

    let spec = Spec::load(&spec_path)?;

    match section {
        Some(section_name) => output_spec_section(&spec, section_name, json),
        None => output_spec(&spec, json),
    }
}

/// Query specs for relevant content
pub fn cmd_query(query: &str) -> Result<()> {
    let project_root = find_project_root()?;
    let specs = find_all_specs(&project_root)?;

    println!(
        "{} {}",
        "Searching specs for:".bold(),
        query.cyan()
    );
    println!();

    let query_lower = query.to_lowercase();
    let mut found = false;

    for spec_path in specs {
        let spec = match Spec::load(&spec_path) {
            Ok(s) => s,
            Err(_) => continue,
        };

        let mut matches = Vec::new();

        // Search in intent
        if let Some(ref intent) = spec.intent {
            if intent.to_lowercase().contains(&query_lower) {
                matches.push("intent");
            }
        }

        // Search in decisions
        if let Some(ref decisions) = spec.decisions {
            let decisions_str = serde_json::to_string(decisions).unwrap_or_default();
            if decisions_str.to_lowercase().contains(&query_lower) {
                matches.push("decisions");
            }
        }

        // Search in anti_patterns
        if let Some(ref anti_patterns) = spec.anti_patterns {
            let anti_str = serde_json::to_string(anti_patterns).unwrap_or_default();
            if anti_str.to_lowercase().contains(&query_lower) {
                matches.push("anti_patterns");
            }
        }

        // Search in constraints
        if let Some(ref constraints) = spec.constraints {
            let constraints_str = serde_yaml::to_string(constraints).unwrap_or_default();
            if constraints_str.to_lowercase().contains(&query_lower) {
                matches.push("constraints");
            }
        }

        // Search in examples
        if let Some(ref examples) = spec.examples {
            let examples_str = serde_json::to_string(examples).unwrap_or_default();
            if examples_str.to_lowercase().contains(&query_lower) {
                matches.push("examples");
            }
        }

        if !matches.is_empty() {
            found = true;
            println!(
                "{} {}",
                spec.name.green(),
                format!("({})", matches.join(", ")).dimmed()
            );

            // Show relevant excerpts
            for section in &matches {
                let content = match *section {
                    "intent" => spec.intent.as_deref().unwrap_or(""),
                    "decisions" => {
                        // Show a brief excerpt
                        if let Some(ref decisions) = spec.decisions {
                            for decision in decisions {
                                if decision.choice.to_lowercase().contains(&query_lower)
                                    || decision.why.to_lowercase().contains(&query_lower) {
                                    println!("  - {}", decision.choice.dimmed());
                                    break;
                                }
                            }
                        }
                        ""
                    }
                    "anti_patterns" => {
                        if let Some(ref anti_patterns) = spec.anti_patterns {
                            for ap in anti_patterns {
                                if ap.pattern.to_lowercase().contains(&query_lower)
                                    || ap.why_bad.to_lowercase().contains(&query_lower) {
                                    println!("  - {}", ap.pattern.dimmed());
                                    break;
                                }
                            }
                        }
                        ""
                    }
                    _ => "",
                };

                // For intent, show lines containing the query
                if *section == "intent" && !content.is_empty() {
                    for line in content.lines().take(3) {
                        if line.to_lowercase().contains(&query_lower) {
                            println!("  {}", line.dimmed());
                        }
                    }
                }
            }
            println!();
        }
    }

    if !found {
        println!("{}", format!("No matches found for: {}", query).yellow());
    }

    Ok(())
}

/// List all specs
pub fn cmd_list(show_missing: bool, _show_stale: bool) -> Result<()> {
    let project_root = find_project_root()?;
    let specs = find_all_specs(&project_root)?;

    println!(
        "{}",
        format!("Specs in {}", project_root.display()).bold()
    );
    println!();

    if specs.is_empty() {
        println!("{}", "No specs found".yellow());
        return Ok(());
    }

    for spec_path in &specs {
        let spec = match Spec::load(spec_path) {
            Ok(s) => s,
            Err(_) => continue,
        };

        let rel_path = spec_path
            .strip_prefix(&project_root)
            .unwrap_or(spec_path);

        println!(
            "  {:<20} {}",
            spec.name.cyan(),
            rel_path.display().to_string().dimmed()
        );
    }

    // Show missing if requested
    if show_missing {
        println!();
        println!("{}", "Components without specs:".bold());

        // Look for tool directories without specs
        let tools_dir = project_root.join("daedalos-tools");
        if tools_dir.exists() {
            if let Ok(entries) = std::fs::read_dir(&tools_dir) {
                for entry in entries.flatten() {
                    if entry.path().is_dir() {
                        let name = entry.file_name().to_string_lossy().to_string();
                        let spec_file = entry.path().join(format!("{}.spec.yaml", name));
                        if !spec_file.exists() && name != "daedalos-tools" {
                            println!("  {}", name.dimmed());
                        }
                    }
                }
            }
        }
    }

    Ok(())
}

/// Get context for a task
pub fn cmd_context(task: &str) -> Result<()> {
    let project_root = find_project_root()?;
    let specs = find_all_specs(&project_root)?;

    let task_lower = task.to_lowercase();
    let task_words: Vec<&str> = task_lower.split_whitespace().collect();

    for spec_path in specs {
        let spec = match Spec::load(&spec_path) {
            Ok(s) => s,
            Err(_) => continue,
        };

        let name_lower = spec.name.to_lowercase();
        let mut relevance = 0;

        // Check if component name appears in task
        if task_lower.contains(&name_lower) {
            relevance = 100;
        }

        // Check if task keywords appear in intent
        if let Some(ref intent) = spec.intent {
            let intent_lower = intent.to_lowercase();
            for word in &task_words {
                if word.len() > 3 && intent_lower.contains(*word) {
                    relevance += 10;
                }
            }
        }

        if relevance > 20 {
            println!("---");
            println!("# {} (relevance: {})", spec.name, relevance);
            println!();

            // Output focused context
            if let Some(ref intent) = spec.intent {
                println!("## Intent");
                println!("{}", intent);
                println!();
            }

            // Get interface
            if let Some(ref interface) = spec.interface {
                println!("## Interface");
                let yaml = serde_yaml::to_string(interface)?;
                // Only show first 30 lines
                for line in yaml.lines().take(30) {
                    println!("{}", line);
                }
                println!();
            }

            // Always include anti-patterns
            if let Some(ref anti_patterns) = spec.anti_patterns {
                println!("## Anti-patterns (AVOID)");
                for ap in anti_patterns {
                    println!("- {}", ap.pattern);
                    println!("  {}", ap.why_bad.lines().next().unwrap_or(""));
                }
                println!();
            }

            // Include relevant decisions
            if let Some(ref decisions) = spec.decisions {
                println!("## Relevant Decisions");
                for decision in decisions.iter().take(3) {
                    println!("- {}", decision.choice);
                }
            }
        }
    }

    Ok(())
}

/// Validate specs
pub fn cmd_validate(path: Option<&str>) -> Result<()> {
    let project_root = find_project_root()?;
    let validate_path = path
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| project_root.clone());

    println!(
        "{}",
        format!("Validating specs in: {}", validate_path.display()).bold()
    );
    println!();

    let results = validate_all_specs(&validate_path)?;

    let mut errors = 0;
    let mut warnings = 0;

    for result in &results {
        match result {
            ValidationResult::Ok { path } => {
                let rel_path = path.strip_prefix(&project_root).unwrap_or(path);
                println!("{} {}", "OK".green(), rel_path.display());
            }
            ValidationResult::Error { path, messages } => {
                let rel_path = path.strip_prefix(&project_root).unwrap_or(path);
                println!("{} {}", "ERROR".red(), rel_path.display());
                for msg in messages {
                    println!("  - {}", msg);
                }
                errors += 1;
            }
            ValidationResult::Warning { path, messages } => {
                let rel_path = path.strip_prefix(&project_root).unwrap_or(path);
                println!("{} {}", "WARN".yellow(), rel_path.display());
                for msg in messages {
                    println!("  - {}", msg);
                }
                warnings += 1;
            }
        }
    }

    println!();
    if errors > 0 {
        println!("{}", format!("{} spec(s) have errors", errors).red());
        std::process::exit(3);
    } else if warnings > 0 {
        println!("{}", format!("{} spec(s) have warnings", warnings).yellow());
    } else {
        println!("{}", "All specs valid".green());
    }

    Ok(())
}

/// Create a new spec from template
pub fn cmd_new(name: &str, type_: &str) -> Result<()> {
    let project_root = find_project_root()?;
    let output_path = create_spec_from_template(&project_root, name, type_)?;

    println!("{} {}", "Created:".green(), output_path.display());
    println!(
        "{}",
        "Edit the spec to fill in intent, constraints, and decisions".dimmed()
    );

    Ok(())
}
