//! CLI command definitions and handlers

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use std::path::PathBuf;

use crate::detect::{detect_project_type, ProjectType};
use crate::pipeline::load_pipeline;
use crate::runner::{run_verification, VerificationResult};

/// verify - Universal project verification
///
/// "One command. All checks. No excuses."
#[derive(Parser)]
#[command(name = "verify")]
#[command(version = "1.0.0")]
#[command(about = "Universal project verification - one command, all checks")]
#[command(after_help = "\
TRIGGER:
    Run verify before declaring work complete, before commits, before moving on.
    Auto-detects project type and runs appropriate checks.

EXAMPLES:
    verify                     Run all checks for current project
    verify --quick             Fast checks only (lint + types)
    verify --staged            Only check git staged files
    verify --fix               Auto-fix issues where possible
    verify --step lint         Run only the lint step
    verify --skip test         Skip the test step
    verify status              Show detected project type and pipeline
    verify pipelines           List available verification pipelines
    verify init                Create .daedalos/verify.yaml config

SUPPORTED PROJECTS:
    rust, python, typescript, javascript, go, swift, shell

PHILOSOPHY:
    Verification is not optional. Never claim \"done\" without evidence.")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Commands>,

    /// Path to verify (default: current directory)
    #[arg(global = true)]
    pub path: Option<PathBuf>,

    /// Fast checks only (lint + types)
    #[arg(short, long, global = true)]
    pub quick: bool,

    /// Only check git staged files
    #[arg(long, global = true)]
    pub staged: bool,

    /// Auto-fix issues where possible
    #[arg(long, global = true)]
    pub fix: bool,

    /// Use specific pipeline
    #[arg(long, global = true)]
    pub pipeline: Option<String>,

    /// Run only specific step
    #[arg(long, global = true)]
    pub step: Option<String>,

    /// Skip specific step (can be repeated)
    #[arg(long, global = true)]
    pub skip: Vec<String>,

    /// Output as JSON
    #[arg(long, global = true)]
    pub json: bool,

    /// Minimal output
    #[arg(long, global = true)]
    pub quiet: bool,

    /// Verbose output
    #[arg(short, long, global = true)]
    pub verbose: bool,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Show verification status
    #[command(about = "Show verification status and detected pipeline")]
    Status,

    /// List available pipelines
    #[command(about = "List available verification pipelines")]
    Pipelines,

    /// Create project-specific config
    #[command(about = "Create .daedalos/verify.yaml config file")]
    Init,
}

/// Run the CLI
pub async fn run(cli: Cli) -> Result<()> {
    let path = cli
        .path
        .clone()
        .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));

    match cli.command {
        Some(Commands::Status) => cmd_status(&path, cli.json).await,
        Some(Commands::Pipelines) => cmd_pipelines(cli.json).await,
        Some(Commands::Init) => cmd_init(&path).await,
        None => {
            // Default: run verification
            cmd_verify(cli, &path).await
        }
    }
}

/// Run verification
async fn cmd_verify(cli: Cli, path: &PathBuf) -> Result<()> {
    let project_type = if let Some(ref pipeline_name) = cli.pipeline {
        ProjectType::from_name(pipeline_name)
    } else {
        detect_project_type(path)
    };

    if project_type == ProjectType::Unknown {
        if cli.json {
            println!(
                "{}",
                serde_json::json!({
                    "success": false,
                    "error": "Could not detect project type. Use --pipeline to specify.",
                    "duration_ms": 0,
                    "steps": []
                })
            );
        } else if !cli.quiet {
            eprintln!("ERROR: Could not detect project type. Use --pipeline to specify.");
        }
        std::process::exit(2);
    }

    let pipeline = load_pipeline(&project_type)?;

    if !cli.quiet && !cli.json && cli.verbose {
        println!("Detected project type: {}", project_type.name());
        println!("Using pipeline: {}", project_type.name());
    }

    let result = run_verification(
        &pipeline,
        path,
        cli.quick,
        cli.staged,
        cli.fix,
        cli.step.as_deref(),
        &cli.skip,
        cli.json,
        cli.quiet,
        cli.verbose,
    )
    .await?;

    // Save status
    save_verification_status(&project_type, result.success, path)?;

    if cli.json {
        print_json_result(&result);
    } else if !cli.quiet {
        print_summary(&result);
    }

    if result.success {
        Ok(())
    } else {
        std::process::exit(1);
    }
}

/// Show verification status
async fn cmd_status(path: &PathBuf, json: bool) -> Result<()> {
    let project_type = detect_project_type(path);

    if project_type == ProjectType::Unknown {
        if json {
            println!(
                "{}",
                serde_json::json!({
                    "project_type": "unknown",
                    "error": "Could not detect project type"
                })
            );
        } else {
            println!("Could not detect project type");
        }
        return Ok(());
    }

    let pipeline = load_pipeline(&project_type)?;
    let last_status = load_verification_status(path);

    if json {
        let steps: Vec<_> = pipeline
            .steps
            .iter()
            .map(|s| {
                serde_json::json!({
                    "name": s.name,
                    "quick": s.quick,
                    "timeout": s.timeout
                })
            })
            .collect();

        println!(
            "{}",
            serde_json::json!({
                "project_type": project_type.name(),
                "pipeline": project_type.name(),
                "last_run": last_status.as_ref().map(|s| &s.timestamp),
                "last_result": last_status.as_ref().map(|s| if s.passed { "passed" } else { "failed" }),
                "steps": steps
            })
        );
    } else {
        println!("Verify Status");
        println!("----------------------------");
        println!("Project type: {}", project_type.name());
        println!("Pipeline: {}", project_type.name());

        if let Some(status) = last_status {
            println!("Last run: {}", status.timestamp);
            println!(
                "Last result: {}",
                if status.passed { "passed" } else { "failed" }
            );
        } else {
            println!("Last run: never");
            println!("Last result: unknown");
        }

        println!();
        println!("Available steps:");
        for step in &pipeline.steps {
            if step.quick {
                println!("  - {} (quick)", step.name);
            } else {
                println!("  - {}", step.name);
            }
        }
    }

    Ok(())
}

