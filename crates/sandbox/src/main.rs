//! sandbox - Ephemeral experiment environments for Daedalos
//!
//! "Try anything. Break nothing."
//!
//! Create isolated, disposable environments for experimentation.
//! Changes in a sandbox don't affect the original until promoted.
//!
//! Commands:
//! - create [name]: Create a new sandbox environment
//! - list: List all sandboxes
//! - enter <name>: Enter sandbox environment
//! - diff <name>: Show changes in sandbox
//! - promote <name>: Apply sandbox changes to source
//! - discard <name>: Delete sandbox
//! - info <name>: Show sandbox details
//! - run <name> <cmd>: Run command in sandbox

mod backend;
mod sandbox;

use anyhow::{bail, Context, Result};
use chrono::{DateTime, Local, Utc};
use clap::{Parser, Subcommand};
use daedalos_core::Paths;
use std::path::PathBuf;

use crate::sandbox::{Backend, SandboxManager};

#[derive(Parser)]
#[command(name = "sandbox")]
#[command(about = "Ephemeral experiment environments - try anything, break nothing")]
#[command(version)]
#[command(after_help = "\
TRIGGER:
    Use sandbox when you need true filesystem isolation for experiments.
    Unlike scratch (which copies files), sandbox uses filesystem-level
    copy-on-write for instant creation and minimal disk usage.

EXAMPLES:
    sandbox create                  Create sandbox from current directory
    sandbox create --name test      Create with specific name
    sandbox list                    Show all sandboxes
    sandbox enter test              Enter sandbox shell
    sandbox diff test               See changes made
    sandbox promote test            Apply changes back to source
    sandbox discard test            Delete sandbox
    sandbox run test make build     Run command in sandbox
    sandbox info test               Show sandbox details

BACKENDS:
    btrfs    - Btrfs subvolume snapshot (instant, near-zero disk)
    overlay  - OverlayFS (requires fuse-overlayfs for non-root)
    rsync    - Full copy fallback (works everywhere)

vs SCRATCH:
    sandbox = filesystem-level isolation (Btrfs/overlay)
    scratch = project-level copies (git worktree or rsync)
    Use sandbox for lower-level experimentation, scratch for coding")]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// Create a new sandbox environment
    Create {
        /// Name for the sandbox (auto-generated if not provided)
        name: Option<String>,

        /// Source directory to sandbox (default: current directory)
        #[arg(short, long)]
        from: Option<PathBuf>,

        /// Backend: btrfs, overlay, or rsync (auto-detected)
        #[arg(long)]
        backend: Option<String>,
    },

    /// List all sandboxes
    List {
        /// Output as JSON
        #[arg(long)]
        json: bool,
    },

    /// Enter sandbox environment
    Enter {
        /// Sandbox name
        name: String,

        /// Run command instead of shell
        #[arg(short, long)]
        command: Option<String>,
    },

    /// Show changes in sandbox compared to source
    Diff {
        /// Sandbox name
        name: String,

        /// List changed files only
        #[arg(long)]
        files_only: bool,
    },

    /// Apply sandbox changes to original source
    Promote {
        /// Sandbox name
        name: String,

        /// Show what would be promoted without doing it
        #[arg(long)]
        dry_run: bool,

        /// Create backup of source first
        #[arg(long)]
        backup: bool,
    },

    /// Delete sandbox and all changes permanently
    Discard {
        /// Sandbox name
        name: String,

        /// Don't ask for confirmation
        #[arg(short, long)]
        force: bool,
    },

    /// Show detailed sandbox information
    Info {
        /// Sandbox name
        name: String,

        /// Output as JSON
        #[arg(long)]
        json: bool,
    },

    /// Execute command inside sandbox without entering
    Run {
        /// Sandbox name
        name: String,

        /// Command to run
        command: Vec<String>,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    let paths = Paths::new();
    let sandbox_root = paths.data.join("sandbox");
    let manager = SandboxManager::new(&sandbox_root)?;

    match cli.command {
        Some(Commands::Create { name, from, backend }) => {
            cmd_create(&manager, name, from, backend)
        }
        Some(Commands::List { json }) => {
            cmd_list(&manager, json)
        }
        Some(Commands::Enter { name, command }) => {
            cmd_enter(&manager, &name, command)
        }
        Some(Commands::Diff { name, files_only }) => {
            cmd_diff(&manager, &name, files_only)
        }
        Some(Commands::Promote { name, dry_run, backup }) => {
            cmd_promote(&manager, &name, dry_run, backup)
        }
        Some(Commands::Discard { name, force }) => {
            cmd_discard(&manager, &name, force)
        }
        Some(Commands::Info { name, json }) => {
            cmd_info(&manager, &name, json)
        }
        Some(Commands::Run { name, command }) => {
            cmd_run(&manager, &name, command)
        }
        None => {
            // Default: list sandboxes
            cmd_list(&manager, false)
        }
    }
}

