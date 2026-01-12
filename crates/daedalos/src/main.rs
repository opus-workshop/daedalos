//! daedalos - Unified entry point for all Daedalos tools
//!
//! "One command. All tools. Zero friction."
//!
//! The daedalos command exists because fragmentation kills adoption.
//! Twenty separate tools means twenty things to remember. This is
//! the front door - start here, everything is reachable.

use std::os::unix::process::CommandExt;
use std::path::PathBuf;
use std::process::Command;

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use colored::Colorize;
use daedalos_core::{process, Paths};
use serde::Serialize;

const VERSION: &str = "1.0.0";

/// Tool metadata
#[derive(Debug, Clone, Serialize)]
struct Tool {
    name: &'static str,
    description: &'static str,
    category: ToolCategory,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
enum ToolCategory {
    Core,
    Safety,
    Intelligence,
    Infrastructure,
    Supervision,
    Human,
}

impl Tool {
    const fn new(name: &'static str, description: &'static str, category: ToolCategory) -> Self {
        Self {
            name,
            description,
            category,
        }
    }
}

/// All Daedalos tools, categorized
const TOOLS: &[Tool] = &[
    // Core
    Tool::new("loop", "Iterate until promise met - THE CORE", ToolCategory::Core),
    // Safety
    Tool::new("verify", "Universal lint/type/build/test runner", ToolCategory::Safety),
    Tool::new("undo", "File-level undo with timeline", ToolCategory::Safety),
    Tool::new("sandbox", "Ephemeral experiment environments", ToolCategory::Safety),
    Tool::new("scratch", "Project-scoped ephemeral environments", ToolCategory::Safety),
    // Intelligence
    Tool::new("project", "Pre-computed codebase intelligence", ToolCategory::Intelligence),
    Tool::new("codex", "Semantic code search", ToolCategory::Intelligence),
    Tool::new("context", "Context window management", ToolCategory::Intelligence),
    Tool::new("error-db", "Error pattern database", ToolCategory::Intelligence),
    Tool::new("spec", "Rich specifications for AI context", ToolCategory::Intelligence),
    Tool::new("resolve", "Uncertainty resolution through context", ToolCategory::Intelligence),
    Tool::new("evolve", "Code evolution analysis", ToolCategory::Intelligence),
    Tool::new("analyze", "Analyze gaps and fill them (tests, coverage)", ToolCategory::Intelligence),
    // Infrastructure
    Tool::new("agent", "Multi-agent orchestration", ToolCategory::Infrastructure),
    Tool::new("mcp-hub", "MCP server management", ToolCategory::Infrastructure),
    Tool::new("lsp-pool", "Pre-warmed language servers", ToolCategory::Infrastructure),
    // Supervision
    Tool::new("observe", "Real-time TUI dashboard", ToolCategory::Supervision),
    Tool::new("gates", "Configurable approval checkpoints", ToolCategory::Supervision),
    Tool::new("journal", "Narrative reconstruction - what happened?", ToolCategory::Supervision),
    // Human-focused
    Tool::new("env", "Project environment switching", ToolCategory::Human),
    Tool::new("notify", "Desktop notifications", ToolCategory::Human),
    Tool::new("session", "Terminal session save/restore", ToolCategory::Human),
    Tool::new("secrets", "Local secrets vault (age encryption)", ToolCategory::Human),
    Tool::new("pair", "Pair programming (shared tmux)", ToolCategory::Human),
    Tool::new("handoff", "Context summaries for shift changes", ToolCategory::Human),
    Tool::new("review", "Human code review workflow", ToolCategory::Human),
    Tool::new("focus", "Pomodoro timer + distraction blocking", ToolCategory::Human),
    Tool::new("metrics", "Productivity statistics", ToolCategory::Human),
    Tool::new("template", "Project scaffolding", ToolCategory::Human),
    Tool::new("container", "Docker/Podman management", ToolCategory::Human),
    Tool::new("remote", "SSH + remote development", ToolCategory::Human),
    Tool::new("backup", "Project backup with encryption", ToolCategory::Human),
];

/// Daedalos - A Linux distribution and toolset designed BY AI, FOR AI development.
#[derive(Parser)]
#[command(name = "daedalos")]
#[command(version = VERSION)]
#[command(about = "Unified entry point for all Daedalos tools")]
#[command(disable_help_subcommand = true)]
#[command(arg_required_else_help = false)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,

