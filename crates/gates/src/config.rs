//! Gates configuration management
//!
//! Handles supervision levels, gate actions, and configuration loading/saving.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

/// Supervision levels from most autonomous to most manual
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SupervisionLevel {
    /// AI runs freely, only catastrophic actions gated
    Autonomous,
    /// AI runs, human gets notifications, can intervene
    Supervised,
    /// AI proposes, human approves major actions
    Collaborative,
    /// Human drives, AI suggests and helps
    Assisted,
    /// AI only responds to direct commands
    Manual,
}

impl SupervisionLevel {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Autonomous => "autonomous",
            Self::Supervised => "supervised",
            Self::Collaborative => "collaborative",
            Self::Assisted => "assisted",
            Self::Manual => "manual",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "autonomous" => Some(Self::Autonomous),
            "supervised" => Some(Self::Supervised),
            "collaborative" => Some(Self::Collaborative),
            "assisted" => Some(Self::Assisted),
            "manual" => Some(Self::Manual),
            _ => None,
        }
    }

    /// Index in the strictness ordering (higher = more restrictive)
    pub fn strictness(&self) -> u8 {
        match self {
            Self::Autonomous => 0,
            Self::Supervised => 1,
            Self::Collaborative => 2,
            Self::Assisted => 3,
            Self::Manual => 4,
        }
    }

    pub fn all() -> &'static [Self] {
        &[
            Self::Autonomous,
            Self::Supervised,
            Self::Collaborative,
            Self::Assisted,
            Self::Manual,
        ]
    }
}

impl Default for SupervisionLevel {
    fn default() -> Self {
        Self::Supervised
    }
}

impl std::fmt::Display for SupervisionLevel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// Gate actions
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum GateAction {
    /// Proceed without asking
    Allow,
    /// Notify but don't block
    Notify,
    /// Require explicit approval
    Approve,
    /// Always deny
    Deny,
}

impl GateAction {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Allow => "allow",
            Self::Notify => "notify",
            Self::Approve => "approve",
            Self::Deny => "deny",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "allow" => Some(Self::Allow),
            "notify" => Some(Self::Notify),
            "approve" => Some(Self::Approve),
            "deny" => Some(Self::Deny),
            _ => None,
        }
    }

    /// Strictness level (higher = more restrictive)
    pub fn strictness(&self) -> u8 {
        match self {
            Self::Allow => 0,
            Self::Notify => 1,
            Self::Approve => 2,
            Self::Deny => 3,
        }
    }

    pub fn all() -> &'static [Self] {
        &[Self::Allow, Self::Notify, Self::Approve, Self::Deny]
    }
}

impl std::fmt::Display for GateAction {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// Known gate types
#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GateType {
    FileDelete,
    FileCreate,
    FileModify,
    GitCommit,
    GitPush,
    GitForcePush,
    LoopStart,
    AgentSpawn,
    ShellCommand,
    SensitiveFile,
}

#[allow(dead_code)]
impl GateType {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::FileDelete => "file_delete",
            Self::FileCreate => "file_create",
            Self::FileModify => "file_modify",
            Self::GitCommit => "git_commit",
            Self::GitPush => "git_push",
            Self::GitForcePush => "git_force_push",
            Self::LoopStart => "loop_start",
            Self::AgentSpawn => "agent_spawn",
            Self::ShellCommand => "shell_command",
            Self::SensitiveFile => "sensitive_file",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "file_delete" => Some(Self::FileDelete),
            "file_create" => Some(Self::FileCreate),
            "file_modify" => Some(Self::FileModify),
            "git_commit" => Some(Self::GitCommit),
            "git_push" => Some(Self::GitPush),
            "git_force_push" => Some(Self::GitForcePush),
            "loop_start" => Some(Self::LoopStart),
            "agent_spawn" => Some(Self::AgentSpawn),
            "shell_command" => Some(Self::ShellCommand),
            "sensitive_file" => Some(Self::SensitiveFile),
            _ => None,
        }
    }

    pub fn all() -> &'static [Self] {
        &[
            Self::FileDelete,
            Self::FileCreate,
            Self::FileModify,
            Self::GitCommit,
            Self::GitPush,
            Self::GitForcePush,
            Self::LoopStart,
            Self::AgentSpawn,
            Self::ShellCommand,
            Self::SensitiveFile,
        ]
    }

    pub fn description(&self) -> &'static str {
        match self {
            Self::FileDelete => "Deleting files",
            Self::FileCreate => "Creating new files",
            Self::FileModify => "Modifying existing files",
            Self::GitCommit => "Making git commits",
            Self::GitPush => "Pushing to remote",
            Self::GitForcePush => "Force pushing (dangerous)",
            Self::LoopStart => "Starting iteration loops",
            Self::AgentSpawn => "Spawning new agents",
            Self::ShellCommand => "Running shell commands",
            Self::SensitiveFile => "Modifying sensitive files (secrets, env, keys)",
        }
    }
}