/// Create a new sandbox
fn cmd_create(
    manager: &SandboxManager,
    name: Option<String>,
    from: Option<PathBuf>,
    backend_str: Option<String>,
) -> Result<()> {
    let source = from.unwrap_or_else(|| std::env::current_dir().unwrap_or_default());
    let source = source.canonicalize().context("Source directory not found")?;

    if !source.is_dir() {
        bail!("Source must be a directory: {}", source.display());
    }

    let name = name.unwrap_or_else(generate_name);

    if manager.exists(&name) {
        bail!("Sandbox '{}' already exists", name);
    }

    let backend = if let Some(b) = backend_str {
        match b.as_str() {
            "btrfs" => Backend::Btrfs,
            "overlay" => Backend::Overlay,
            "rsync" => Backend::Rsync,
            _ => bail!("Unknown backend: {}. Use btrfs, overlay, or rsync", b),
        }
    } else {
        detect_backend(&source)
    };

    println!("Creating sandbox '{}' from {}", name, source.display());
    println!("Using backend: {:?}", backend);

    let _sandbox = manager.create(&name, &source, backend)?;

    println!("\nSandbox '{}' created!", name);
    println!();
    println!("Enter with:   sandbox enter {}", name);
    println!("See changes:  sandbox diff {}", name);
    println!("Apply:        sandbox promote {}", name);
    println!("Discard:      sandbox discard {}", name);

    Ok(())
}

/// List all sandboxes
fn cmd_list(manager: &SandboxManager, json: bool) -> Result<()> {
    let sandboxes = manager.list()?;

    if json {
        let json_list: Vec<serde_json::Value> = sandboxes
            .iter()
            .map(|s| {
                serde_json::json!({
                    "name": s.name,
                    "source": s.source.display().to_string(),
                    "created": s.created.to_rfc3339(),
                    "backend": format!("{:?}", s.backend).to_lowercase(),
                })
            })
            .collect();
        println!("{}", serde_json::to_string_pretty(&json_list)?);
        return Ok(());
    }

    if sandboxes.is_empty() {
        println!("No sandboxes found. Create one with: sandbox create");
        return Ok(());
    }

    println!(
        "{:<20} {:<30} {:<15} {:<10}",
        "NAME", "SOURCE", "CREATED", "BACKEND"
    );
    println!("{}", "-".repeat(75));

    for sandbox in &sandboxes {
        let source_display = truncate_path(&sandbox.source, 28);
        let age = format_time_ago(&sandbox.created);
        let backend = format!("{:?}", sandbox.backend).to_lowercase();

        println!(
            "{:<20} {:<30} {:<15} {:<10}",
            sandbox.name, source_display, age, backend
        );
    }

    Ok(())
}

/// Enter sandbox environment
fn cmd_enter(manager: &SandboxManager, name: &str, command: Option<String>) -> Result<()> {
    let sandbox = manager.get(name)?;
    let work_path = sandbox.work_path();

    if !work_path.exists() {
        bail!("Sandbox '{}' not found", name);
    }

    // Set environment variables
    std::env::set_var("SANDBOX_NAME", name);
    std::env::set_var("SANDBOX_SOURCE", sandbox.source.display().to_string());
    std::env::set_var("IN_SANDBOX", "1");
    std::env::set_current_dir(&work_path)?;

    if let Some(cmd) = command {
        // Run specific command
        let status = std::process::Command::new("sh")
            .arg("-c")
            .arg(&cmd)
            .status()
            .context("Failed to run command")?;

        std::process::exit(status.code().unwrap_or(1));
    } else {
        // Enter interactive shell
        println!("Entering sandbox '{}' - exit to return", name);

        let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/bash".to_string());
        let status = std::process::Command::new(&shell)
            .env("PS1", format!("[sandbox:{}] \\$ ", name))
            .status()
            .context("Failed to start shell")?;

        std::process::exit(status.code().unwrap_or(0));
    }
}