    /// Tool name to run (alternative to subcommand)
    #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
    args: Vec<String>,
}

#[derive(Subcommand)]
enum Commands {
    /// Show detailed help with philosophy and tool overview
    Help,

    /// List all tools with installation status
    Tools {
        /// Output as JSON
        #[arg(long)]
        json: bool,
    },

    /// Show status of all Daedalos services
    Status,

    /// Verify installation and diagnose issues
    Doctor,

    /// Show version information
    Version,

    /// Install shell completions
    InstallCompletions {
        /// Shell type (bash or zsh)
        shell: Option<String>,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    // Handle explicit subcommands
    if let Some(command) = cli.command {
        return match command {
            Commands::Help => cmd_help(),
            Commands::Tools { json } => cmd_tools(json),
            Commands::Status => cmd_status(),
            Commands::Doctor => cmd_doctor(),
            Commands::Version => cmd_version(),
            Commands::InstallCompletions { shell } => cmd_install_completions(shell),
        };
    }

    // If no args, show help
    if cli.args.is_empty() {
        return cmd_help();
    }

    // First arg might be a tool name
    let first = &cli.args[0];

    // Check if it's a known tool
    if let Some(tool) = TOOLS.iter().find(|t| t.name == first) {
        dispatch_tool(tool.name, &cli.args[1..])
    } else {
        // Unknown command
        eprintln!("{} Unknown command: {}", "error:".red(), first);
        eprintln!("Run {} for usage", "daedalos help".bold());
        std::process::exit(1);
    }
}

fn cmd_help() -> Result<()> {
    let logo = r#"
    ____                  __      __
   / __ \____ ____  ____/ /___ _/ /___  _____
  / / / / __ `/ _ \/ __  / __ `/ / __ \/ ___/
 / /_/ / /_/ /  __/ /_/ / /_/ / / /_/ (__  )
/_____/\__,_/\___/\__,_/\__,_/_/\____/____/
"#;

    println!("{}", logo.cyan().bold());
    println!(
        "{}",
        "A Linux distribution and toolset designed BY AI, FOR AI development."
            .dimmed()
    );
    println!();

    println!("{}", "USAGE".bold());
    println!("    daedalos <tool> [args...]    Run a Daedalos tool");
    println!("    daedalos <command>           Run a meta-command");
    println!();

    // THE CORE
    println!("{}", "THE CORE".bold());
    if let Some(tool) = TOOLS.iter().find(|t| t.name == "loop") {
        println!("    {}          {}", tool.name.cyan(), tool.description);
        println!(
            "                  {}",
            "loop start \"fix tests\" --promise \"pytest\"".dimmed()
        );
    }
    println!();

    // Safety & Verification
    println!("{}", "SAFETY & VERIFICATION".bold());
    for tool in TOOLS.iter().filter(|t| t.category == ToolCategory::Safety) {
        println!("    {:12} {}", tool.name.cyan(), tool.description);
    }
    println!();

    // Intelligence
    println!("{}", "INTELLIGENCE".bold());
    for tool in TOOLS.iter().filter(|t| t.category == ToolCategory::Intelligence) {
        println!("    {:12} {}", tool.name.cyan(), tool.description);
    }
    println!();

    // Infrastructure
    println!("{}", "INFRASTRUCTURE".bold());
    for tool in TOOLS.iter().filter(|t| t.category == ToolCategory::Infrastructure) {
        println!("    {:12} {}", tool.name.cyan(), tool.description);
    }
    println!();

    // Supervision
    println!("{}", "SUPERVISION".bold());
    for tool in TOOLS.iter().filter(|t| t.category == ToolCategory::Supervision) {
        println!("    {:12} {}", tool.name.cyan(), tool.description);
    }
    println!();

    // Human-focused
    println!("{}", "HUMAN-FOCUSED".bold());
    for tool in TOOLS.iter().filter(|t| t.category == ToolCategory::Human) {
        println!("    {:12} {}", tool.name.green(), tool.description);
    }
    println!();

    // Meta-commands
    println!("{}", "META-COMMANDS".bold());
    println!("    {:12} Show status of all Daedalos services", "status".magenta());
    println!("    {:12} Verify installation and diagnose issues", "doctor".magenta());
    println!("    {:12} List all tools with installation status", "tools".magenta());
    println!("    {:12} Show version information", "version".magenta());
    println!(
        "    {:12} Install shell completions (bash/zsh)",
        "install-completions".magenta()
    );
    println!();

    // Philosophy
    println!("{}", "PHILOSOPHY".bold());
    println!(
        "    \"{}\"",
        "A loop is not a feature. A loop is how intelligent work gets done."
    );
    println!();
    println!("    {} - Single-pass inference is a myth", "Iterate until done".bold());
    println!(
        "    {} - Every iteration has a rollback",
        "Checkpoints everywhere".bold()
    );
    println!("    {} - Works with any AI coding agent", "Agent agnostic".bold());
    println!("    {} - No vendor lock-in", "FOSS by design".bold());
    println!();

    // Quick start
    println!("{}", "QUICK START".bold());
    println!(
        "    daedalos loop start \"implement feature\" --promise \"npm test\""
    );
    println!("    daedalos verify --quick");
    println!("    daedalos undo checkpoint \"before-refactor\"");
    println!("    daedalos codex search \"authentication\"");
    println!();

    println!("{}", "DOCUMENTATION".bold());
    println!("    https://github.com/daedalos/daedalos");
    println!();

    Ok(())
}

fn cmd_tools(json: bool) -> Result<()> {
    let paths = Paths::new();

    #[derive(Serialize)]
    struct ToolInfo {
        name: &'static str,
        description: &'static str,
        installed: bool,
        path: PathBuf,
    }

    let tools_info: Vec<ToolInfo> = TOOLS
        .iter()
        .map(|t| {
            let path = paths.tools.join(t.name);
            let installed = path.is_file() && is_executable(&path);
            ToolInfo {
                name: t.name,
                description: t.description,
                installed,
                path,
            }
        })
        .collect();

    if json {
        println!("{}", serde_json::to_string_pretty(&tools_info)?);
        return Ok(());
    }

    println!("{}", "Daedalos Tools".bold());
    println!("===============");
    println!();

    let mut installed = 0;
    let mut missing = 0;

    for info in &tools_info {
        if info.installed {
            println!(
                "{} {} - {}",
                "ok".green(),
                info.name.cyan(),
                info.description
            );
            installed += 1;
        } else {
            println!(
                "{}  {} - {} {}",
                "!".yellow(),
                info.name.dimmed(),
                info.description,
                "(not installed)".red()
            );
            missing += 1;
        }
    }

    println!();
    println!("{} installed, {} missing", installed, missing);

    if missing > 0 {
        println!();
        println!(
            "Run {} to diagnose installation issues.",
            "daedalos doctor".bold()
        );
    }

    Ok(())
}

fn cmd_status() -> Result<()> {
    let paths = Paths::new();

    println!("{}", "Daedalos Status".bold());
    println!("================");
    println!();

    // Check loop daemon
    print!("Loop daemon:     ");
    if process::is_running("loopd") {
        println!("{}", "running".green());
    } else {
        println!("{}", "stopped".dimmed());
    }

    // Check MCP Hub
    print!("MCP Hub:         ");
    let mcp_socket = paths.runtime.join("mcp-hub").join("mcp-hub.sock");
    if mcp_socket.exists() {
        println!("{}", "running".green());
    } else {
        println!("{}", "stopped".dimmed());
    }

    // Check LSP Pool
    print!("LSP Pool:        ");
    let lsp_socket = paths.runtime.join("lsp-pool").join("lsp-pool.sock");
    if lsp_socket.exists() {
        println!("{}", "running".green());
    } else {
        println!("{}", "stopped".dimmed());
    }

    // Check undo daemon
    print!("Undo daemon:     ");
    if process::is_running("undod") {
        println!("{}", "running".green());
    } else {
        println!("{}", "stopped".dimmed());
    }

    println!();

    // Active loops
    println!("{}", "Active Loops".bold());
    let loop_path = paths.tools.join("loop");
    if loop_path.exists() {
        let output = Command::new(&loop_path).arg("list").output();
        match output {
            Ok(out) if out.status.success() => {
                let stdout = String::from_utf8_lossy(&out.stdout);
                if stdout.trim().is_empty() {
                    println!("{}", "No active loops".dimmed());
                } else {
                    print!("{}", stdout);
                }
            }
            _ => println!("{}", "No active loops".dimmed()),
        }
    } else {
        println!("{}", "loop tool not installed".dimmed());
    }

    println!();

    // Active agents
    println!("{}", "Active Agents".bold());
    let agent_path = paths.tools.join("agent");
    if agent_path.exists() {
        let output = Command::new(&agent_path).arg("list").output();
        match output {
            Ok(out) if out.status.success() => {
                let stdout = String::from_utf8_lossy(&out.stdout);
                if stdout.trim().is_empty() {
                    println!("{}", "No active agents".dimmed());
                } else {
                    print!("{}", stdout);
                }
            }
            _ => println!("{}", "No active agents".dimmed()),
        }
    } else {
        println!("{}", "agent tool not installed".dimmed());
    }

    Ok(())
}

fn cmd_doctor() -> Result<()> {
    let paths = Paths::new();

    println!("{}", "Daedalos Doctor".bold());
    println!("================");
    println!();

    let mut issues = 0;

    // Check tools directory
    println!("{}", "Checking installation...".bold());
    if paths.tools.exists() {
        println!(
            "{} Tools directory exists: {}",
            "ok".green(),
            paths.tools.display()
        );
    } else {
        println!(
            "{}  Tools directory missing: {}",
            "!".yellow(),
            paths.tools.display()
        );
        issues += 1;
    }

    // Check each tool
    println!();
    println!("{}", "Checking tools...".bold());
    for tool in TOOLS {
        let tool_path = paths.tools.join(tool.name);
        if tool_path.exists() && is_executable(&tool_path) {
            println!("{} {}", "ok".green(), tool.name);
        } else {
            println!("{}  {} - not found", "!".yellow(), tool.name);
            issues += 1;
        }
    }

    // Check data directories
    println!();
    println!("{}", "Checking data directories...".bold());
    for dir in [&paths.data, &paths.config] {
        if dir.exists() {
            println!("{} {}", "ok".green(), dir.display());
        } else {
            println!(
                "{}  {} - will be created on first use",
                "!".yellow(),
                dir.display()
            );
        }
    }

    // Check dependencies
    println!();
    println!("{}", "Checking dependencies...".bold());
    let required_deps = ["sqlite3", "tmux", "git"];
    for dep in required_deps {
        if which::which(dep).is_ok() {
            println!("{} {}", "ok".green(), dep);
        } else {
            println!("{}  {} - not found", "!".yellow(), dep);
            issues += 1;
        }
    }

    // Optional dependencies
    println!();
    println!("{}", "Optional dependencies...".bold());
    let optional_deps = ["btrfs", "fuse-overlayfs", "python3"];
    for dep in optional_deps {
        if which::which(dep).is_ok() {
            println!("{} {}", "ok".green(), dep);
        } else {
            println!("{} {} - not found (optional)", "-".dimmed(), dep.dimmed());
        }
    }

    println!();
    if issues == 0 {
        println!("{} All checks passed!", "ok".green());
    } else {
        println!("{}  {} issues found", "!".yellow(), issues);
        println!();
        println!("To fix missing tools, run the install scripts in daedalos-tools/");
    }

    Ok(())
}

fn cmd_version() -> Result<()> {
    println!("daedalos {}", VERSION);
    println!();

    let paths = Paths::new();
    println!("Tools:");

    let key_tools = ["loop", "verify", "undo", "agent", "mcp-hub", "lsp-pool"];
    for name in key_tools {
        let tool_path = paths.tools.join(name);
        if tool_path.exists() {
            // Try --version, then version subcommand
            let ver = Command::new(&tool_path)
                .arg("--version")
                .output()
                .ok()
                .filter(|o| o.status.success())
                .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
                .or_else(|| {
                    Command::new(&tool_path)
                        .arg("version")
                        .output()
                        .ok()
                        .filter(|o| o.status.success())
                        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
                })
                .unwrap_or_else(|| "installed".to_string());

            println!("  {}: {}", name, ver);
        }
    }

    Ok(())
}

fn cmd_install_completions(shell: Option<String>) -> Result<()> {
    let shell = shell.unwrap_or_else(|| {
        std::env::var("SHELL")
            .map(|s| {
                if s.contains("zsh") {
                    "zsh".to_string()
                } else {
                    "bash".to_string()
                }
            })
            .unwrap_or_else(|_| "bash".to_string())
    });

    match shell.as_str() {
        "bash" => {
            let home = dirs::home_dir().context("Could not find home directory")?;
            let target = home.join(".bash_completion.d");
            std::fs::create_dir_all(&target)?;

            // Generate completion script
            let completion = generate_bash_completion();
            let comp_file = target.join("daedalos.bash");
            std::fs::write(&comp_file, completion)?;

            println!("{} Installed bash completions to {}", "ok".green(), comp_file.display());
            println!();
            println!("Add to your ~/.bashrc:");
            println!("  source ~/.bash_completion.d/daedalos.bash");
        }
        "zsh" => {
            let home = dirs::home_dir().context("Could not find home directory")?;
            let target = home.join(".zsh/completions");
            std::fs::create_dir_all(&target)?;

            // Generate completion script
            let completion = generate_zsh_completion();
            let comp_file = target.join("_daedalos");
            std::fs::write(&comp_file, completion)?;

            println!("{} Installed zsh completions to {}", "ok".green(), comp_file.display());
            println!();
            println!("Add to your ~/.zshrc (before compinit):");
            println!("  fpath=(~/.zsh/completions $fpath)");
        }
        _ => {
            eprintln!("{} Unknown shell: {}", "error:".red(), shell);
            eprintln!("Supported shells: bash, zsh");
            std::process::exit(1);
        }
    }

    Ok(())
}

fn dispatch_tool(name: &str, args: &[String]) -> Result<()> {
    let paths = Paths::new();
    let tool_path = paths.tools.join(name);

    if !tool_path.exists() {
        eprintln!("{} Tool '{}' is not installed", "error:".red(), name);
        eprintln!("Run {} to check installation", "daedalos doctor".bold());
        std::process::exit(1);
    }

    // Use exec to replace the current process (no overhead)
    let err = Command::new(&tool_path).args(args).exec();

    // If we get here, exec failed
    Err(err).context(format!("Failed to exec {}", tool_path.display()))
}

fn is_executable(path: &PathBuf) -> bool {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if let Ok(metadata) = std::fs::metadata(path) {
            return metadata.permissions().mode() & 0o111 != 0;
        }
    }
    path.is_file()
}