impl std::fmt::Display for GateType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// Default gate actions for each supervision level
pub fn default_gates_for_level(level: SupervisionLevel) -> HashMap<String, GateAction> {
    let mut gates = HashMap::new();

    match level {
        SupervisionLevel::Autonomous => {
            gates.insert("file_delete".into(), GateAction::Notify);
            gates.insert("file_create".into(), GateAction::Allow);
            gates.insert("file_modify".into(), GateAction::Allow);
            gates.insert("git_commit".into(), GateAction::Allow);
            gates.insert("git_push".into(), GateAction::Notify);
            gates.insert("git_force_push".into(), GateAction::Approve);
            gates.insert("loop_start".into(), GateAction::Allow);
            gates.insert("agent_spawn".into(), GateAction::Allow);
            gates.insert("shell_command".into(), GateAction::Allow);
            gates.insert("sensitive_file".into(), GateAction::Approve);
        }
        SupervisionLevel::Supervised => {
            gates.insert("file_delete".into(), GateAction::Approve);
            gates.insert("file_create".into(), GateAction::Notify);
            gates.insert("file_modify".into(), GateAction::Notify);
            gates.insert("git_commit".into(), GateAction::Notify);
            gates.insert("git_push".into(), GateAction::Approve);
            gates.insert("git_force_push".into(), GateAction::Deny);
            gates.insert("loop_start".into(), GateAction::Notify);
            gates.insert("agent_spawn".into(), GateAction::Notify);
            gates.insert("shell_command".into(), GateAction::Notify);
            gates.insert("sensitive_file".into(), GateAction::Approve);
        }
        SupervisionLevel::Collaborative => {
            gates.insert("file_delete".into(), GateAction::Approve);
            gates.insert("file_create".into(), GateAction::Approve);
            gates.insert("file_modify".into(), GateAction::Notify);
            gates.insert("git_commit".into(), GateAction::Approve);
            gates.insert("git_push".into(), GateAction::Approve);
            gates.insert("git_force_push".into(), GateAction::Deny);
            gates.insert("loop_start".into(), GateAction::Approve);
            gates.insert("agent_spawn".into(), GateAction::Approve);
            gates.insert("shell_command".into(), GateAction::Approve);
            gates.insert("sensitive_file".into(), GateAction::Approve);
        }
        SupervisionLevel::Assisted => {
            gates.insert("file_delete".into(), GateAction::Approve);
            gates.insert("file_create".into(), GateAction::Approve);
            gates.insert("file_modify".into(), GateAction::Approve);
            gates.insert("git_commit".into(), GateAction::Approve);
            gates.insert("git_push".into(), GateAction::Approve);
            gates.insert("git_force_push".into(), GateAction::Deny);
            gates.insert("loop_start".into(), GateAction::Approve);
            gates.insert("agent_spawn".into(), GateAction::Approve);
            gates.insert("shell_command".into(), GateAction::Approve);
            gates.insert("sensitive_file".into(), GateAction::Approve);
        }
        SupervisionLevel::Manual => {
            gates.insert("file_delete".into(), GateAction::Approve);
            gates.insert("file_create".into(), GateAction::Approve);
            gates.insert("file_modify".into(), GateAction::Approve);
            gates.insert("git_commit".into(), GateAction::Approve);
            gates.insert("git_push".into(), GateAction::Approve);
            gates.insert("git_force_push".into(), GateAction::Deny);
            gates.insert("loop_start".into(), GateAction::Approve);
            gates.insert("agent_spawn".into(), GateAction::Approve);
            gates.insert("shell_command".into(), GateAction::Approve);
            gates.insert("sensitive_file".into(), GateAction::Approve);
        }
    }

    gates
}

/// Default sensitive path patterns
pub fn default_sensitive_paths() -> Vec<String> {
    vec![
        "*.env".into(),
        "*.env.*".into(),
        ".env*".into(),
        "**/secrets/**".into(),
        "**/credentials/**".into(),
        "**/.ssh/**".into(),
        "**/id_rsa*".into(),
        "**/*.pem".into(),
        "**/*.key".into(),
    ]
}

/// Autonomy limits configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AutonomyLimits {
    pub max_iterations: u32,
    pub max_file_changes: u32,
    pub max_lines_changed: u32,
    pub sensitive_paths: Vec<String>,
}