/// List available pipelines
async fn cmd_pipelines(json: bool) -> Result<()> {
    let pipelines = vec![
        ("rust", "Rust/Cargo projects"),
        ("python", "Python projects"),
        ("typescript", "TypeScript/Node.js projects"),
        ("javascript", "JavaScript/Node.js projects"),
        ("go", "Go projects"),
        ("swift", "Swift and Xcode projects"),
        ("shell", "Shell script projects"),
    ];

    if json {
        let items: Vec<_> = pipelines
            .iter()
            .map(|(name, desc)| {
                serde_json::json!({
                    "name": name,
                    "description": desc
                })
            })
            .collect();
        println!("{}", serde_json::to_string_pretty(&items)?);
    } else {
        println!("Available Pipelines");
        println!("----------------------------");
        for (name, desc) in pipelines {
            println!("  {:<12} {}", name, desc);
        }
    }

    Ok(())
}

/// Create project config
async fn cmd_init(path: &PathBuf) -> Result<()> {
    let config_dir = path.join(".daedalos");
    let config_file = config_dir.join("verify.yaml");

    if config_file.exists() {
        eprintln!("Config already exists: {}", config_file.display());
        std::process::exit(1);
    }

    let project_type = detect_project_type(path);

    std::fs::create_dir_all(&config_dir).context("Failed to create .daedalos directory")?;

    let content = format!(
        r#"# verify configuration
# Generated for: {}

# Override pipeline (optional)
# pipeline: {}

# Custom steps (optional)
# steps:
#   - name: lint
#     command: your-lint-command
#     quick: true
#
#   - name: test
#     command: your-test-command
#     timeout: 300
"#,
        project_type.name(),
        project_type.name()
    );

    std::fs::write(&config_file, content).context("Failed to write config file")?;

    println!("Created {}", config_file.display());
    println!("Edit this file to customize verification for your project.");

    Ok(())
}

/// Print JSON result
fn print_json_result(result: &VerificationResult) {
    let steps: Vec<_> = result
        .steps
        .iter()
        .map(|s| {
            serde_json::json!({
                "name": s.name,
                "success": s.success,
                "duration_ms": s.duration_ms,
                "output": s.output
            })
        })
        .collect();

    println!(
        "{}",
        serde_json::json!({
            "success": result.success,
            "duration_ms": result.duration_ms,
            "steps": steps
        })
    );
}

/// Print summary
fn print_summary(result: &VerificationResult) {
    println!("----------------------------");
    let duration_str = format_duration(result.duration_ms);
    if result.success {
        println!("Total: {} All checks passed", duration_str);
    } else {
        let error_count = result.steps.iter().filter(|s| !s.success).count();
        println!("Total: {} {} error(s)", duration_str, error_count);
    }
}

/// Format duration in milliseconds to human-readable string
fn format_duration(ms: u64) -> String {
    if ms < 1000 {
        format!("{}ms", ms)
    } else if ms < 60000 {
        format!("{:.1}s", ms as f64 / 1000.0)
    } else {
        let mins = ms / 60000;
        let secs = (ms % 60000) / 1000;
        format!("{}m{}s", mins, secs)
    }
}

/// Verification status saved to disk
#[derive(serde::Serialize, serde::Deserialize)]
struct VerificationStatus {
    project_path: String,
    project_type: String,
    passed: bool,
    timestamp: String,
}

/// Get state directory
fn get_state_dir() -> PathBuf {
    dirs::data_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("daedalos")
        .join("verify")
}

/// Save verification status
fn save_verification_status(
    project_type: &ProjectType,
    passed: bool,
    path: &PathBuf,
) -> Result<()> {
    let state_dir = get_state_dir();
    std::fs::create_dir_all(&state_dir)?;

    let status = VerificationStatus {
        project_path: path
            .canonicalize()
            .unwrap_or_else(|_| path.clone())
            .to_string_lossy()
            .to_string(),
        project_type: project_type.name().to_string(),
        passed,
        timestamp: chrono::Utc::now().to_rfc3339(),
    };

    let status_file = state_dir.join("last_status.json");
    let content = serde_json::to_string_pretty(&status)?;
    std::fs::write(status_file, content)?;

    Ok(())
}

/// Load verification status
fn load_verification_status(path: &PathBuf) -> Option<VerificationStatus> {
    let state_dir = get_state_dir();
    let status_file = state_dir.join("last_status.json");

    if !status_file.exists() {
        return None;
    }

    let content = std::fs::read_to_string(status_file).ok()?;
    let status: VerificationStatus = serde_json::from_str(&content).ok()?;

    // Only return if it's for the same project
    let canonical = path.canonicalize().ok()?;
    if status.project_path == canonical.to_string_lossy() {
        Some(status)
    } else {
        None
    }
}