/// Show changes in sandbox
fn cmd_diff(manager: &SandboxManager, name: &str, files_only: bool) -> Result<()> {
    let sandbox = manager.get(name)?;
    let work_path = sandbox.work_path();

    if !work_path.exists() {
        bail!("Sandbox '{}' not found", name);
    }

    // Use diff command to compare
    let mut cmd = std::process::Command::new("diff");

    if files_only {
        cmd.arg("-rq");
    } else {
        cmd.arg("-ru");
    }

    cmd.arg(&sandbox.source);
    cmd.arg(&work_path);

    let output = cmd.output().context("Failed to run diff")?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    if stdout.is_empty() && stderr.is_empty() {
        println!("No changes");
    } else {
        if !stdout.is_empty() {
            print!("{}", stdout);
        }
        if !stderr.is_empty() {
            eprint!("{}", stderr);
        }
    }

    Ok(())
}

/// Promote sandbox changes to source
fn cmd_promote(manager: &SandboxManager, name: &str, dry_run: bool, backup: bool) -> Result<()> {
    let sandbox = manager.get(name)?;
    let work_path = sandbox.work_path();

    if !work_path.exists() {
        bail!("Sandbox '{}' not found", name);
    }

    if dry_run {
        println!("Dry run - showing what would be promoted:");
        let output = std::process::Command::new("rsync")
            .args(["-avn", "--delete"])
            .arg(format!("{}/", work_path.display()))
            .arg(format!("{}/", sandbox.source.display()))
            .output()
            .context("Failed to run rsync dry-run")?;

        print!("{}", String::from_utf8_lossy(&output.stdout));
        return Ok(());
    }

    // Create backup if requested
    if backup {
        let backup_path = format!(
            "{}.backup.{}",
            sandbox.source.display(),
            Local::now().format("%Y%m%d_%H%M%S")
        );
        println!("Creating backup at {}", backup_path);

        std::process::Command::new("rsync")
            .args(["-a"])
            .arg(format!("{}/", sandbox.source.display()))
            .arg(format!("{}/", backup_path))
            .status()
            .context("Failed to create backup")?;
    }

    println!("Promoting sandbox '{}' to {}", name, sandbox.source.display());

    let status = std::process::Command::new("rsync")
        .args(["-av", "--delete"])
        .arg(format!("{}/", work_path.display()))
        .arg(format!("{}/", sandbox.source.display()))
        .status()
        .context("Failed to promote changes")?;

    if !status.success() {
        bail!("Promote failed");
    }

    println!("\nSandbox '{}' promoted to {}", name, sandbox.source.display());
    println!("Run 'sandbox discard {}' to clean up the sandbox", name);

    Ok(())
}

/// Discard (delete) sandbox
fn cmd_discard(manager: &SandboxManager, name: &str, force: bool) -> Result<()> {
    if !manager.exists(name) {
        bail!("Sandbox '{}' not found", name);
    }

    if !force {
        // In non-interactive mode, require --force
        if !atty::is(atty::Stream::Stdin) {
            bail!("Use --force to discard without confirmation");
        }

        print!("Discard sandbox '{}'? This cannot be undone. [y/N] ", name);
        use std::io::Write;
        std::io::stdout().flush()?;

        let mut input = String::new();
        std::io::stdin().read_line(&mut input)?;

        if !input.trim().eq_ignore_ascii_case("y") && !input.trim().eq_ignore_ascii_case("yes") {
            println!("Aborted.");
            return Ok(());
        }
    }

    println!("Discarding sandbox '{}'", name);
    manager.discard(name)?;
    println!("Sandbox '{}' discarded", name);

    Ok(())
}

/// Show sandbox information
fn cmd_info(manager: &SandboxManager, name: &str, json: bool) -> Result<()> {
    let sandbox = manager.get(name)?;
    let work_path = sandbox.work_path();

    // Calculate size
    let size = get_dir_size(&work_path);
    let size_human = human_size(size);

    // Count changed files
    let changes = count_changes(&sandbox.source, &work_path);

    if json {
        let info = serde_json::json!({
            "name": sandbox.name,
            "source": sandbox.source.display().to_string(),
            "created": sandbox.created.to_rfc3339(),
            "backend": format!("{:?}", sandbox.backend).to_lowercase(),
            "size": size_human,
            "changes": changes,
            "path": work_path.display().to_string(),
        });
        println!("{}", serde_json::to_string_pretty(&info)?);
        return Ok(());
    }

    println!("Sandbox: {}", sandbox.name);
    println!("{}", "=".repeat(40));
    println!("Source:     {}", sandbox.source.display());
    println!("Created:    {} ({})",
        sandbox.created.format("%Y-%m-%d %H:%M:%S"),
        format_time_ago(&sandbox.created)
    );
    println!("Backend:    {:?}", sandbox.backend);
    println!("Size:       {}", size_human);
    println!("Changes:    {} files", changes);
    println!("Path:       {}", work_path.display());

    Ok(())
}

