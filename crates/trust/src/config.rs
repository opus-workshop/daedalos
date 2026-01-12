//! Trust configuration management
//!
//! Configuration files:
//! - ~/.config/daedalos/trust.yaml - Global trust settings
//! - .daedalos/trust.yaml - Project-specific overrides

use crate::level::TrustLevel;
use anyhow::{Context, Result};
use daedalos_core::Paths;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;

/// Global trust configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrustConfig {
    /// Default settings
    #[serde(default)]
    pub defaults: Defaults,

    /// Path-based trust assignments
    #[serde(default)]
    pub paths: HashMap<String, TrustLevel>,

    /// Domain allowlist for network operations
    #[serde(default)]
    pub domains: DomainConfig,

    /// Tool-specific overrides
    #[serde(default)]
    pub tools: ToolOverrides,

    /// Audit settings
    #[serde(default)]
    pub audit: AuditConfig,
}

impl Default for TrustConfig {
    fn default() -> Self {
        let mut paths = HashMap::new();
        // Sensible defaults
        paths.insert("~/projects/*".to_string(), TrustLevel::Developer);
        paths.insert("~/work/*".to_string(), TrustLevel::Contractor);
        paths.insert("/tmp/scratch-*".to_string(), TrustLevel::Sandbox);
        paths.insert("~/.config".to_string(), TrustLevel::Guest);

        Self {
            defaults: Defaults::default(),
            paths,
            domains: DomainConfig::default(),
            tools: ToolOverrides::default(),
            audit: AuditConfig::default(),
        }
    }
}

impl TrustConfig {
    /// Load configuration from default location
    pub fn load() -> Result<Self> {
        let paths = Paths::new();
        let config_path = paths.config.join("trust.yaml");
        Self::load_from(&config_path)
    }

    /// Load configuration from a specific path
    pub fn load_from(path: &Path) -> Result<Self> {
        if path.exists() {
            let content = std::fs::read_to_string(path)
                .with_context(|| format!("Failed to read trust config from {:?}", path))?;
            let config: Self = serde_yaml::from_str(&content)
                .with_context(|| format!("Failed to parse trust config from {:?}", path))?;
            Ok(config)
        } else {
            Ok(Self::default())
        }
    }

    /// Save configuration to default location
    pub fn save(&self) -> Result<()> {
        let paths = Paths::new();
        let config_path = paths.config.join("trust.yaml");
        self.save_to(&config_path)
    }

    /// Save configuration to a specific path
    pub fn save_to(&self, path: &Path) -> Result<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let content = serde_yaml::to_string(self)?;
        std::fs::write(path, content)?;
        Ok(())
    }

    /// Get trust level for a path
    pub fn trust_level_for_path(&self, path: &Path) -> TrustLevel {
        let path_str = path.to_string_lossy();

        // First check for project-level config
        if let Some(project_level) = self.load_project_trust(path) {
            return project_level;
        }

        // Then check configured paths (most specific first)
        let mut best_match: Option<(usize, TrustLevel)> = None;

        for (pattern, level) in &self.paths {
            let expanded = expand_path(pattern);
            if matches_path_pattern(&path_str, &expanded) {
                let specificity = pattern.len();
                if best_match.map_or(true, |(s, _)| specificity > s) {
                    best_match = Some((specificity, *level));
                }
            }
        }

        best_match.map_or(self.defaults.unknown_project, |(_, level)| level)
    }

    /// Check if a domain is allowed
    pub fn domain_status(&self, domain: &str) -> DomainStatus {
        let domain_lower = domain.to_lowercase();

        // Check blocked first
        for blocked in &self.domains.blocked {
            if matches_domain_pattern(&domain_lower, blocked) {
                return DomainStatus::Blocked;
            }
        }

        // Check allowed
        for allowed in &self.domains.allowed {
            if matches_domain_pattern(&domain_lower, allowed) {
                return DomainStatus::Allowed;
            }
        }

        // Check ask
        for ask in &self.domains.ask {
            if matches_domain_pattern(&domain_lower, ask) {
                return DomainStatus::Ask;
            }
        }

        DomainStatus::Unknown
    }

    /// Load project-specific trust config
    fn load_project_trust(&self, path: &Path) -> Option<TrustLevel> {
        // Walk up to find .daedalos/trust.yaml
        let mut current = path.to_path_buf();
        while current.parent().is_some() {
            let project_config = current.join(".daedalos/trust.yaml");
            if project_config.exists() {
                if let Ok(content) = std::fs::read_to_string(&project_config) {
                    if let Ok(config) = serde_yaml::from_str::<ProjectTrustConfig>(&content) {
                        return Some(config.trust_level);
                    }
                }
            }
            current = current.parent()?.to_path_buf();
        }
        None
    }
}

/// Default settings
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Defaults {
    /// Trust level for unknown projects
    #[serde(default = "default_unknown_project")]
    pub unknown_project: TrustLevel,

    /// Number of approvals before suggesting auto-allow
    #[serde(default = "default_suggestion_threshold")]
    pub suggestion_threshold: u32,

    /// Days to retain audit logs
    #[serde(default = "default_audit_retention_days")]
    pub audit_retention_days: u32,
}

