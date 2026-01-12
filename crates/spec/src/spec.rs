//! Spec data structures and loading

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::Path;

/// A spec.yaml file structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Spec {
    /// Component name
    pub name: String,

    /// Version string
    #[serde(default)]
    pub version: Option<String>,

    /// Creation date
    #[serde(default)]
    pub created: Option<String>,

    /// WHY this component exists
    #[serde(default)]
    pub intent: Option<String>,

    /// Hard requirements - can be strings or structured constraints
    #[serde(default)]
    pub constraints: Option<serde_yaml::Value>,

    /// The component's interface
    #[serde(default)]
    pub interface: Option<serde_yaml::Value>,

    /// Usage examples
    #[serde(default)]
    pub examples: Option<Vec<Example>>,

    /// Design decisions
    #[serde(default)]
    pub decisions: Option<Vec<Decision>>,

    /// Anti-patterns to avoid
    #[serde(default)]
    pub anti_patterns: Option<Vec<AntiPattern>>,

    /// Connections to other components
    #[serde(default)]
    pub connects_to: Option<Vec<Connection>>,

    /// Success/failure metrics
    #[serde(default)]
    pub metrics: Option<Metrics>,

    /// Allow extra fields without failing
    #[serde(flatten)]
    pub extra: std::collections::HashMap<String, serde_yaml::Value>,
}

/// Usage example
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Example {
    /// Scenario name
    #[serde(default)]
    pub scenario: Option<String>,

    /// Context description
    #[serde(default)]
    pub context: Option<String>,

    /// Action taken
    #[serde(default)]
    pub action: Option<String>,

    /// Result of action
    #[serde(default)]
    pub result: Option<String>,

    /// Why it matters
    #[serde(default)]
    pub why_it_matters: Option<String>,
}

/// Design decision
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Decision {
    /// The choice made
    pub choice: String,

    /// Why this choice was made
    #[serde(default)]
    pub why: String,

    /// Alternatives considered
    #[serde(default)]
    pub alternatives: Option<Vec<Alternative>>,
}

/// Alternative considered
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Alternative {
    /// Option name
    #[serde(default)]
    pub option: Option<String>,

    /// Why rejected
    #[serde(default)]
    pub rejected_because: Option<String>,
}

/// Anti-pattern to avoid
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AntiPattern {
    /// The pattern to avoid
    pub pattern: String,

    /// Why it's bad
    #[serde(default)]
    pub why_bad: String,
}

/// Connection to another component
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Connection {
    /// Component name
    pub component: String,

    /// How they relate
    #[serde(default)]
    pub relationship: Option<String>,
}

/// Metrics for success
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Metrics {
    /// Success criteria
    #[serde(default)]
    pub success_criteria: Option<Vec<String>>,

    /// Failure indicators
    #[serde(default)]
    pub failure_indicators: Option<Vec<String>>,
}

impl Spec {
    /// Load a spec from a YAML file
    pub fn load(path: &Path) -> Result<Self> {
        let content = std::fs::read_to_string(path)
            .with_context(|| format!("Failed to read spec file: {}", path.display()))?;

        let spec: Spec = serde_yaml::from_str(&content)
            .with_context(|| format!("Failed to parse spec file: {}", path.display()))?;

        Ok(spec)
    }

    /// Get a section by name as YAML value
    pub fn get_section(&self, section: &str) -> Option<serde_yaml::Value> {
        match section {
            "name" => Some(serde_yaml::Value::String(self.name.clone())),
            "version" => self.version.clone().map(serde_yaml::Value::String),
            "created" => self.created.clone().map(serde_yaml::Value::String),
            "intent" => self.intent.clone().map(serde_yaml::Value::String),
            "constraints" => self.constraints.clone(),
            "interface" => self.interface.clone(),
            "examples" => self.examples.as_ref().and_then(|e| {
                serde_yaml::to_value(e).ok()
            }),
            "decisions" => self.decisions.as_ref().and_then(|d| {
                serde_yaml::to_value(d).ok()
            }),
            "anti_patterns" => self.anti_patterns.as_ref().and_then(|a| {
                serde_yaml::to_value(a).ok()
            }),
            "connects_to" => self.connects_to.as_ref().and_then(|c| {
                serde_yaml::to_value(c).ok()
            }),
            "metrics" => self.metrics.as_ref().and_then(|m| {
                serde_yaml::to_value(m).ok()
            }),
            _ => self.extra.get(section).cloned(),
        }
    }

    /// Check if a required section exists and is non-empty
    pub fn has_section(&self, section: &str) -> bool {
        match section {
            "name" => !self.name.is_empty(),
            "intent" => self.intent.as_ref().map(|s| !s.is_empty()).unwrap_or(false),
            "constraints" => self.constraints.as_ref().map(|c| !is_yaml_empty(c)).unwrap_or(false),
            "interface" => self.interface.as_ref().map(|i| !is_yaml_empty(i)).unwrap_or(false),
            "examples" => self.examples.as_ref().map(|e| !e.is_empty()).unwrap_or(false),
            "decisions" => self.decisions.as_ref().map(|d| !d.is_empty()).unwrap_or(false),
            "anti_patterns" => self.anti_patterns.as_ref().map(|a| !a.is_empty()).unwrap_or(false),
            _ => self.extra.contains_key(section),
        }
    }
}

/// Check if a YAML value is empty or null
fn is_yaml_empty(value: &serde_yaml::Value) -> bool {
    match value {
        serde_yaml::Value::Null => true,
        serde_yaml::Value::Sequence(seq) => seq.is_empty(),
        serde_yaml::Value::Mapping(map) => map.is_empty(),
        serde_yaml::Value::String(s) => s.is_empty(),
        _ => false,
    }
}
