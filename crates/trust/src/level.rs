//! Trust levels define the baseline permissions for a context
//!
//! Levels form a hierarchy from most to least permissive:
//! owner > developer > contractor > sandbox > guest

use serde::{Deserialize, Serialize};
use std::fmt;

/// Trust levels for projects and sessions
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TrustLevel {
    /// Unknown territory. Minimal trust.
    /// Allows: read files
    /// Asks: all writes, deletions, network, git
    Guest,

    /// Ephemeral environment. Full trust because it's disposable.
    /// Allows: everything (within sandbox boundaries)
    Sandbox,

    /// Restricted write. Trust but verify.
    /// Allows: read all, write source, git add/commit
    /// Asks: deletions, git push, network, config writes
    Contractor,

    /// Working code. High trust with guardrails.
    /// Allows: read/write all, git (except force), network to known hosts
    /// Asks: first destructive op, rm source files (learns pattern)
    Developer,

    /// Your code. Full trust.
    /// Allows: everything
    /// Asks: force push to protected, rm -rf dirs, network to unknown
    Owner,
}

impl Default for TrustLevel {
    fn default() -> Self {
        TrustLevel::Guest
    }
}

impl TrustLevel {
    /// Parse from string
    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "owner" => Some(TrustLevel::Owner),
            "developer" | "dev" => Some(TrustLevel::Developer),
            "contractor" => Some(TrustLevel::Contractor),
            "sandbox" => Some(TrustLevel::Sandbox),
            "guest" => Some(TrustLevel::Guest),
            _ => None,
        }
    }

    /// Get the string name
    pub fn as_str(&self) -> &'static str {
        match self {
            TrustLevel::Owner => "owner",
            TrustLevel::Developer => "developer",
            TrustLevel::Contractor => "contractor",
            TrustLevel::Sandbox => "sandbox",
            TrustLevel::Guest => "guest",
        }
    }

    /// Check if this level can perform an operation that requires a minimum level
    pub fn can_do(&self, required: TrustLevel) -> bool {
        *self >= required
    }

    /// Check if level allows read operations by default
    pub fn allows_read(&self) -> bool {
        true // All levels allow read
    }

    /// Check if level allows write operations by default
    pub fn allows_write(&self) -> bool {
        match self {
            TrustLevel::Owner | TrustLevel::Developer | TrustLevel::Contractor | TrustLevel::Sandbox => true,
            TrustLevel::Guest => false,
        }
    }

    /// Check if level allows destructive operations by default
    pub fn allows_destructive(&self) -> bool {
        match self {
            TrustLevel::Owner | TrustLevel::Sandbox => true,
            TrustLevel::Developer | TrustLevel::Contractor | TrustLevel::Guest => false,
        }
    }

    /// Check if level allows network operations by default
    pub fn allows_network(&self) -> bool {
        match self {
            TrustLevel::Owner | TrustLevel::Developer | TrustLevel::Sandbox => true,
            TrustLevel::Contractor | TrustLevel::Guest => false,
        }
    }

    /// Get the maximum level an agent can escalate to from this level
    pub fn max_escalation(&self) -> TrustLevel {
        match self {
            TrustLevel::Owner => TrustLevel::Owner,
            TrustLevel::Developer => TrustLevel::Developer,
            TrustLevel::Contractor => TrustLevel::Developer, // Can escalate one level
            TrustLevel::Sandbox => TrustLevel::Sandbox, // No escalation from sandbox
            TrustLevel::Guest => TrustLevel::Contractor, // Can escalate one level
        }
    }
}

impl fmt::Display for TrustLevel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// Tool risk categories
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolCategory {
    /// Read operations - never need permission
    ReadOnly,
    /// Reversible mutations - ask once per session
    SafeMutation,
    /// Pattern-dependent destruction - learn from patterns
    PatternDestructive,
    /// Always destructive - always ask
    AlwaysDestructive,
    /// System operations - context dependent
    System,
    /// Network operations - domain and session aware
    Network,
}