impl Default for AutonomyLimits {
    fn default() -> Self {
        Self {
            max_iterations: 50,
            max_file_changes: 100,
            max_lines_changed: 1000,
            sensitive_paths: default_sensitive_paths(),
        }
    }
}

/// Supervision configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SupervisionConfig {
    pub level: SupervisionLevel,
    pub gates: HashMap<String, GateAction>,
    pub autonomy: AutonomyLimits,
    #[serde(default)]
    pub overrides: HashMap<String, GateAction>,
}

impl Default for SupervisionConfig {
    fn default() -> Self {
        let level = SupervisionLevel::default();
        Self {
            gates: default_gates_for_level(level),
            level,
            autonomy: AutonomyLimits::default(),
            overrides: HashMap::new(),
        }
    }
}

impl SupervisionConfig {
    /// Create a new config with the given level and default gates
    pub fn with_level(level: SupervisionLevel) -> Self {
        Self {
            level,
            gates: default_gates_for_level(level),
            autonomy: AutonomyLimits::default(),
            overrides: HashMap::new(),
        }
    }

    /// Get the action for a gate, checking overrides first
    pub fn get_gate(&self, gate: &str) -> GateAction {
        if let Some(&action) = self.overrides.get(gate) {
            return action;
        }
        self.gates.get(gate).copied().unwrap_or(GateAction::Approve)
    }

    /// Check if a path matches any sensitive path pattern
    pub fn is_sensitive_path(&self, path: &str) -> bool {
        let path_str = path;
        let filename = Path::new(path)
            .file_name()
            .map(|f| f.to_string_lossy().to_string())
            .unwrap_or_default();

        for pattern in &self.autonomy.sensitive_paths {
            if glob::Pattern::new(pattern)
                .map(|p| p.matches(path_str) || p.matches(&filename))
                .unwrap_or(false)
            {
                return true;
            }
        }
        false
    }

    /// Convert to a JSON-serializable representation
    pub fn to_json_value(&self) -> serde_json::Value {
        let gates: HashMap<String, String> = self
            .gates
            .iter()
            .map(|(k, v)| (k.clone(), v.as_str().to_string()))
            .collect();

        let overrides: HashMap<String, String> = self
            .overrides
            .iter()
            .map(|(k, v)| (k.clone(), v.as_str().to_string()))
            .collect();

        serde_json::json!({
            "level": self.level.as_str(),
            "gates": gates,
            "autonomy": {
                "max_iterations": self.autonomy.max_iterations,
                "max_file_changes": self.autonomy.max_file_changes,
                "max_lines_changed": self.autonomy.max_lines_changed,
                "sensitive_paths": self.autonomy.sensitive_paths,
            },
            "overrides": overrides,
        })
    }
}

/// Get the config directory path
pub fn get_config_dir() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("~/.config"))
        .join("daedalos")
}

/// Get the supervision config file path
pub fn get_config_path() -> PathBuf {
    get_config_dir().join("supervision.json")
}

/// Load supervision config from file
pub fn load_config() -> Result<SupervisionConfig> {
    let config_path = get_config_path();

    if !config_path.exists() {
        return Ok(SupervisionConfig::default());
    }

    let content = fs::read_to_string(&config_path)
        .with_context(|| format!("Failed to read config file: {}", config_path.display()))?;

    // Parse JSON config
    let data: serde_json::Value = serde_json::from_str(&content)
        .with_context(|| format!("Failed to parse config file: {}", config_path.display()))?;

    let level = data
        .get("level")
        .and_then(|v| v.as_str())
        .and_then(SupervisionLevel::from_str)
        .unwrap_or_default();

    let mut gates = default_gates_for_level(level);

    // Apply explicit gate settings
    if let Some(gates_obj) = data.get("gates").and_then(|v| v.as_object()) {
        for (key, value) in gates_obj {
            if let Some(action) = value.as_str().and_then(GateAction::from_str) {
                gates.insert(key.clone(), action);
            }
        }
    }

    let mut autonomy = AutonomyLimits::default();
    if let Some(autonomy_obj) = data.get("autonomy").and_then(|v| v.as_object()) {
        if let Some(v) = autonomy_obj.get("max_iterations").and_then(|v| v.as_u64()) {
            autonomy.max_iterations = v as u32;
        }
        if let Some(v) = autonomy_obj.get("max_file_changes").and_then(|v| v.as_u64()) {
            autonomy.max_file_changes = v as u32;
        }
        if let Some(v) = autonomy_obj.get("max_lines_changed").and_then(|v| v.as_u64()) {
            autonomy.max_lines_changed = v as u32;
        }
        if let Some(paths) = autonomy_obj.get("sensitive_paths").and_then(|v| v.as_array()) {
            autonomy.sensitive_paths = paths
                .iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect();
        }
    }

    let mut overrides = HashMap::new();
    if let Some(overrides_obj) = data.get("overrides").and_then(|v| v.as_object()) {
        for (key, value) in overrides_obj {
            if let Some(action) = value.as_str().and_then(GateAction::from_str) {
                overrides.insert(key.clone(), action);
            }
        }
    }

    Ok(SupervisionConfig {
        level,
        gates,
        autonomy,
        overrides,
    })
}

