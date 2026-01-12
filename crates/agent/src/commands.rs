//! CLI command implementations

use anyhow::Result;
use chrono::Utc;

use crate::messaging::{self, MessageType, Messaging};
use crate::state::{generate_name, validate_name, AgentState, AgentStatus};
use crate::templates::{get_template, list_templates};
use crate::tmux;

/// Spawn a new agent
pub fn spawn(
    state: &AgentState,
    name: Option<String>,
    project: Option<String>,
    template: Option<String>,
    no_focus: bool,
    initial_prompt: Option<String>,
) -> Result<()> {
    // Check tmux availability
    if !tmux::is_available() {
        anyhow::bail!("tmux is not available. Install tmux to use agent spawning.");
    }

    // Resolve project directory
    let project = project
        .map(|p| {
            std::fs::canonicalize(&p)
                .unwrap_or_else(|_| std::path::PathBuf::from(&p))
                .to_string_lossy()
                .to_string()
        })
        .unwrap_or_else(|| {
            std::env::current_dir()
                .map(|p| p.to_string_lossy().to_string())
                .unwrap_or_else(|_| ".".to_string())
        });

    // Generate or validate name
    let name = match name {
        Some(n) => {
            validate_name(&n)?;
            n
        }
        None => generate_name(state, &project)?,
    };

    // Check if agent already exists
    if state.agent_exists(&name)? {
        anyhow::bail!("Agent already exists: {}", name);
    }

    // Check agent limit
    if state.at_limit()? {
        anyhow::bail!("Maximum number of agents reached ({})", crate::state::MAX_AGENTS);
    }

    // Get next slot
    let slot = state
        .next_slot()?
        .ok_or_else(|| anyhow::anyhow!("No available slots"))?;

    // Get template if specified
    let template_name = template.as_deref().unwrap_or("default");
    let tpl = template.as_ref().and_then(|t| get_template(t));

    if template.is_some() && tpl.is_none() && template_name != "default" {
        anyhow::bail!("Template not found: {}", template_name);
    }

    let sandbox = tpl
        .as_ref()
        .map(|t| t.sandbox.clone())
        .unwrap_or_else(|| "implement".to_string());

    // Create agent in state
    let agent = state.create_agent(
        name.clone(),
        project.clone(),
        template_name.to_string(),
        sandbox,
        slot,
    )?;

    eprintln!("Spawning agent: {} (slot {})", name, slot);
    eprintln!("Project: {}", project);

    // Build claude command
    let mut claude_cmd = vec!["claude"];

    // Add template args if any
    if let Some(ref tpl) = tpl {
        for arg in &tpl.claude_args {
            claude_cmd.push(arg);
        }
    }

    let data_dir = state.data_dir().to_string_lossy().to_string();

    // Create tmux session
    tmux::create_session(
        &agent.tmux_session,
        &project,
        &claude_cmd,
        &name,
        &data_dir,
    )?;

    // Set slot env var
    tmux::set_environment(&agent.tmux_session, "DAEDALOS_AGENT_SLOT", &slot.to_string())?;

    // Get PID and update
    std::thread::sleep(std::time::Duration::from_millis(500));
    if let Ok(Some(pid)) = tmux::get_pane_pid(&agent.tmux_session) {
        state.update_agent(&name, |a| {
            a.pid = pid;
            a.status = AgentStatus::Active;
        })?;
    }

    // Send initial prompt if provided
    if let Some(prompt) = initial_prompt {
        std::thread::sleep(std::time::Duration::from_secs(1));
        let full_prompt = if let Some(ref tpl) = tpl {
            format!("{}{}", tpl.prompt_prefix, prompt)
        } else {
            prompt
        };
        tmux::send_keys(&agent.tmux_session, &full_prompt)?;
        tmux::send_keys(&agent.tmux_session, "Enter")?;
    }

    eprintln!("Agent spawned: {} (slot {})", name, slot);

    // Focus unless told not to
    if !no_focus {
        tmux::focus_session(&agent.tmux_session)?;
    }

    Ok(())
}

/// List all agents
pub fn list(state: &AgentState, as_json: bool, quiet: bool) -> Result<()> {
    let agents = state.list_agents()?;

    if as_json {
        let json = serde_json::to_string_pretty(&agents)?;
        println!("{}", json);
        return Ok(());
    }

    if quiet {
        for agent in agents {
            println!("{}", agent.name);
        }
        return Ok(());
    }

    if agents.is_empty() {
        println!("No agents running.");
        return Ok(());
    }

    // Print table header
    println!(
        "{:>4}  {:<16}  {:<12}  {:<24}  {:>8}",
        "SLOT", "NAME", "STATUS", "PROJECT", "UPTIME"
    );
    println!("{}", "-".repeat(76));

    for agent in agents {
        // Check if tmux session actually exists
        let actual_status = if tmux::session_exists(&agent.tmux_session) {
            agent.status
        } else {
            AgentStatus::Dead
        };

        let uptime = format_duration(Utc::now() - agent.created);
        let project_short = if agent.project.len() > 24 {
            format!("...{}", &agent.project[agent.project.len() - 21..])
        } else {
            agent.project.clone()
        };

        println!(
            "{:>4}  {:<16}  {:<12}  {:<24}  {:>8}",
            agent.slot, agent.name, actual_status, project_short, uptime
        );
    }

    Ok(())
}