/// Run command in sandbox
fn cmd_run(manager: &SandboxManager, name: &str, command: Vec<String>) -> Result<()> {
    if command.is_empty() {
        bail!("Command required");
    }

    let sandbox = manager.get(name)?;
    let work_path = sandbox.work_path();

    if !work_path.exists() {
        bail!("Sandbox '{}' not found", name);
    }

    let cmd_string = command.join(" ");

    let status = std::process::Command::new("sh")
        .arg("-c")
        .arg(&cmd_string)
        .current_dir(&work_path)
        .env("SANDBOX_NAME", name)
        .env("IN_SANDBOX", "1")
        .status()
        .context("Failed to run command")?;

    std::process::exit(status.code().unwrap_or(1));
}

// Helper functions

/// Generate a memorable sandbox name
fn generate_name() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};

    let adjectives = ["quick", "bold", "calm", "deep", "fast", "keen", "warm", "cool", "swift", "wild"];
    let nouns = ["fox", "owl", "elk", "bee", "ant", "ray", "oak", "ivy", "ash", "bay"];

    let seed = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos() as usize;

    let adj = adjectives[seed % adjectives.len()];
    let noun = nouns[(seed / adjectives.len()) % nouns.len()];
    let num = (seed / (adjectives.len() * nouns.len())) % 100;

    format!("{}-{}-{}", adj, noun, num)
}

/// Detect the best available backend
fn detect_backend(source: &std::path::Path) -> Backend {
    // Check for Btrfs
    if std::process::Command::new("btrfs")
        .args(["subvolume", "show", source.to_str().unwrap_or(".")])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
    {
        return Backend::Btrfs;
    }

    // Check for overlay support (usually requires root or fuse-overlayfs)
    if std::path::Path::new("/proc/filesystems").exists() {
        if let Ok(content) = std::fs::read_to_string("/proc/filesystems") {
            if content.contains("overlay") {
                // Check for fuse-overlayfs on non-root
                if std::process::Command::new("which")
                    .arg("fuse-overlayfs")
                    .output()
                    .map(|o| o.status.success())
                    .unwrap_or(false)
                {
                    return Backend::Overlay;
                }
            }
        }
    }

    // Default to rsync (works everywhere)
    Backend::Rsync
}

/// Format time as human-readable "X ago"
fn format_time_ago(dt: &DateTime<Utc>) -> String {
    let now = Utc::now();
    let duration = now.signed_duration_since(*dt);

    if duration.num_seconds() < 60 {
        "just now".to_string()
    } else if duration.num_minutes() < 60 {
        format!("{} min ago", duration.num_minutes())
    } else if duration.num_hours() < 24 {
        format!("{} hours ago", duration.num_hours())
    } else {
        format!("{} days ago", duration.num_days())
    }
}

/// Truncate path for display
fn truncate_path(path: &std::path::Path, max_len: usize) -> String {
    let path_str = path.display().to_string();
    if path_str.len() <= max_len {
        path_str
    } else {
        format!("...{}", &path_str[path_str.len() - (max_len - 3)..])
    }
}

/// Get directory size in bytes
fn get_dir_size(path: &std::path::Path) -> u64 {
    if !path.exists() {
        return 0;
    }

    // Use du command for efficiency
    std::process::Command::new("du")
        .args(["-sb", path.to_str().unwrap_or(".")])
        .output()
        .ok()
        .and_then(|o| {
            String::from_utf8_lossy(&o.stdout)
                .split_whitespace()
                .next()
                .and_then(|s| s.parse().ok())
        })
        .unwrap_or(0)
}

/// Format bytes as human-readable size
fn human_size(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = KB * 1024;
    const GB: u64 = MB * 1024;

    if bytes < KB {
        format!("{}B", bytes)
    } else if bytes < MB {
        format!("{}K", bytes / KB)
    } else if bytes < GB {
        format!("{}M", bytes / MB)
    } else {
        format!("{}G", bytes / GB)
    }
}

/// Count changed files between source and work
fn count_changes(source: &std::path::Path, work: &std::path::Path) -> usize {
    std::process::Command::new("diff")
        .args(["-rq"])
        .arg(source)
        .arg(work)
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).lines().count())
        .unwrap_or(0)
}
