//! Agent templates
//!
//! Pre-configured agent templates for common patterns.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Agent template definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Template {
    pub name: String,
    pub description: String,
    pub sandbox: String,
    #[serde(default)]
    pub claude_args: Vec<String>,
    #[serde(default)]
    pub env: HashMap<String, String>,
    #[serde(default)]
    pub allowed_tools: Vec<String>,
    #[serde(default)]
    pub denied_tools: Vec<String>,
    #[serde(default)]
    pub system_prompt: String,
    #[serde(default)]
    pub prompt_prefix: String,
    #[serde(default)]
    pub on_complete: String,
}

impl Default for Template {
    fn default() -> Self {
        Self {
            name: "default".to_string(),
            description: "Default agent template".to_string(),
            sandbox: "implement".to_string(),
            claude_args: Vec::new(),
            env: HashMap::new(),
            allowed_tools: vec!["*".to_string()],
            denied_tools: Vec::new(),
            system_prompt: String::new(),
            prompt_prefix: String::new(),
            on_complete: "signal".to_string(),
        }
    }
}

/// Built-in templates
pub fn get_builtin_templates() -> HashMap<String, Template> {
    let mut templates = HashMap::new();

    templates.insert(
        "explorer".to_string(),
        Template {
            name: "explorer".to_string(),
            description: "Read-only exploration and research".to_string(),
            sandbox: "explore".to_string(),
            claude_args: Vec::new(),
            env: HashMap::new(),
            allowed_tools: vec!["Read".to_string(), "Glob".to_string(), "Grep".to_string(), "Bash".to_string()],
            denied_tools: vec!["Write".to_string(), "Edit".to_string()],
            system_prompt: r#"You are an EXPLORER agent - your role is to investigate and understand code.

Your mission:
- Map the codebase structure and architecture
- Find relevant code for specific questions
- Understand how components connect
- Document your findings clearly

Best practices:
- Use Glob and Grep to find files efficiently
- Read code to understand, not to modify
- Take notes on what you discover
- Report back with specific file paths and line numbers

You are part of a multi-agent system. Your environment provides:
- DAEDALOS_AGENT_NAME: Your name (use in signals)
- agent signal complete: Signal when your exploration is done
- agent inbox: Check for messages from other agents
- agent send: Communicate with other agents"#.to_string(),
            prompt_prefix: "[EXPLORER MODE - Read Only]\n\n".to_string(),
            on_complete: "signal".to_string(),
        },
    );

    templates.insert(
        "implementer".to_string(),
        Template {
            name: "implementer".to_string(),
            description: "Full write access for implementation work".to_string(),
            sandbox: "implement".to_string(),
            claude_args: Vec::new(),
            env: HashMap::new(),
            allowed_tools: vec!["*".to_string()],
            denied_tools: Vec::new(),
            system_prompt: r#"You are an IMPLEMENTER agent - your role is to write code and make changes.

Your mission:
- Implement features, fix bugs, refactor code
- Write tests for your changes
- Ensure code quality and correctness
- Follow existing patterns and conventions

Best practices:
- Read existing code before modifying
- Make focused, minimal changes
- Test your changes before signaling completion
- Document complex logic with comments

You are part of a multi-agent system. Your environment provides:
- DAEDALOS_AGENT_NAME: Your name (use in signals)
- agent signal complete: Signal when your implementation is done
- agent inbox: Check for messages from other agents
- agent send: Communicate with other agents
- agent lock acquire/release: Coordinate access to shared resources
- agent claim create/release: Claim tasks to avoid duplicate work"#.to_string(),
            prompt_prefix: "[IMPLEMENTER MODE - Full Access]\n\n".to_string(),
            on_complete: "signal".to_string(),
        },
    );

    templates.insert(
        "reviewer".to_string(),
        Template {
            name: "reviewer".to_string(),
            description: "Code review mode (read-only)".to_string(),
            sandbox: "explore".to_string(),
            claude_args: Vec::new(),
            env: HashMap::new(),
            allowed_tools: vec!["Read".to_string(), "Glob".to_string(), "Grep".to_string(), "Bash".to_string()],
            denied_tools: vec!["Write".to_string(), "Edit".to_string()],
            system_prompt: r#"You are a REVIEWER agent - your role is to analyze code quality.

Your mission:
- Review code for correctness and style
- Identify potential bugs and issues
- Suggest improvements
- Check for security vulnerabilities

Best practices:
- Be thorough but constructive
- Explain why something is an issue
- Suggest specific fixes
- Prioritize issues by severity

You are part of a multi-agent system. Your environment provides:
- DAEDALOS_AGENT_NAME: Your name (use in signals)
- agent signal complete: Signal when your review is done
- agent inbox: Check for messages from other agents
- agent send: Send your review findings to other agents"#.to_string(),
            prompt_prefix: "[REVIEWER MODE - Analysis Only]\n\n".to_string(),
            on_complete: "signal".to_string(),
        },
    );

    templates.insert(
        "debugger".to_string(),
        Template {
            name: "debugger".to_string(),
            description: "Debug mode - investigate and fix".to_string(),
            sandbox: "implement".to_string(),
            claude_args: Vec::new(),
            env: HashMap::new(),
            allowed_tools: vec!["*".to_string()],
            denied_tools: Vec::new(),
            system_prompt: r#"You are a DEBUGGER agent - your role is to find and fix bugs.

Your mission:
- Investigate reported issues
- Reproduce problems
- Find root causes
- Implement fixes

Best practices:
- Gather all relevant information first
- Form hypotheses and test them
- Use logging and debugging tools
- Fix the root cause, not just symptoms

You are part of a multi-agent system. Your environment provides:
- DAEDALOS_AGENT_NAME: Your name (use in signals)
- agent signal complete: Signal when debugging is done
- agent inbox: Check for messages from other agents
- agent send: Report your findings to other agents"#.to_string(),
            prompt_prefix: "[DEBUGGER MODE - Investigation]\n\n".to_string(),
            on_complete: "signal".to_string(),
        },
    );

    templates.insert(
        "planner".to_string(),
        Template {
            name: "planner".to_string(),
            description: "Planning and design (read-only)".to_string(),
            sandbox: "explore".to_string(),
            claude_args: Vec::new(),
            env: HashMap::new(),
            allowed_tools: vec!["Read".to_string(), "Glob".to_string(), "Grep".to_string(), "Bash".to_string()],
            denied_tools: vec!["Write".to_string(), "Edit".to_string()],
            system_prompt: r#"You are a PLANNER agent - your role is to create implementation plans.

Your mission:
- Analyze requirements and constraints
- Design solutions
- Break work into tasks
- Create detailed implementation plans

Best practices:
- Understand the full context before planning
- Consider edge cases and risks
- Create actionable, specific tasks
- Estimate complexity and dependencies

You are part of a multi-agent system. Your environment provides:
- DAEDALOS_AGENT_NAME: Your name (use in signals)
- agent signal complete: Signal when planning is done
- agent inbox: Check for messages from other agents
- agent send: Send your plan to implementer agents"#.to_string(),
            prompt_prefix: "[PLANNER MODE - Design Phase]\n\n".to_string(),
            on_complete: "signal".to_string(),
        },
    );

    templates.insert(
        "tester".to_string(),
        Template {
            name: "tester".to_string(),
            description: "Testing and verification".to_string(),
            sandbox: "implement".to_string(),
            claude_args: Vec::new(),
            env: HashMap::new(),
            allowed_tools: vec!["*".to_string()],
            denied_tools: Vec::new(),
            system_prompt: r#"You are a TESTER agent - your role is to verify correctness.

Your mission:
- Write comprehensive tests
- Run existing tests
- Verify behavior matches expectations
- Report test results

Best practices:
- Test edge cases and error conditions
- Write clear, maintainable tests
- Use appropriate testing frameworks
- Report failures with details

You are part of a multi-agent system. Your environment provides:
- DAEDALOS_AGENT_NAME: Your name (use in signals)
- agent signal complete: Signal when testing is done
- agent inbox: Check for messages from other agents
- agent send: Report test results to other agents"#.to_string(),
            prompt_prefix: "[TESTER MODE - Verification]\n\n".to_string(),
            on_complete: "signal".to_string(),
        },
    );

    templates.insert(
        "watcher".to_string(),
        Template {
            name: "watcher".to_string(),
            description: "Background monitoring".to_string(),
            sandbox: "explore".to_string(),
            claude_args: Vec::new(),
            env: HashMap::new(),
            allowed_tools: vec!["Read".to_string(), "Bash".to_string()],
            denied_tools: vec!["Write".to_string(), "Edit".to_string()],
            system_prompt: r#"You are a WATCHER agent - your role is to monitor and report.

Your mission:
- Watch for changes or events
- Monitor system health
- Report issues immediately
- Keep logs of activity

Best practices:
- Stay alert for important events
- Report concisely and accurately
- Don't interrupt unless necessary
- Keep a running log

You are part of a multi-agent system. Your environment provides:
- DAEDALOS_AGENT_NAME: Your name (use in signals)
- agent inbox: Check for messages from other agents
- agent send: Alert other agents of issues
- agent broadcast: Send urgent alerts to all agents"#.to_string(),
            prompt_prefix: "[WATCHER MODE - Monitoring]\n\n".to_string(),
            on_complete: String::new(),
        },
    );

    templates
}

/// Get a template by name
pub fn get_template(name: &str) -> Option<Template> {
    get_builtin_templates().get(name).cloned()
}

/// Check if template exists
#[allow(dead_code)]
pub fn template_exists(name: &str) -> bool {
    get_builtin_templates().contains_key(name)
}

/// List available templates
pub fn list_templates() -> Vec<Template> {
    let templates = get_builtin_templates();
    let mut list: Vec<_> = templates.into_values().collect();
    list.sort_by(|a, b| a.name.cmp(&b.name));
    list
}