fn generate_bash_completion() -> String {
    let tools: Vec<&str> = TOOLS.iter().map(|t| t.name).collect();
    let commands = ["help", "tools", "status", "doctor", "version", "install-completions"];
    let all: Vec<&str> = tools.iter().copied().chain(commands.iter().copied()).collect();

    format!(
        r#"# Daedalos bash completion
_daedalos() {{
    local cur prev words cword
    _init_completion || return

    if [[ ${{cword}} -eq 1 ]]; then
        COMPREPLY=($(compgen -W "{}" -- "${{cur}}"))
        return
    fi

    # Let the tool handle its own completions
    case "${{words[1]}}" in
        loop|verify|undo|agent|codex|project)
            # Try to get completions from the tool itself
            if command -v "${{words[1]}}" &>/dev/null; then
                COMPREPLY=($(compgen -W "$("${{words[1]}}" --help 2>/dev/null | grep -oE '^\s+\w+' | tr -d ' ')" -- "${{cur}}"))
            fi
            ;;
    esac
}}

complete -F _daedalos daedalos
"#,
        all.join(" ")
    )
}

fn generate_zsh_completion() -> String {
    let mut tool_completions = String::new();
    for tool in TOOLS {
        tool_completions.push_str(&format!(
            "            '{}:{}'\n",
            tool.name,
            tool.description.replace('\'', "")
        ));
    }

    format!(
        r#"#compdef daedalos

_daedalos() {{
    local -a commands
    commands=(
        'help:Show detailed help with philosophy and tool overview'
        'tools:List all tools with installation status'
        'status:Show status of all Daedalos services'
        'doctor:Verify installation and diagnose issues'
        'version:Show version information'
        'install-completions:Install shell completions'
{}    )

    _arguments -C \
        '1: :->command' \
        '*:: :->args'

    case $state in
        command)
            _describe -t commands 'daedalos commands' commands
            ;;
        args)
            case $words[1] in
                tools)
                    _arguments '--json[Output as JSON]'
                    ;;
                install-completions)
                    _values 'shell' 'bash' 'zsh'
                    ;;
            esac
            ;;
    esac
}}

_daedalos
"#,
        tool_completions
    )
}