impl ToolCategory {
    /// Categorize a tool/command
    pub fn categorize(tool: &str, args: &[&str]) -> Self {
        let tool_lower = tool.to_lowercase();

        // Read-only tools
        if matches!(
            tool_lower.as_str(),
            "cat" | "head" | "tail" | "less" | "more"
                | "ls" | "tree" | "find"
                | "grep" | "rg" | "ag" | "ack"
                | "file" | "stat" | "which" | "type"
                | "pwd" | "echo" | "printf"
        ) {
            return ToolCategory::ReadOnly;
        }

        // Git commands need special handling
        if tool_lower == "git" {
            if let Some(subcmd) = args.first() {
                match *subcmd {
                    "status" | "log" | "diff" | "show" | "branch" | "remote" | "tag" => {
                        return ToolCategory::ReadOnly;
                    }
                    "add" | "commit" | "stash" => {
                        return ToolCategory::SafeMutation;
                    }
                    "push" => {
                        // Check for --force
                        if args.iter().any(|a| a.contains("force") || *a == "-f") {
                            return ToolCategory::AlwaysDestructive;
                        }
                        return ToolCategory::Network;
                    }
                    "fetch" | "pull" | "clone" => {
                        return ToolCategory::Network;
                    }
                    "reset" | "checkout" => {
                        // Check for --hard or file arguments
                        if args.iter().any(|a| *a == "--hard") {
                            return ToolCategory::AlwaysDestructive;
                        }
                        return ToolCategory::PatternDestructive;
                    }
                    _ => {}
                }
            }
            return ToolCategory::SafeMutation;
        }

        // rm needs pattern analysis
        if tool_lower == "rm" {
            // rm -rf is always destructive
            if args.iter().any(|a| a.contains('r') && a.starts_with('-')) {
                return ToolCategory::AlwaysDestructive;
            }
            return ToolCategory::PatternDestructive;
        }

        // Safe mutations
        if matches!(
            tool_lower.as_str(),
            "touch" | "mkdir" | "cp" | "ln"
        ) {
            return ToolCategory::SafeMutation;
        }

        // Pattern destructive
        if matches!(tool_lower.as_str(), "mv" | "chmod" | "chown") {
            return ToolCategory::PatternDestructive;
        }

        // Always destructive
        if matches!(tool_lower.as_str(), "sudo" | "doas") {
            return ToolCategory::System;
        }

        // Network tools
        if matches!(
            tool_lower.as_str(),
            "curl" | "wget" | "fetch"
                | "npm" | "yarn" | "pnpm"
                | "pip" | "pip3" | "pipx"
                | "cargo" | "rustup"
                | "brew"
        ) {
            return ToolCategory::Network;
        }

        // Default to safe mutation for unknown tools in trusted contexts
        ToolCategory::SafeMutation
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_trust_level_ordering() {
        assert!(TrustLevel::Owner > TrustLevel::Developer);
        assert!(TrustLevel::Developer > TrustLevel::Contractor);
        assert!(TrustLevel::Contractor > TrustLevel::Sandbox);
        assert!(TrustLevel::Sandbox > TrustLevel::Guest);
    }

    #[test]
    fn test_can_do() {
        assert!(TrustLevel::Owner.can_do(TrustLevel::Developer));
        assert!(TrustLevel::Developer.can_do(TrustLevel::Developer));
        assert!(!TrustLevel::Guest.can_do(TrustLevel::Developer));
    }

    #[test]
    fn test_tool_categorization() {
        assert_eq!(
            ToolCategory::categorize("cat", &[]),
            ToolCategory::ReadOnly
        );
        assert_eq!(
            ToolCategory::categorize("git", &["status"]),
            ToolCategory::ReadOnly
        );
        assert_eq!(
            ToolCategory::categorize("git", &["push"]),
            ToolCategory::Network
        );
        assert_eq!(
            ToolCategory::categorize("git", &["push", "--force"]),
            ToolCategory::AlwaysDestructive
        );
        assert_eq!(
            ToolCategory::categorize("rm", &["-rf", "dir"]),
            ToolCategory::AlwaysDestructive
        );
        assert_eq!(
            ToolCategory::categorize("rm", &["file.txt"]),
            ToolCategory::PatternDestructive
        );
    }
}