fn default_unknown_project() -> TrustLevel {
    TrustLevel::Guest
}

fn default_suggestion_threshold() -> u32 {
    3
}

fn default_audit_retention_days() -> u32 {
    30
}

impl Default for Defaults {
    fn default() -> Self {
        Self {
            unknown_project: default_unknown_project(),
            suggestion_threshold: default_suggestion_threshold(),
            audit_retention_days: default_audit_retention_days(),
        }
    }
}

/// Domain configuration
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DomainConfig {
    /// Always allowed domains
    #[serde(default = "default_allowed_domains")]
    pub allowed: Vec<String>,

    /// Domains that require confirmation
    #[serde(default)]
    pub ask: Vec<String>,

    /// Blocked domains
    #[serde(default)]
    pub blocked: Vec<String>,
}

fn default_allowed_domains() -> Vec<String> {
    vec![
        "api.github.com".to_string(),
        "github.com".to_string(),
        "pypi.org".to_string(),
        "registry.npmjs.org".to_string(),
        "crates.io".to_string(),
        "rubygems.org".to_string(),
        "pkg.go.dev".to_string(),
    ]
}

/// Tool-specific overrides
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ToolOverrides {
    /// Git push settings
    #[serde(default)]
    pub git_push: GitPushConfig,
}

/// Git push configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitPushConfig {
    /// Protected branches that always require confirmation
    #[serde(default = "default_protected_branches")]
    pub protected_branches: Vec<String>,

    /// Action for protected branches
    #[serde(default = "default_protected_action")]
    pub action: String,
}

fn default_protected_branches() -> Vec<String> {
    vec!["main".to_string(), "master".to_string(), "production".to_string()]
}

fn default_protected_action() -> String {
    "ask".to_string()
}

impl Default for GitPushConfig {
    fn default() -> Self {
        Self {
            protected_branches: default_protected_branches(),
            action: default_protected_action(),
        }
    }
}

/// Audit configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditConfig {
    /// Enable audit logging
    #[serde(default = "default_audit_enabled")]
    pub enabled: bool,

    /// Retention in days
    #[serde(default = "default_audit_retention_days")]
    pub retention_days: u32,
}

fn default_audit_enabled() -> bool {
    true
}

impl Default for AuditConfig {
    fn default() -> Self {
        Self {
            enabled: default_audit_enabled(),
            retention_days: default_audit_retention_days(),
        }
    }
}

/// Project-specific trust configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectTrustConfig {
    /// Trust level for this project
    pub trust_level: TrustLevel,

    /// Protected paths within the project
    #[serde(default)]
    pub protected_paths: Vec<String>,

    /// Additional allowed domains
    #[serde(default)]
    pub domains: ProjectDomainConfig,
}

/// Project domain config
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ProjectDomainConfig {
    /// Additional allowed domains
    #[serde(default)]
    pub allowed: Vec<String>,
}

/// Domain status result
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DomainStatus {
    Allowed,
    Ask,
    Blocked,
    Unknown,
}

/// Expand ~ and environment variables in path
fn expand_path(path: &str) -> String {
    shellexpand::tilde(path).to_string()
}

/// Check if a path matches a pattern (supports globs)
fn matches_path_pattern(path: &str, pattern: &str) -> bool {
    // Simple glob matching
    if pattern.ends_with("/*") {
        let prefix = &pattern[..pattern.len() - 2];
        path.starts_with(prefix)
    } else if pattern.ends_with("*") {
        let prefix = &pattern[..pattern.len() - 1];
        path.starts_with(prefix)
    } else {
        path == pattern || path.starts_with(&format!("{}/", pattern))
    }
}

/// Check if a domain matches a pattern
fn matches_domain_pattern(domain: &str, pattern: &str) -> bool {
    if pattern.starts_with("*.") {
        // Wildcard subdomain
        let suffix = &pattern[1..]; // ".example.com"
        domain.ends_with(suffix) || domain == &pattern[2..]
    } else {
        domain == pattern
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_path_matching() {
        assert!(matches_path_pattern(
            "/home/user/projects/foo",
            "/home/user/projects/*"
        ));
        assert!(matches_path_pattern(
            "/home/user/projects/foo/bar",
            "/home/user/projects/*"
        ));
        assert!(!matches_path_pattern(
            "/home/user/work/foo",
            "/home/user/projects/*"
        ));
    }

    #[test]
    fn test_domain_matching() {
        assert!(matches_domain_pattern("api.github.com", "api.github.com"));
        assert!(matches_domain_pattern("api.github.com", "*.github.com"));
        assert!(matches_domain_pattern("github.com", "*.github.com"));
        assert!(!matches_domain_pattern("github.org", "*.github.com"));
    }

    #[test]
    fn test_default_domains() {
        let config = TrustConfig::default();
        assert_eq!(config.domain_status("api.github.com"), DomainStatus::Allowed);
        assert_eq!(config.domain_status("pypi.org"), DomainStatus::Allowed);
        assert_eq!(config.domain_status("random.xyz"), DomainStatus::Unknown);
    }
}
