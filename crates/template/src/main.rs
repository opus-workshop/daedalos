//! template - Project scaffolding for Daedalos
//!
//! "Never start from scratch. Build on proven foundations."
//!
//! Create new projects from templates with variable substitution.
//!
//! Commands:
//! - new <TEMPLATE> <NAME>: Create new project from template
//! - list: List available templates
//! - show <TEMPLATE>: Show template details
//! - add <PATH>: Add directory as template
//! - remove <TEMPLATE>: Remove a user template
//! - vars <TEMPLATE>: Show template variables
//! - init: Initialize template in current directory

use anyhow::{bail, Context, Result};
use clap::{Parser, Subcommand};
use std::path::PathBuf;
use template::{TemplateStore, Variables};

#[derive(Parser)]
#[command(name = "template")]
#[command(about = "Project scaffolding for Daedalos - create new projects from templates")]
#[command(version)]
#[command(after_help = r#"BUILT-IN TEMPLATES:
    bash-tool       Bash CLI tool with standard structure
    python-cli      Python CLI with Click
    node-api        Node.js REST API
    rust-cli        Rust CLI with clap
    go-service      Go microservice
    daedalos-tool   Daedalos tool structure

TEMPLATE VARIABLES:
    Templates can include placeholders:
    {{NAME}}        Project name
    {{AUTHOR}}      Author name (from git config)
    {{EMAIL}}       Author email (from git config)
    {{DATE}}        Current date (YYYY-MM-DD)
    {{YEAR}}        Current year
    {{DESCRIPTION}} Project description

EXAMPLES:
    template new bash-tool my-tool          # Create bash tool
    template new python-cli myapp           # Create Python CLI
    template new bash-tool cli --var DESC="My CLI tool"
    template add ~/my-template              # Save as template
    template list                           # Show all templates
"#)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// Create new project from template
    New {
        /// Template name to use
        template: String,

        /// Project name
        name: String,

        /// Set template variable (KEY=VALUE)
        #[arg(long = "var", value_name = "KEY=VALUE")]
        vars: Vec<String>,

        /// Don't initialize git repository
        #[arg(long)]
        no_git: bool,

        /// Overwrite existing directory
        #[arg(long)]
        force: bool,
    },

    /// List available templates
    List {
        /// Output as JSON
        #[arg(long)]
        json: bool,
    },

    /// Show template details
    Show {
        /// Template name
        template: String,
    },

    /// Add directory as user template
    Add {
        /// Path to directory
        path: PathBuf,

        /// Name for the template (default: directory name)
        name: Option<String>,
    },

    /// Remove a user template
    Remove {
        /// Template name to remove
        template: String,
    },

    /// Show template variables
    Vars {
        /// Template name
        template: String,
    },

    /// Initialize template.json in current directory
    Init {
        /// Template name (default: directory name)
        name: Option<String>,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    let store = TemplateStore::new()?;

    match cli.command {
        Some(Commands::New {
            template,
            name,
            vars,
            no_git,
            force,
        }) => cmd_new(&store, &template, &name, &vars, !no_git, force),

        Some(Commands::List { json }) => cmd_list(&store, json),

        Some(Commands::Show { template }) => cmd_show(&store, &template),

        Some(Commands::Add { path, name }) => cmd_add(&store, &path, name.as_deref()),

        Some(Commands::Remove { template }) => cmd_remove(&store, &template),

        Some(Commands::Vars { template }) => cmd_vars(&store, &template),

        Some(Commands::Init { name }) => cmd_init(name.as_deref()),

        None => cmd_list(&store, false),
    }
}

/// Create a new project from a template
fn cmd_new(
    store: &TemplateStore,
    template_name: &str,
    project_name: &str,
    custom_vars: &[String],
    init_git: bool,
    force: bool,
) -> Result<()> {
    let template = store
        .find(template_name)?
        .ok_or_else(|| anyhow::anyhow!("Template not found: {}", template_name))?;

    let dest = PathBuf::from(project_name);

    // Check if destination exists
    if dest.exists() {
        if force {
            std::fs::remove_dir_all(&dest)
                .with_context(|| format!("Failed to remove existing directory: {}", dest.display()))?;
        } else {
            bail!(
                "Directory already exists: {}\nUse --force to overwrite",
                dest.display()
            );
        }
    }

    // Set up variables
    let mut vars = Variables::new(project_name);
    vars.add_from_pairs(custom_vars);

    // Check for DESC/DESCRIPTION in custom vars
    for var in custom_vars {
        if let Some((key, value)) = var.split_once('=') {
            if key.eq_ignore_ascii_case("DESC") || key.eq_ignore_ascii_case("DESCRIPTION") {
                vars.set_description(value);
            }
        }
    }

    println!("info: Creating project from template: {}", template_name);

    template
        .instantiate(&dest, &vars, init_git)
        .with_context(|| format!("Failed to create project from template: {}", template_name))?;

    println!("success: Project created: {}", project_name);
    println!();
    println!("Next steps:");
    println!("  cd {}", project_name);

    // Show template-specific next steps
    if let Some(ref metadata) = template.metadata {
        for step in &metadata.next_steps {
            // Substitute variables in next steps too
            let step = vars.substitute(step);
            println!("  {}", step);
        }
    }

    Ok(())
}

