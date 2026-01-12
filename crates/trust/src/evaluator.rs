//! Trust evaluation engine
//!
//! The core logic that decides whether an operation should be allowed,
//! denied, or should prompt the user.

use crate::config::{DomainStatus, TrustConfig};
use crate::level::{ToolCategory, TrustLevel};
use crate::pattern::{normalize_pattern, PatternDecision, PatternStore};
use crate::session::Session;
use serde::{Deserialize, Serialize};
use std::path::Path;

/// Result of a trust evaluation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Decision {
    /// The final decision
    pub action: Action,

    /// Reason for the decision
    pub reason: Reason,

    /// Details about the evaluation
    pub details: String,

    /// Pattern that matched (if any)
    pub matched_pattern: Option<String>,
}

/// Actions the trust system can take
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Action {
    /// Allow the operation without prompting
    Allow,
    /// Deny the operation without prompting
    Deny,
    /// Ask the user for permission
    Ask,
}

/// Reason for the decision
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Reason {
    /// Allowed by trust level
    TrustLevel,
    /// Allowed by learned pattern
    PatternMatch,
    /// Allowed by session escalation
    SessionEscalation,
    /// Denied by trust level
    InsufficientTrust,
    /// Denied by learned pattern
    PatternDeny,
    /// Denied by domain block
    DomainBlocked,
    /// Must ask - first occurrence
    FirstOccurrence,
    /// Must ask - destructive operation
    Destructive,
    /// Must ask - network to unknown domain
    UnknownDomain,
    /// Must ask - protected branch
    ProtectedBranch,
    /// Tool category requires asking
    ToolCategoryRequiresAsk,
}

/// Request to evaluate
#[derive(Debug, Clone)]
pub struct EvalRequest<'a> {
    /// The tool being invoked
    pub tool: &'a str,
    /// Arguments to the tool
    pub args: &'a [&'a str],
    /// Working directory
    pub working_dir: &'a Path,
    /// Domain (for network operations)
    pub domain: Option<&'a str>,
    /// Git branch (for git operations)
    pub branch: Option<&'a str>,
}

/// Trust evaluator - the main evaluation engine
pub struct TrustEvaluator {
    config: TrustConfig,
    patterns: PatternStore,
}

impl TrustEvaluator {
    /// Create a new evaluator with loaded config and patterns
    pub fn new() -> anyhow::Result<Self> {
        let config = TrustConfig::load()?;
        let patterns = PatternStore::load()?;
        Ok(Self { config, patterns })
    }

    /// Create an evaluator with specific config and patterns
    pub fn with_config(config: TrustConfig, patterns: PatternStore) -> Self {
        Self { config, patterns }
    }

    /// Evaluate a request
    pub fn evaluate(&self, request: &EvalRequest, session: Option<&Session>) -> Decision {
        // Get trust level for working directory
        let trust_level = session
            .and_then(|s| s.escalated_level)
            .unwrap_or_else(|| self.config.trust_level_for_path(request.working_dir));

        // Categorize the tool
        let category = ToolCategory::categorize(request.tool, request.args);

        // Handle by category
        match category {
            ToolCategory::ReadOnly => self.allow(Reason::TrustLevel, "Read operations always allowed"),

            ToolCategory::SafeMutation => self.evaluate_safe_mutation(request, trust_level, session),

            ToolCategory::PatternDestructive => {
                self.evaluate_pattern_destructive(request, trust_level, session)
            }

            ToolCategory::AlwaysDestructive => self.evaluate_always_destructive(request, trust_level),

            ToolCategory::Network => self.evaluate_network(request, trust_level, session),

            ToolCategory::System => self.ask(Reason::Destructive, "System operations require confirmation"),
        }
    }