/// Save supervision config to file
pub fn save_config(config: &SupervisionConfig) -> Result<()> {
    let config_path = get_config_path();
    let config_dir = config_path.parent().unwrap();

    fs::create_dir_all(config_dir)
        .with_context(|| format!("Failed to create config directory: {}", config_dir.display()))?;

    let content = serde_json::to_string_pretty(&config.to_json_value())
        .context("Failed to serialize config")?;

    fs::write(&config_path, content)
        .with_context(|| format!("Failed to write config file: {}", config_path.display()))?;

    Ok(())
}

/// Load config with project-level overrides applied
pub fn load_project_config(project_path: Option<&Path>) -> Result<SupervisionConfig> {
    let mut config = load_config()?;

    let project_dir = project_path
        .map(PathBuf::from)
        .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));

    // Check for project-level config
    let project_config_paths = [
        project_dir.join(".daedalos/supervision.json"),
        project_dir.join(".daedalos/supervision.yaml"),
    ];

    for project_config_path in &project_config_paths {
        if project_config_path.exists() {
            if let Ok(content) = fs::read_to_string(project_config_path) {
                if let Ok(data) = serde_json::from_str::<serde_json::Value>(&content) {
                    // Apply project-level gate overrides (project can only make things stricter)
                    if let Some(gates_obj) = data.get("gates").and_then(|v| v.as_object()) {
                        for (key, value) in gates_obj {
                            if let Some(project_action) = value.as_str().and_then(GateAction::from_str)
                            {
                                let current_action = config.get_gate(key);
                                // Only apply if project is stricter
                                if project_action.strictness() > current_action.strictness() {
                                    config.overrides.insert(key.clone(), project_action);
                                }
                            }
                        }
                    }

                    // Project can require a more restrictive level
                    if let Some(project_level) =
                        data.get("level").and_then(|v| v.as_str()).and_then(SupervisionLevel::from_str)
                    {
                        if project_level.strictness() > config.level.strictness() {
                            config.level = project_level;
                            // Re-apply defaults for new level
                            let new_defaults = default_gates_for_level(project_level);
                            for (key, action) in new_defaults {
                                if !config.overrides.contains_key(&key) {
                                    config.gates.insert(key, action);
                                }
                            }
                        }
                    }
                }
            }
            break;
        }
    }

    Ok(config)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_supervision_level_ordering() {
        assert!(SupervisionLevel::Autonomous.strictness() < SupervisionLevel::Supervised.strictness());
        assert!(SupervisionLevel::Supervised.strictness() < SupervisionLevel::Collaborative.strictness());
        assert!(SupervisionLevel::Collaborative.strictness() < SupervisionLevel::Assisted.strictness());
        assert!(SupervisionLevel::Assisted.strictness() < SupervisionLevel::Manual.strictness());
    }

    #[test]
    fn test_gate_action_ordering() {
        assert!(GateAction::Allow.strictness() < GateAction::Notify.strictness());
        assert!(GateAction::Notify.strictness() < GateAction::Approve.strictness());
        assert!(GateAction::Approve.strictness() < GateAction::Deny.strictness());
    }

    #[test]
    fn test_sensitive_path_detection() {
        let config = SupervisionConfig::default();
        assert!(config.is_sensitive_path(".env"));
        assert!(config.is_sensitive_path("config/.env.production"));
        assert!(config.is_sensitive_path("/path/to/secrets/api_key"));
        assert!(!config.is_sensitive_path("src/main.rs"));
    }

    #[test]
    fn test_default_gates_for_level() {
        let autonomous_gates = default_gates_for_level(SupervisionLevel::Autonomous);
        assert_eq!(autonomous_gates.get("file_create"), Some(&GateAction::Allow));
        assert_eq!(autonomous_gates.get("git_force_push"), Some(&GateAction::Approve));

        let supervised_gates = default_gates_for_level(SupervisionLevel::Supervised);
        assert_eq!(supervised_gates.get("file_delete"), Some(&GateAction::Approve));
        assert_eq!(supervised_gates.get("git_force_push"), Some(&GateAction::Deny));
    }
}
