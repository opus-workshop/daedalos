//! agent - Multi-agent orchestration for Daedalos
//!
//! Manages multiple Claude Code instances running in tmux sessions.
//! Part of the Daedalos toolsuite.

mod commands;
mod messaging;
mod state;
mod templates;
mod tmux;

use anyhow::Result;
use clap::{Parser, Subcommand};

use state::AgentState;

/// Multi-agent orchestration for Daedalos
#[derive(Parser)]
#[command(name = "agent")]
#[command(version)]
#[command(about = "Multi-agent orchestration - spawn and coordinate parallel agents")]
#[command(after_help = "\
TRIGGER:
    Use agent spawn when a task would benefit from parallel exploration
    or when you need specialized agents for different aspects of work.

EXAMPLES:
    agent spawn -t explorer        Spawn an explorer agent
    agent spawn -t implementer     Spawn an implementer agent
    agent spawn --name reviewer    Spawn with custom name
    agent list                     List all running agents
    agent focus explorer           Switch to an agent
    agent status                   Show all agent statuses
    agent send explorer \"check auth\"  Send message to agent
    agent inbox                    Check your messages
    agent broadcast \"status?\"      Message all agents
    agent kill explorer            Stop an agent

TEMPLATES:
    explorer    - For codebase exploration and research
    implementer - For writing code and features
    reviewer    - For code review
    debugger    - For bug investigation
    planner     - For architectural planning
    tester      - For writing tests
    watcher     - For monitoring and observation

PHILOSOPHY:
    Single agents hit walls. Multiple specialized agents break through.
    Coordinate them with messaging. Each focuses on what it does best.")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Spawn a new agent
    Spawn {
        /// Agent name
        #[arg(short, long)]
        name: Option<String>,

        /// Project directory (default: current)
        #[arg(short, long)]
        project: Option<String>,

        /// Template to use (explorer, implementer, reviewer, debugger, planner, tester, watcher)
        #[arg(short, long)]
        template: Option<String>,

        /// Don't focus the new agent
        #[arg(long)]
        no_focus: bool,

        /// Initial prompt to send
        #[arg(long)]
        prompt: Option<String>,
    },

    /// List all agents
    List {
        /// Output as JSON
        #[arg(long)]
        json: bool,

        /// Only output agent names
        #[arg(short, long)]
        quiet: bool,
    },

    /// Focus (switch to) an agent
    Focus {
        /// Agent name or slot number
        identifier: String,
    },

    /// Show agent status
    Status {
        /// Agent name or slot number (optional, shows all if omitted)
        identifier: Option<String>,

        /// Output as JSON
        #[arg(long)]
        json: bool,
    },

    /// Kill an agent
    Kill {
        /// Agent name or slot number (or --all)
        identifier: Option<String>,

        /// Force kill without graceful shutdown
        #[arg(short, long)]
        force: bool,

        /// Kill all agents
        #[arg(short, long)]
        all: bool,
    },

    /// Show agent logs
    Logs {
        /// Agent name or slot number
        identifier: String,

        /// Follow logs
        #[arg(short, long, default_value = "true")]
        follow: bool,

        /// Number of lines to show
        #[arg(short = 'n', default_value = "100")]
        lines: u32,
    },

    /// Search across agent outputs
    Search {
        /// Search query
        query: String,

        /// Search only in this agent
        #[arg(short, long)]
        agent: Option<String>,

        /// Case insensitive search
        #[arg(short, long)]
        ignore_case: bool,

        /// Output as JSON
        #[arg(long)]
        json: bool,
    },

    /// Pause an agent (SIGSTOP)
    Pause {
        /// Agent name or slot number
        identifier: String,
    },

    /// Resume a paused agent (SIGCONT)
    Resume {
        /// Agent name or slot number
        identifier: String,
    },

    /// Send a message to another agent
    Send {
        /// Target agent name
        to: String,

        /// Message content
        message: String,
    },

    /// Check agent's inbox
    Inbox {
        /// Agent name (default: current agent)
        agent: Option<String>,

        /// Show all messages including read
        #[arg(long)]
        all: bool,

        /// Output as JSON
        #[arg(long)]
        json: bool,
    },

    /// Broadcast a message to all agents
    Broadcast {
        /// Message content
        message: String,
    },

    /// Manage templates
    Templates {
        #[command(subcommand)]
        action: Option<TemplatesAction>,
    },
}

#[derive(Subcommand)]
enum TemplatesAction {
    /// List available templates
    List {
        /// Output as JSON
        #[arg(long)]
        json: bool,
    },

    /// Show template details
    Show {
        /// Template name
        name: String,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let state = AgentState::new()?;

    match cli.command {
        Commands::Spawn {
            name,
            project,
            template,
            no_focus,
            prompt,
        } => {
            commands::spawn(&state, name, project, template, no_focus, prompt)?;
        }

        Commands::List { json, quiet } => {
            commands::list(&state, json, quiet)?;
        }

        Commands::Focus { identifier } => {
            commands::focus(&state, &identifier)?;
        }

        Commands::Status { identifier, json } => {
            commands::status(&state, identifier.as_deref(), json)?;
        }

        Commands::Kill {
            identifier,
            force,
            all,
        } => {
            if all {
                // Confirmation prompt
                eprintln!("This will kill all agents.");
                eprint!("Are you sure? (y/N) ");
                let mut input = String::new();
                std::io::stdin().read_line(&mut input)?;
                if input.trim().to_lowercase() != "y" {
                    eprintln!("Aborted.");
                    return Ok(());
                }
                commands::kill_all(&state, force)?;
            } else if let Some(id) = identifier {
                commands::kill(&state, &id, force)?;
            } else {
                anyhow::bail!("Usage: agent kill <name|slot> or agent kill --all");
            }
        }

        Commands::Logs {
            identifier,
            follow,
            lines,
        } => {
            commands::logs(&state, &identifier, lines, follow)?;
        }

        Commands::Search {
            query,
            agent,
            ignore_case,
            json,
        } => {
            commands::search(&state, &query, agent.as_deref(), ignore_case, json)?;
        }

        Commands::Pause { identifier } => {
            commands::pause(&state, &identifier)?;
        }

        Commands::Resume { identifier } => {
            commands::resume(&state, &identifier)?;
        }

        Commands::Send { to, message } => {
            commands::send(&state, &to, &message)?;
        }

        Commands::Inbox { agent, all, json } => {
            commands::inbox(&state, agent.as_deref(), all, json)?;
        }

        Commands::Broadcast { message } => {
            commands::broadcast(&state, &message)?;
        }

        Commands::Templates { action } => match action {
            Some(TemplatesAction::List { json }) => {
                commands::templates_list(json)?;
            }
            Some(TemplatesAction::Show { name }) => {
                commands::templates_show(&name)?;
            }
            None => {
                commands::templates_list(false)?;
            }
        },
    }

    Ok(())
}
