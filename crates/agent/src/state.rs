//! Agent state management - persistence and CRUD operations
//!
//! Stores agent metadata in JSON files at ~/.local/share/daedalos/agent/

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

/// Maximum number of concurrent agents (mapped to Super+1-9)
pub const MAX_AGENTS: usize = 9;

/// Prefix for tmux session names
pub const TMUX_SESSION_PREFIX: &str = "claude-agent-";

/// Agent metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Agent {
    pub name: String,
    pub slot: u32,
    pub project: String,
    pub template: String,
    pub sandbox: String,
    pub tmux_session: String,
    #[serde(default)]
    pub agent_type: Option<String>,
    pub pid: u32,
    pub created: DateTime<Utc>,
    pub status: AgentStatus,
    pub last_activity: DateTime<Utc>,
}

/// Agent status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AgentStatus {
    Starting,
    Active,
    Thinking,
    Waiting,
    Idle,
    Paused,
    Error,
    Dead,
}

impl std::fmt::Display for AgentStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AgentStatus::Starting => write!(f, "starting"),
            AgentStatus::Active => write!(f, "active"),
            AgentStatus::Thinking => write!(f, "thinking"),
            AgentStatus::Waiting => write!(f, "waiting"),
            AgentStatus::Idle => write!(f, "idle"),
            AgentStatus::Paused => write!(f, "paused"),
            AgentStatus::Error => write!(f, "error"),
            AgentStatus::Dead => write!(f, "dead"),
        }
    }
}

/// Top-level agents database
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentsDb {
    pub agents: HashMap<String, Agent>,
    pub next_slot: u32,
    pub max_slots: u32,
}

impl Default for AgentsDb {
    fn default() -> Self {
        Self {
            agents: HashMap::new(),
            next_slot: 1,
            max_slots: MAX_AGENTS as u32,
        }
    }
}

/// Agent state manager
pub struct AgentState {
    data_dir: PathBuf,
    #[allow(dead_code)]
    config_dir: PathBuf,
}

impl AgentState {
    /// Create a new state manager
    pub fn new() -> Result<Self> {
        let data_dir = dirs::data_dir()
            .unwrap_or_else(|| PathBuf::from("~/.local/share"))
            .join("daedalos/agent");

        let config_dir = dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from("~/.config"))
            .join("daedalos/agent");

        // Ensure directories exist
        fs::create_dir_all(&data_dir)?;
        fs::create_dir_all(&config_dir)?;
        fs::create_dir_all(data_dir.join("messages"))?;
        fs::create_dir_all(data_dir.join("signals"))?;
        fs::create_dir_all(data_dir.join("shared"))?;
        fs::create_dir_all(config_dir.join("templates"))?;