    /// Evaluate safe mutation operations
    fn evaluate_safe_mutation(
        &self,
        _request: &EvalRequest,
        trust_level: TrustLevel,
        session: Option<&Session>,
    ) -> Decision {
        // Owner and sandbox always allow safe mutations
        if trust_level >= TrustLevel::Developer {
            return self.allow(Reason::TrustLevel, format!("Trust level {} allows mutations", trust_level));
        }

        // Check for session-level approval
        if let Some(s) = session {
            if s.has_approved_category("safe_mutation") {
                return self.allow(Reason::SessionEscalation, "Approved for session");
            }
        }

        // Contractor and below need to ask first time
        self.ask(Reason::FirstOccurrence, "First mutation in session")
    }

    /// Evaluate pattern-dependent destructive operations
    fn evaluate_pattern_destructive(
        &self,
        request: &EvalRequest,
        trust_level: TrustLevel,
        session: Option<&Session>,
    ) -> Decision {
        // Generate pattern for this operation
        let pattern = normalize_pattern(request.tool, request.args);
        let scope = request.working_dir.to_string_lossy();

        // Check learned patterns first
        if let Some(learned) = self.patterns.get(&pattern, &scope) {
            match learned.decision {
                PatternDecision::Allow => {
                    return Decision {
                        action: Action::Allow,
                        reason: Reason::PatternMatch,
                        details: format!("Matches learned pattern: {}", pattern),
                        matched_pattern: Some(learned.pattern.clone()),
                    };
                }
                PatternDecision::Deny => {
                    return Decision {
                        action: Action::Deny,
                        reason: Reason::PatternDeny,
                        details: format!("Denied by learned pattern: {}", pattern),
                        matched_pattern: Some(learned.pattern.clone()),
                    };
                }
                PatternDecision::Ask => {
                    return Decision {
                        action: Action::Ask,
                        reason: Reason::ToolCategoryRequiresAsk,
                        details: format!("Pattern configured to always ask: {}", pattern),
                        matched_pattern: Some(learned.pattern.clone()),
                    };
                }
            }
        }

        // Owner level allows destructive operations
        if trust_level == TrustLevel::Owner {
            return self.allow(Reason::TrustLevel, "Owner trust allows destructive operations");
        }

        // Sandbox allows everything
        if trust_level == TrustLevel::Sandbox {
            return self.allow(Reason::TrustLevel, "Sandbox allows all operations");
        }

        // Check session approvals
        if let Some(s) = session {
            if s.has_approved_pattern(&pattern) {
                return self.allow(Reason::SessionEscalation, "Pattern approved for session");
            }
        }

        // Need to ask
        Decision {
            action: Action::Ask,
            reason: Reason::Destructive,
            details: format!("Destructive operation: {}", pattern),
            matched_pattern: None,
        }
    }

    /// Evaluate always-destructive operations
    fn evaluate_always_destructive(&self, request: &EvalRequest, trust_level: TrustLevel) -> Decision {
        // Check for force push to protected branch
        if request.tool == "git" {
            if let Some(branch) = request.branch {
                if self.config.tools.git_push.protected_branches.contains(&branch.to_string()) {
                    return self.ask(
                        Reason::ProtectedBranch,
                        format!("Force push to protected branch: {}", branch),
                    );
                }
            }
        }

        // Sandbox is the only level that auto-allows destructive
        if trust_level == TrustLevel::Sandbox {
            return self.allow(Reason::TrustLevel, "Sandbox allows destructive operations");
        }

        // Always ask for truly destructive operations
        let pattern = normalize_pattern(request.tool, request.args);
        self.ask(Reason::Destructive, format!("Destructive operation: {}", pattern))
    }