/// List all available templates
fn cmd_list(store: &TemplateStore, json: bool) -> Result<()> {
    let templates = store.list()?;

    if json {
        let json_output: Vec<_> = templates
            .iter()
            .map(|t| {
                serde_json::json!({
                    "name": t.name,
                    "path": t.path.to_string_lossy(),
                    "builtin": t.builtin,
                    "description": t.description(),
                })
            })
            .collect();
        println!("{}", serde_json::to_string_pretty(&json_output)?);
        return Ok(());
    }

    println!("\x1b[1mAvailable Templates\x1b[0m");
    println!();

    // Separate built-in and user templates
    let builtin: Vec<_> = templates.iter().filter(|t| t.builtin).collect();
    let user: Vec<_> = templates.iter().filter(|t| !t.builtin).collect();

    if !builtin.is_empty() {
        println!("\x1b[36mBuilt-in:\x1b[0m");
        for t in &builtin {
            println!("  \x1b[32m{}\x1b[0m", t.name);
            if !t.description().is_empty() && t.description() != "No description" {
                println!("    \x1b[2m{}\x1b[0m", t.description());
            }
        }
        println!();
    }

    if !user.is_empty() {
        println!("\x1b[36mUser Templates:\x1b[0m");
        for t in &user {
            println!("  \x1b[32m{}\x1b[0m", t.name);
            if !t.description().is_empty() && t.description() != "No description" {
                println!("    \x1b[2m{}\x1b[0m", t.description());
            }
        }
    } else if builtin.is_empty() {
        println!("\x1b[2mNo templates found.\x1b[0m");
        println!("Add one with: template add /path/to/template");
    } else {
        println!("\x1b[2mNo user templates. Add one with: template add /path/to/template\x1b[0m");
    }

    Ok(())
}

/// Show details about a template
fn cmd_show(store: &TemplateStore, template_name: &str) -> Result<()> {
    let template = store
        .find(template_name)?
        .ok_or_else(|| anyhow::anyhow!("Template not found: {}", template_name))?;

    println!("\x1b[1mTemplate: {}\x1b[0m", template.name);
    println!("\x1b[2mPath: {}\x1b[0m", template.path.display());
    println!();

    // Show metadata
    if let Some(ref metadata) = template.metadata {
        println!("\x1b[36mMetadata:\x1b[0m");
        println!("{}", serde_json::to_string_pretty(metadata)?);
        println!();
    }

    // Show structure
    println!("\x1b[36mStructure:\x1b[0m");
    print_tree(&template.path, "", true)?;

    Ok(())
}

/// Print directory tree structure
fn print_tree(path: &PathBuf, prefix: &str, is_last: bool) -> Result<()> {
    let name = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or(".");

    // Skip template.json in display
    if name == "template.json" {
        return Ok(());
    }

    let connector = if prefix.is_empty() {
        ""
    } else if is_last {
        "--- "
    } else {
        "|-- "
    };

    println!("{}{}{}", prefix, connector, name);

    if path.is_dir() {
        let mut entries: Vec<_> = std::fs::read_dir(path)?
            .filter_map(|e| e.ok())
            .collect();

        // Sort entries
        entries.sort_by_key(|e| e.file_name());

        // Filter out template.json
        entries.retain(|e| e.file_name() != "template.json");

        let count = entries.len();
        for (i, entry) in entries.into_iter().enumerate() {
            let child_path = entry.path();
            let is_last_child = i == count - 1;

            let new_prefix = if prefix.is_empty() {
                String::new()
            } else if is_last {
                format!("{}    ", prefix)
            } else {
                format!("{}|   ", prefix)
            };

            print_tree(&child_path, &new_prefix, is_last_child)?;
        }
    }

    Ok(())
}

/// Add a directory as a user template
fn cmd_add(store: &TemplateStore, path: &PathBuf, name: Option<&str>) -> Result<()> {
    let abs_path = if path.is_absolute() {
        path.clone()
    } else {
        std::env::current_dir()?.join(path)
    };

    let template = store.add(&abs_path, name)?;

    println!("success: Template added: {}", template.name);

    Ok(())
}

/// Remove a user template
fn cmd_remove(store: &TemplateStore, template_name: &str) -> Result<()> {
    store.remove(template_name)?;

    println!("success: Template removed: {}", template_name);

    Ok(())
}

/// Show variables used in a template
fn cmd_vars(store: &TemplateStore, template_name: &str) -> Result<()> {
    let template = store
        .find(template_name)?
        .ok_or_else(|| anyhow::anyhow!("Template not found: {}", template_name))?;

    println!("\x1b[1mTemplate Variables: {}\x1b[0m", template.name);
    println!();

    println!("\x1b[36mStandard Variables:\x1b[0m");
    println!("  {{{{NAME}}}}        - Project name");
    println!("  {{{{AUTHOR}}}}      - Git user.name or $USER");
    println!("  {{{{EMAIL}}}}       - Git user.email");
    println!("  {{{{DATE}}}}        - Current date (YYYY-MM-DD)");
    println!("  {{{{YEAR}}}}        - Current year");
    println!("  {{{{DESCRIPTION}}}} - Project description");
    println!();

    // Find variables used in this template
    let vars = template.find_variables()?;

    println!("\x1b[36mVariables Used in Template:\x1b[0m");
    if vars.is_empty() {
        println!("  (none found)");
    } else {
        for var in &vars {
            println!("  {{{{{}}}}}", var);
        }
    }

    Ok(())
}

/// Initialize template.json in current directory
fn cmd_init(name: Option<&str>) -> Result<()> {
    let path = TemplateStore::init(name)?;

    println!("success: Template initialized: {}", path.display());
    println!("Edit template.json and use {{{{PLACEHOLDERS}}}} in your files");
    println!("Then run: template add . <name>");

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cli_parsing() {
        use clap::CommandFactory;
        Cli::command().debug_assert();
    }
}