        Ok(Self { data_dir, config_dir })
    }

    /// Path to the agents database file
    pub fn agents_file(&self) -> PathBuf {
        self.data_dir.join("agents.json")
    }

    /// Path to messages directory
    pub fn messages_dir(&self) -> PathBuf {
        self.data_dir.join("messages")
    }

    /// Path to an agent's message queue
    pub fn message_queue(&self, agent: &str) -> PathBuf {
        self.messages_dir().join(format!("{}.jsonl", agent))
    }

    /// Path to signals directory
    #[allow(dead_code)]
    pub fn signals_dir(&self) -> PathBuf {
        self.data_dir.join("signals")
    }

    /// Data directory
    pub fn data_dir(&self) -> &PathBuf {
        &self.data_dir
    }

    /// Load the agents database
    pub fn load_db(&self) -> Result<AgentsDb> {
        let path = self.agents_file();
        if !path.exists() {
            return Ok(AgentsDb::default());
        }

        let content = fs::read_to_string(&path)
            .context("Failed to read agents database")?;

        serde_json::from_str(&content)
            .context("Failed to parse agents database")
    }

    /// Save the agents database
    pub fn save_db(&self, db: &AgentsDb) -> Result<()> {
        let path = self.agents_file();
        let content = serde_json::to_string_pretty(db)?;
        fs::write(&path, content)
            .context("Failed to write agents database")?;
        Ok(())
    }

    /// Get tmux session name for an agent
    pub fn session_name(agent_name: &str) -> String {
        format!("{}{}", TMUX_SESSION_PREFIX, agent_name)
    }

    /// List all agents
    pub fn list_agents(&self) -> Result<Vec<Agent>> {
        let db = self.load_db()?;
        let mut agents: Vec<_> = db.agents.values().cloned().collect();
        agents.sort_by_key(|a| a.slot);
        Ok(agents)
    }

    /// Get an agent by name
    pub fn get_agent(&self, name: &str) -> Result<Option<Agent>> {
        let db = self.load_db()?;
        Ok(db.agents.get(name).cloned())
    }

    /// Get an agent by slot number
    pub fn get_agent_by_slot(&self, slot: u32) -> Result<Option<Agent>> {
        let db = self.load_db()?;
        Ok(db.agents.values().find(|a| a.slot == slot).cloned())
    }

    /// Resolve agent identifier (name or slot number) to agent
    pub fn resolve_agent(&self, identifier: &str) -> Result<Option<Agent>> {
        // Try slot number first
        if let Ok(slot) = identifier.parse::<u32>() {
            if let Some(agent) = self.get_agent_by_slot(slot)? {
                return Ok(Some(agent));
            }
        }

        // Try exact name match
        if let Some(agent) = self.get_agent(identifier)? {
            return Ok(Some(agent));
        }

        // Try fuzzy match (prefix)
        let db = self.load_db()?;
        for (name, agent) in &db.agents {
            if name.starts_with(identifier) {
                return Ok(Some(agent.clone()));
            }
        }

        // Try contains match
        for (name, agent) in &db.agents {
            if name.contains(identifier) {
                return Ok(Some(agent.clone()));
            }
        }

        Ok(None)
    }

    /// Check if agent exists
    pub fn agent_exists(&self, name: &str) -> Result<bool> {
        let db = self.load_db()?;
        Ok(db.agents.contains_key(name))
    }

    /// Get next available slot
    pub fn next_slot(&self) -> Result<Option<u32>> {
        let db = self.load_db()?;
        let used_slots: Vec<u32> = db.agents.values().map(|a| a.slot).collect();

        for slot in 1..=db.max_slots {
            if !used_slots.contains(&slot) {
                return Ok(Some(slot));
            }
        }

        Ok(None)
    }

    /// Check if at agent limit
    pub fn at_limit(&self) -> Result<bool> {
        let db = self.load_db()?;
        Ok(db.agents.len() >= db.max_slots as usize)
    }

    /// Create a new agent
    pub fn create_agent(
        &self,
        name: String,
        project: String,
        template: String,
        sandbox: String,
        slot: u32,
    ) -> Result<Agent> {
        let mut db = self.load_db()?;

        if db.agents.contains_key(&name) {
            anyhow::bail!("Agent already exists: {}", name);
        }

        let now = Utc::now();
        let agent = Agent {
            name: name.clone(),
            slot,
            project,
            template,
            sandbox,
            tmux_session: Self::session_name(&name),
            agent_type: None,
            pid: 0,
            created: now,
            status: AgentStatus::Starting,
            last_activity: now,
        };

        db.agents.insert(name, agent.clone());
        self.save_db(&db)?;

        Ok(agent)
    }

    /// Update agent field
    pub fn update_agent(&self, name: &str, update: impl FnOnce(&mut Agent)) -> Result<()> {
        let mut db = self.load_db()?;

        if let Some(agent) = db.agents.get_mut(name) {
            update(agent);
            self.save_db(&db)?;
        }

        Ok(())
    }

    /// Delete an agent
    pub fn delete_agent(&self, name: &str) -> Result<()> {
        let mut db = self.load_db()?;
        db.agents.remove(name);
        self.save_db(&db)?;
        Ok(())
    }

    /// Get agent count
    #[allow(dead_code)]
    pub fn agent_count(&self) -> Result<usize> {
        let db = self.load_db()?;
        Ok(db.agents.len())
    }
}

/// Validate agent name
pub fn validate_name(name: &str) -> Result<()> {
    if name.is_empty() {
        anyhow::bail!("Agent name cannot be empty");
    }

    let first_char = name.chars().next().unwrap();
    if !first_char.is_ascii_alphabetic() {
        anyhow::bail!("Agent name must start with a letter");
    }

    for c in name.chars() {
        if !c.is_ascii_alphanumeric() && c != '-' && c != '_' {
            anyhow::bail!(
                "Agent name must contain only letters, numbers, hyphens, and underscores"
            );
        }
    }

    if name.len() > 32 {
        anyhow::bail!("Agent name must be 32 characters or less");
    }

    Ok(())
}

/// Generate a unique agent name based on project
pub fn generate_name(state: &AgentState, project: &str) -> Result<String> {
    let base = std::path::Path::new(project)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("agent");

    let mut name = base.to_string();
    let mut counter = 1;

    while state.agent_exists(&name)? {
        name = format!("{}-{}", base, counter);
        counter += 1;
    }

    Ok(name)
}
