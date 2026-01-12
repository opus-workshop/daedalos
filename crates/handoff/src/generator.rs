//! Handoff generation - aggregates context from git, journal, and environment

use anyhow::Result;
use chrono::{Local, Utc};
use std::env;
use std::process::Command;

use crate::storage::Handoff;

/// Git repository information
#[derive(Debug, Clone, Default)]
pub struct GitInfo {
    /// Current branch name
    pub branch: Option<String>,
    /// Number of changed files (staged + unstaged)
    pub changed_files: usize,
    /// Recent commits (within time window)
    pub recent_commits: Vec<String>,
    /// Commits ahead of remote
    pub ahead: usize,
    /// Commits behind remote
    pub behind: usize,
}

/// Environment information
#[derive(Debug, Clone)]
pub struct EnvInfo {
    /// Current working directory
    pub directory: String,
    /// Project name (from env or directory name)
    pub project_name: String,
    /// Project type if known
    pub project_type: Option<String>,
    /// Active loop status if any
    pub active_loop: Option<String>,
    /// Number of active agents
    pub active_agents: usize,
}

/// Generates handoff summaries by aggregating context
pub struct HandoffGenerator {
    /// Current working directory
    cwd: String,
}

impl HandoffGenerator {
    pub fn new() -> Self {
        let cwd = env::current_dir()
            .map(|p| p.display().to_string())
            .unwrap_or_else(|_| ".".to_string());

        Self { cwd }
    }

    /// Generate a full handoff document
    pub fn generate(
        &self,
        name: Option<&str>,
        to: Option<&str>,
        hours: u64,
    ) -> Result<Handoff> {
        let now = Local::now();
        let name = name
            .map(|s| s.to_string())
            .unwrap_or_else(|| format!("handoff-{}", now.format("%Y%m%d-%H%M")));

        let from = env::var("USER").unwrap_or_else(|_| "unknown".to_string());
        let git_info = self.get_git_info(hours);
        let env_info = self.get_env_info();

        let content = self.build_document(&name, &from, to, hours, &git_info, &env_info);

        Ok(Handoff {
            name,
            created: now.with_timezone(&Utc),
            from,
            to: to.map(|s| s.to_string()),
            hours,
            content,
        })
    }

    /// Generate a quick status summary (no file creation)
    pub fn quick_status(&self, hours: u64) -> Result<String> {
        let git_info = self.get_git_info(hours);
        let env_info = self.get_env_info();

        let mut output = String::new();
        output.push_str("Quick Status\n");
        output.push_str("\n");

        // Location
        output.push_str(&format!("Location: {}\n", env_info.directory));
        output.push_str("\n");

        // Git status
        if git_info.branch.is_some() {
            output.push_str("Git:\n");
            if let Some(ref branch) = git_info.branch {
                output.push_str(&format!("  Branch: {}\n", branch));
            }
            output.push_str(&format!("  Changed: {} files\n", git_info.changed_files));
            if let Some(commit) = git_info.recent_commits.first() {
                output.push_str(&format!("  Last commit: {}\n", commit));
            }
            output.push_str("\n");
        }

        // Active processes
        if env_info.active_loop.is_some() {
            output.push_str(&format!("Loop: Active\n"));
        }
        if env_info.active_agents > 0 {
            output.push_str(&format!("Agents: {} active\n", env_info.active_agents));
        }

        Ok(output)
    }

    /// Get git repository information
    fn get_git_info(&self, hours: u64) -> GitInfo {
        let mut info = GitInfo::default();

        // Check if in git repo
        if !self.is_git_repo() {
            return info;
        }

        // Get current branch
        info.branch = self.run_git(&["branch", "--show-current"]);

        // Get changed files count
        if let Some(output) = self.run_git(&["status", "--porcelain"]) {
            info.changed_files = output.lines().count();
        }

        // Get recent commits
        let since = format!("{} hours ago", hours);
        if let Some(output) = self.run_git(&["log", "--oneline", "--since", &since]) {
            info.recent_commits = output.lines().take(10).map(|s| s.to_string()).collect();
        }

        // Get ahead/behind counts
        if let Some(output) = self.run_git(&["rev-list", "--count", "@{u}..HEAD"]) {
            info.ahead = output.trim().parse().unwrap_or(0);
        }
        if let Some(output) = self.run_git(&["rev-list", "--count", "HEAD..@{u}"]) {
            info.behind = output.trim().parse().unwrap_or(0);
        }

        info
    }

    /// Get environment information
    fn get_env_info(&self) -> EnvInfo {
        let directory = self.cwd.clone();

        let project_name = env::var("DAEDALOS_PROJECT_NAME")
            .or_else(|_| {
                std::path::Path::new(&self.cwd)
                    .file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .ok_or(())
            })
            .unwrap_or_else(|_| "unknown".to_string());

        let project_type = env::var("DAEDALOS_PROJECT_TYPE").ok();

        // Check for active loop
        let active_loop = self.check_active_loop();

        // Check for active agents
        let active_agents = self.check_active_agents();

        EnvInfo {
            directory,
            project_name,
            project_type,
            active_loop,
            active_agents,
        }
    }