/// Focus an agent
pub fn focus(state: &AgentState, identifier: &str) -> Result<()> {
    let agent = state
        .resolve_agent(identifier)?
        .ok_or_else(|| anyhow::anyhow!("Agent not found: {}", identifier))?;

    if !tmux::session_exists(&agent.tmux_session) {
        anyhow::bail!("Session does not exist for agent: {}", agent.name);
    }

    tmux::focus_session(&agent.tmux_session)?;
    Ok(())
}

/// Kill an agent
pub fn kill(state: &AgentState, identifier: &str, force: bool) -> Result<()> {
    let agent = state
        .resolve_agent(identifier)?
        .ok_or_else(|| anyhow::anyhow!("Agent not found: {}", identifier))?;

    eprintln!("Killing agent: {}", agent.name);

    // Kill tmux session
    tmux::kill_session(&agent.tmux_session, force)?;

    // Remove from database
    state.delete_agent(&agent.name)?;

    eprintln!("Agent killed: {}", agent.name);
    Ok(())
}

/// Kill all agents
pub fn kill_all(state: &AgentState, force: bool) -> Result<()> {
    let agents = state.list_agents()?;

    if agents.is_empty() {
        println!("No agents to kill.");
        return Ok(());
    }

    for agent in agents {
        eprintln!("Killing agent: {}", agent.name);
        let _ = tmux::kill_session(&agent.tmux_session, force);
        let _ = state.delete_agent(&agent.name);
    }

    eprintln!("All agents killed.");
    Ok(())
}

/// Show agent status
pub fn status(state: &AgentState, identifier: Option<&str>, as_json: bool) -> Result<()> {
    match identifier {
        Some(id) => {
            let agent = state
                .resolve_agent(id)?
                .ok_or_else(|| anyhow::anyhow!("Agent not found: {}", id))?;

            if as_json {
                let json = serde_json::to_string_pretty(&agent)?;
                println!("{}", json);
            } else {
                // Check if session exists
                let session_alive = tmux::session_exists(&agent.tmux_session);
                let actual_status = if session_alive {
                    agent.status
                } else {
                    AgentStatus::Dead
                };

                println!("Agent: {}", agent.name);
                println!("  Slot: {}", agent.slot);
                println!("  Status: {}", actual_status);
                println!("  Project: {}", agent.project);
                println!("  Template: {}", agent.template);
                println!("  Session: {}", agent.tmux_session);
                println!("  PID: {}", agent.pid);
                println!("  Created: {}", agent.created);
                println!("  Last Activity: {}", agent.last_activity);

                // Show pending messages
                let messaging = Messaging::new(state);
                let pending = messaging.pending_count(&agent.name)?;
                if pending > 0 {
                    println!("  Pending Messages: {}", pending);
                }
            }
        }
        None => {
            list(state, as_json, false)?;
        }
    }

    Ok(())
}

/// Send a message to an agent
pub fn send(state: &AgentState, to: &str, message: &str) -> Result<()> {
    // Determine sender identity
    let from = std::env::var("DAEDALOS_AGENT_NAME").unwrap_or_else(|_| "anonymous".to_string());

    let messaging = Messaging::new(state);
    let msg_id = messaging.send(to, &from, MessageType::Message, message)?;

    eprintln!("Message sent to: {} (from: {})", to, from);
    eprintln!("Message ID: {}", msg_id);

    Ok(())
}

/// Check agent inbox
pub fn inbox(state: &AgentState, agent: Option<&str>, show_all: bool, as_json: bool) -> Result<()> {
    // Determine agent identity
    let agent_name = agent
        .map(|s| s.to_string())
        .or_else(|| std::env::var("DAEDALOS_AGENT_NAME").ok())
        .ok_or_else(|| anyhow::anyhow!("Could not determine agent identity. Use: agent inbox <name>"))?;

    let messaging = Messaging::new(state);
    let messages = messaging.inbox(&agent_name, show_all)?;

    let output = messaging::format_messages(&messages, as_json);
    println!("{}", output);

    // Mark as read if not showing as JSON
    if !as_json && !messages.is_empty() {
        messaging.mark_read(&agent_name, None)?;
    }

    Ok(())
}

/// Broadcast a message to all agents
pub fn broadcast(state: &AgentState, message: &str) -> Result<()> {
    let from = std::env::var("DAEDALOS_AGENT_NAME").unwrap_or_else(|_| "anonymous".to_string());

    let messaging = Messaging::new(state);
    let count = messaging.broadcast(&from, message)?;

    eprintln!("Broadcast sent to {} agents", count);
    Ok(())
}