    /// Evaluate network operations
    fn evaluate_network(
        &self,
        request: &EvalRequest,
        trust_level: TrustLevel,
        session: Option<&Session>,
    ) -> Decision {
        // Check domain if provided
        if let Some(domain) = request.domain {
            match self.config.domain_status(domain) {
                DomainStatus::Allowed => {
                    return self.allow(Reason::TrustLevel, format!("Domain {} is allowed", domain));
                }
                DomainStatus::Blocked => {
                    return Decision {
                        action: Action::Deny,
                        reason: Reason::DomainBlocked,
                        details: format!("Domain {} is blocked", domain),
                        matched_pattern: None,
                    };
                }
                DomainStatus::Ask | DomainStatus::Unknown => {
                    // Check session
                    if let Some(s) = session {
                        if s.has_approved_domain(domain) {
                            return self.allow(
                                Reason::SessionEscalation,
                                format!("Domain {} approved for session", domain),
                            );
                        }
                    }
                    return self.ask(Reason::UnknownDomain, format!("Network access to {}", domain));
                }
            }
        }

        // No domain specified - check trust level
        if trust_level >= TrustLevel::Developer {
            return self.allow(Reason::TrustLevel, "Trust level allows network access");
        }

        // Check session
        if let Some(s) = session {
            if s.has_approved_category("network") {
                return self.allow(Reason::SessionEscalation, "Network approved for session");
            }
        }

        self.ask(Reason::FirstOccurrence, "Network access requires approval")
    }

    /// Helper to create an allow decision
    fn allow(&self, reason: Reason, details: impl Into<String>) -> Decision {
        Decision {
            action: Action::Allow,
            reason,
            details: details.into(),
            matched_pattern: None,
        }
    }

    /// Helper to create an ask decision
    fn ask(&self, reason: Reason, details: impl Into<String>) -> Decision {
        Decision {
            action: Action::Ask,
            reason,
            details: details.into(),
            matched_pattern: None,
        }
    }

    /// Get mutable access to pattern store
    pub fn patterns_mut(&mut self) -> &mut PatternStore {
        &mut self.patterns
    }

    /// Get reference to pattern store
    pub fn patterns(&self) -> &PatternStore {
        &self.patterns
    }

    /// Get reference to config
    pub fn config(&self) -> &TrustConfig {
        &self.config
    }

    /// Save patterns (after learning)
    pub fn save_patterns(&self) -> anyhow::Result<()> {
        self.patterns.save()
    }
}

impl Default for TrustEvaluator {
    fn default() -> Self {
        Self {
            config: TrustConfig::default(),
            patterns: PatternStore::default(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_read_always_allowed() {
        let evaluator = TrustEvaluator::default();
        let request = EvalRequest {
            tool: "cat",
            args: &["file.txt"],
            working_dir: Path::new("/tmp"),
            domain: None,
            branch: None,
        };

        let decision = evaluator.evaluate(&request, None);
        assert_eq!(decision.action, Action::Allow);
    }

    #[test]
    fn test_rm_rf_always_asks() {
        let evaluator = TrustEvaluator::default();
        let request = EvalRequest {
            tool: "rm",
            args: &["-rf", "dir"],
            working_dir: Path::new("/home/user/projects/foo"),
            domain: None,
            branch: None,
        };

        let decision = evaluator.evaluate(&request, None);
        assert_eq!(decision.action, Action::Ask);
        assert_eq!(decision.reason, Reason::Destructive);
    }

    #[test]
    fn test_allowed_domain() {
        let evaluator = TrustEvaluator::default();
        let request = EvalRequest {
            tool: "curl",
            args: &["https://api.github.com/users"],
            working_dir: Path::new("/tmp"),
            domain: Some("api.github.com"),
            branch: None,
        };

        let decision = evaluator.evaluate(&request, None);
        assert_eq!(decision.action, Action::Allow);
    }

    #[test]
    fn test_unknown_domain() {
        let evaluator = TrustEvaluator::default();
        let request = EvalRequest {
            tool: "curl",
            args: &["https://random.xyz"],
            working_dir: Path::new("/tmp"),
            domain: Some("random.xyz"),
            branch: None,
        };

        let decision = evaluator.evaluate(&request, None);
        assert_eq!(decision.action, Action::Ask);
    }
}