    /// Build the full handoff document
    fn build_document(
        &self,
        name: &str,
        from: &str,
        to: Option<&str>,
        hours: u64,
        git_info: &GitInfo,
        env_info: &EnvInfo,
    ) -> String {
        let mut doc = String::new();
        let now = Local::now();

        // Header
        doc.push_str(&format!("# Handoff: {}\n\n", name));
        doc.push_str(&format!("**Created**: {}\n", now.format("%Y-%m-%d %H:%M")));
        doc.push_str(&format!("**From**: {}\n", from));
        if let Some(to) = to {
            doc.push_str(&format!("**To**: {}\n", to));
        }
        doc.push_str(&format!("**Hours covered**: {}\n\n", hours));
        doc.push_str("---\n\n");

        // Current task section
        doc.push_str("## Current Task\n\n");
        doc.push_str("_What are you working on?_\n\n");
        doc.push_str("> \n\n");

        // Environment section
        doc.push_str("## Environment\n\n");
        doc.push_str(&format!("- **Directory**: {}\n", env_info.directory));
        doc.push_str(&format!("- **Project**: {}\n", env_info.project_name));
        if let Some(ref pt) = env_info.project_type {
            doc.push_str(&format!("- **Type**: {}\n", pt));
        }
        if let Some(ref loop_status) = env_info.active_loop {
            doc.push_str(&format!("- **Active Loop**: {}\n", loop_status));
        }
        if env_info.active_agents > 0 {
            doc.push_str(&format!("- **Active Agents**: {}\n", env_info.active_agents));
        }
        doc.push_str("\n");

        // Git activity section
        if git_info.branch.is_some() {
            doc.push_str("## Git Activity\n\n");

            // Recent commits
            if !git_info.recent_commits.is_empty() {
                doc.push_str("### Recent Commits\n");
                doc.push_str("```\n");
                for commit in &git_info.recent_commits {
                    doc.push_str(commit);
                    doc.push_str("\n");
                }
                doc.push_str("```\n\n");
            }

            // Current state
            doc.push_str("### Current State\n");
            if let Some(ref branch) = git_info.branch {
                doc.push_str(&format!("- Branch: {}\n", branch));
            }
            doc.push_str(&format!("- Status: {} files changed\n", git_info.changed_files));
            if git_info.ahead > 0 {
                doc.push_str(&format!("- {} commits ahead of remote\n", git_info.ahead));
            }
            if git_info.behind > 0 {
                doc.push_str(&format!("- {} commits behind remote\n", git_info.behind));
            }
            doc.push_str("\n");
        }

        // Journal activity (placeholder - requires journal integration)
        doc.push_str("## Recent Activity\n\n");
        doc.push_str(&format!("_No journal events in the last {} hours_\n\n", hours));

        // Blockers section
        doc.push_str("## Blockers & Issues\n\n");
        doc.push_str("_Any blockers or known issues?_\n\n");
        doc.push_str("- \n\n");

        // Next steps section
        doc.push_str("## Next Steps\n\n");
        doc.push_str("_What should be done next?_\n\n");
        doc.push_str("1. \n\n");

        // Open questions section
        doc.push_str("## Open Questions\n\n");
        doc.push_str("_Anything uncertain or needing decision?_\n\n");
        doc.push_str("- \n\n");

        // Important context section
        doc.push_str("## Important Context\n\n");
        doc.push_str("_Anything else the next person should know?_\n\n");
        doc.push_str("> \n\n");

        // Footer
        doc.push_str("---\n\n");
        doc.push_str("_Generated by Daedalos handoff_\n");

        doc
    }

    /// Check if current directory is a git repository
    fn is_git_repo(&self) -> bool {
        Command::new("git")
            .args(["rev-parse", "--git-dir"])
            .current_dir(&self.cwd)
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
    }

    /// Run a git command and return stdout as string
    fn run_git(&self, args: &[&str]) -> Option<String> {
        Command::new("git")
            .args(args)
            .current_dir(&self.cwd)
            .output()
            .ok()
            .filter(|o| o.status.success())
            .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
            .filter(|s| !s.is_empty())
    }

    /// Check for active loop (via loop status command)
    fn check_active_loop(&self) -> Option<String> {
        let home = dirs::home_dir()?;
        let loop_bin = home.join(".local/bin/loop");

        if !loop_bin.exists() {
            return None;
        }

        Command::new(&loop_bin)
            .args(["status"])
            .output()
            .ok()
            .filter(|o| o.status.success())
            .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
            .filter(|s| !s.contains("No active") && !s.is_empty())
    }

    /// Check for active agents (via agent list command)
    fn check_active_agents(&self) -> usize {
        let home = match dirs::home_dir() {
            Some(h) => h,
            None => return 0,
        };
        let agent_bin = home.join(".local/bin/agent");

        if !agent_bin.exists() {
            return 0;
        }

        Command::new(&agent_bin)
            .args(["list"])
            .output()
            .ok()
            .map(|o| {
                // Count lines with active indicator
                String::from_utf8_lossy(&o.stdout)
                    .lines()
                    .filter(|line| line.contains("active") || line.contains("[running]"))
                    .count()
            })
            .unwrap_or(0)
    }
}

impl Default for HandoffGenerator {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generator_creation() {
        let gen = HandoffGenerator::new();
        assert!(!gen.cwd.is_empty());
    }

    #[test]
    fn test_quick_status() {
        let gen = HandoffGenerator::new();
        let status = gen.quick_status(4).unwrap();
        assert!(status.contains("Quick Status"));
        assert!(status.contains("Location:"));
    }

    #[test]
    fn test_generate_handoff() {
        let gen = HandoffGenerator::new();
        let handoff = gen.generate(Some("test-handoff"), None, 8).unwrap();

        assert_eq!(handoff.name, "test-handoff");
        assert_eq!(handoff.hours, 8);
        assert!(handoff.content.contains("# Handoff: test-handoff"));
        assert!(handoff.content.contains("## Current Task"));
        assert!(handoff.content.contains("## Next Steps"));
    }

    #[test]
    fn test_auto_name_generation() {
        let gen = HandoffGenerator::new();
        let handoff = gen.generate(None, None, 8).unwrap();

        assert!(handoff.name.starts_with("handoff-"));
    }
}