/// Show agent logs
pub fn logs(state: &AgentState, identifier: &str, lines: u32, follow: bool) -> Result<()> {
    let agent = state
        .resolve_agent(identifier)?
        .ok_or_else(|| anyhow::anyhow!("Agent not found: {}", identifier))?;

    if !tmux::session_exists(&agent.tmux_session) {
        anyhow::bail!("Session does not exist for agent: {}", agent.name);
    }

    if follow {
        // Follow logs in a loop
        loop {
            print!("\x1B[2J\x1B[1;1H"); // Clear screen
            println!("Logs: {} (Ctrl+C to exit)\n", agent.name);
            let content = tmux::get_pane_content(&agent.tmux_session, lines)?;
            print!("{}", content);
            std::thread::sleep(std::time::Duration::from_secs(1));
        }
    } else {
        let content = tmux::get_pane_content(&agent.tmux_session, lines)?;
        print!("{}", content);
    }

    Ok(())
}

/// Pause an agent
pub fn pause(state: &AgentState, identifier: &str) -> Result<()> {
    let agent = state
        .resolve_agent(identifier)?
        .ok_or_else(|| anyhow::anyhow!("Agent not found: {}", identifier))?;

    if !tmux::session_exists(&agent.tmux_session) {
        anyhow::bail!("Session does not exist for agent: {}", agent.name);
    }

    tmux::pause_process(&agent.tmux_session)?;
    state.update_agent(&agent.name, |a| a.status = AgentStatus::Paused)?;

    eprintln!("Agent paused: {}", agent.name);
    Ok(())
}

/// Resume an agent
pub fn resume(state: &AgentState, identifier: &str) -> Result<()> {
    let agent = state
        .resolve_agent(identifier)?
        .ok_or_else(|| anyhow::anyhow!("Agent not found: {}", identifier))?;

    if !tmux::session_exists(&agent.tmux_session) {
        anyhow::bail!("Session does not exist for agent: {}", agent.name);
    }

    tmux::resume_process(&agent.tmux_session)?;
    state.update_agent(&agent.name, |a| a.status = AgentStatus::Active)?;

    eprintln!("Agent resumed: {}", agent.name);
    Ok(())
}

/// Search agent logs
pub fn search(
    state: &AgentState,
    query: &str,
    agent: Option<&str>,
    ignore_case: bool,
    as_json: bool,
) -> Result<()> {
    let agents = match agent {
        Some(id) => {
            vec![state
                .resolve_agent(id)?
                .ok_or_else(|| anyhow::anyhow!("Agent not found: {}", id))?]
        }
        None => state.list_agents()?,
    };

    #[derive(serde::Serialize)]
    struct SearchResult {
        agent: String,
        line_num: usize,
        content: String,
    }

    let mut results: Vec<SearchResult> = Vec::new();

    for ag in agents {
        if !tmux::session_exists(&ag.tmux_session) {
            continue;
        }

        let content = tmux::get_pane_content(&ag.tmux_session, 10000)?;
        for (i, line) in content.lines().enumerate() {
            let matches = if ignore_case {
                line.to_lowercase().contains(&query.to_lowercase())
            } else {
                line.contains(query)
            };

            if matches {
                results.push(SearchResult {
                    agent: ag.name.clone(),
                    line_num: i + 1,
                    content: line.to_string(),
                });
            }
        }
    }

    if as_json {
        println!("{}", serde_json::to_string_pretty(&results)?);
    } else {
        if results.is_empty() {
            println!("No matches found.");
            return Ok(());
        }

        for result in results {
            println!("{}:{}: {}", result.agent, result.line_num, result.content);
        }
    }

    Ok(())
}

/// List available templates
pub fn templates_list(as_json: bool) -> Result<()> {
    let templates = list_templates();

    if as_json {
        println!("{}", serde_json::to_string_pretty(&templates)?);
    } else {
        println!("{:<14}  {}", "NAME", "DESCRIPTION");
        println!("{}", "-".repeat(60));
        for t in templates {
            println!("{:<14}  {}", t.name, t.description);
        }
    }

    Ok(())
}

/// Show template details
pub fn templates_show(name: &str) -> Result<()> {
    let template = get_template(name).ok_or_else(|| anyhow::anyhow!("Template not found: {}", name))?;

    println!("Template: {}", template.name);
    println!("Description: {}", template.description);
    println!("Sandbox: {}", template.sandbox);

    if !template.claude_args.is_empty() {
        println!("Claude Args: {}", template.claude_args.join(" "));
    }

    if !template.allowed_tools.is_empty() {
        println!("Allowed Tools: {}", template.allowed_tools.join(", "));
    }

    if !template.denied_tools.is_empty() {
        println!("Denied Tools: {}", template.denied_tools.join(", "));
    }

    if !template.system_prompt.is_empty() {
        println!("\nSystem Prompt:");
        println!("{}", template.system_prompt);
    }

    Ok(())
}

/// Format a duration as human-readable string
fn format_duration(duration: chrono::Duration) -> String {
    let total_seconds = duration.num_seconds();
    let days = total_seconds / 86400;
    let hours = (total_seconds % 86400) / 3600;
    let minutes = (total_seconds % 3600) / 60;

    if days > 0 {
        format!("{}d {}h", days, hours)
    } else if hours > 0 {
        format!("{}h {}m", hours, minutes)
    } else if minutes > 0 {
        format!("{}m", minutes)
    } else {
        "<1m".to_string()
    }
}
