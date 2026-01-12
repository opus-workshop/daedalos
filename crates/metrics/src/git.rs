//! Git statistics collection
//!
//! Reads commit history and calculates statistics directly from git.

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;
use std::process::Command;

/// Git statistics for a time period
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct GitStats {
    /// Number of commits
    pub commits: u32,
    /// Lines added
    pub insertions: u32,
    /// Lines removed
    pub deletions: u32,
    /// Number of unique files modified
    pub files_touched: u32,
    /// Commits per author
    #[serde(default)]
    pub by_author: HashMap<String, u32>,
    /// Commits by hour (0-23)
    #[serde(default)]
    pub by_hour: HashMap<u32, u32>,
}

impl GitStats {
    /// Calculate net lines (insertions - deletions)
    pub fn net_lines(&self) -> i32 {
        self.insertions as i32 - self.deletions as i32
    }

    /// Find peak commit hour
    pub fn peak_hour(&self) -> Option<(u32, u32)> {
        self.by_hour
            .iter()
            .max_by_key(|(_, count)| *count)
            .map(|(hour, count)| (*hour, *count))
    }
}

/// Get git statistics for a time range
pub fn get_git_stats(since: &str, until: &str, project_dir: Option<&Path>) -> Result<GitStats> {
    let dir = project_dir
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|| std::env::current_dir().unwrap_or_default());

    // Check if it's a git repo
    if !is_git_repo(&dir) {
        return Ok(GitStats::default());
    }

    let mut stats = GitStats::default();

    // Get commit count
    stats.commits = get_commit_count(&dir, since, until)?;

    // Get insertions/deletions
    let (ins, del) = get_line_changes(&dir, since, until)?;
    stats.insertions = ins;
    stats.deletions = del;

    // Get files touched
    stats.files_touched = get_files_touched(&dir, since, until)?;

    // Get commits by author
    stats.by_author = get_commits_by_author(&dir, since, until)?;

    // Get commits by hour
    stats.by_hour = get_commits_by_hour(&dir, since, until)?;

    Ok(stats)
}

/// Get git stats for today
pub fn get_today_stats(project_dir: Option<&Path>) -> Result<GitStats> {
    get_git_stats("today", "now", project_dir)
}

/// Get git stats for the last N days
pub fn get_stats_for_days(days: u32, project_dir: Option<&Path>) -> Result<GitStats> {
    let since = format!("{} days ago", days);
    get_git_stats(&since, "now", project_dir)
}

/// Check if directory is a git repository
fn is_git_repo(dir: &Path) -> bool {
    Command::new("git")
        .args(["-C", &dir.to_string_lossy(), "rev-parse", "--git-dir"])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// Get commit count
fn get_commit_count(dir: &Path, since: &str, until: &str) -> Result<u32> {
    let output = Command::new("git")
        .args([
            "-C",
            &dir.to_string_lossy(),
            "log",
            &format!("--since={}", since),
            &format!("--until={}", until),
            "--oneline",
        ])
        .output()?;

    if !output.status.success() {
        return Ok(0);
    }

    let count = String::from_utf8_lossy(&output.stdout)
        .lines()
        .count() as u32;
    Ok(count)
}

/// Get line insertions and deletions
fn get_line_changes(dir: &Path, since: &str, until: &str) -> Result<(u32, u32)> {
    let output = Command::new("git")
        .args([
            "-C",
            &dir.to_string_lossy(),
            "log",
            &format!("--since={}", since),
            &format!("--until={}", until),
            "--shortstat",
        ])
        .output()?;

    if !output.status.success() {
        return Ok((0, 0));
    }

    let text = String::from_utf8_lossy(&output.stdout);
    let mut insertions = 0u32;
    let mut deletions = 0u32;

    for line in text.lines() {
        // Parse lines like: " 3 files changed, 45 insertions(+), 12 deletions(-)"
        if line.contains("insertion") || line.contains("deletion") {
            let parts: Vec<&str> = line.split_whitespace().collect();
            for (i, part) in parts.iter().enumerate() {
                if part.starts_with("insertion") && i > 0 {
                    if let Ok(n) = parts[i - 1].parse::<u32>() {
                        insertions += n;
                    }
                }
                if part.starts_with("deletion") && i > 0 {
                    if let Ok(n) = parts[i - 1].parse::<u32>() {
                        deletions += n;
                    }
                }
            }
        }
    }

    Ok((insertions, deletions))
}

/// Get count of unique files touched
fn get_files_touched(dir: &Path, since: &str, until: &str) -> Result<u32> {
    let output = Command::new("git")
        .args([
            "-C",
            &dir.to_string_lossy(),
            "log",
            &format!("--since={}", since),
            &format!("--until={}", until),
            "--name-only",
            "--pretty=format:",
        ])
        .output()?;

    if !output.status.success() {
        return Ok(0);
    }

    let text = String::from_utf8_lossy(&output.stdout);
    let unique_files: std::collections::HashSet<&str> = text
        .lines()
        .filter(|l| !l.is_empty())
        .collect();

    Ok(unique_files.len() as u32)
}

/// Get commits grouped by author
fn get_commits_by_author(dir: &Path, since: &str, until: &str) -> Result<HashMap<String, u32>> {
    let output = Command::new("git")
        .args([
            "-C",
            &dir.to_string_lossy(),
            "log",
            &format!("--since={}", since),
            &format!("--until={}", until),
            "--format=%an",
        ])
        .output()?;

    if !output.status.success() {
        return Ok(HashMap::new());
    }

    let text = String::from_utf8_lossy(&output.stdout);
    let mut by_author = HashMap::new();

    for author in text.lines() {
        if !author.is_empty() {
            *by_author.entry(author.to_string()).or_insert(0) += 1;
        }
    }

    Ok(by_author)
}

/// Get commits grouped by hour (0-23)
fn get_commits_by_hour(dir: &Path, since: &str, until: &str) -> Result<HashMap<u32, u32>> {
    let output = Command::new("git")
        .args([
            "-C",
            &dir.to_string_lossy(),
            "log",
            &format!("--since={}", since),
            &format!("--until={}", until),
            "--format=%H",
        ])
        .output()?;

    if !output.status.success() {
        return Ok(HashMap::new());
    }

    let text = String::from_utf8_lossy(&output.stdout);
    let mut by_hour = HashMap::new();

    for hour_str in text.lines() {
        if let Ok(hour) = hour_str.trim().parse::<u32>() {
            if hour < 24 {
                *by_hour.entry(hour).or_insert(0) += 1;
            }
        }
    }

    Ok(by_hour)
}

/// Get daily commit counts for a range of days
pub fn get_daily_commits(days: u32, project_dir: Option<&Path>) -> Result<Vec<(String, u32)>> {
    let dir = project_dir
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|| std::env::current_dir().unwrap_or_default());

    if !is_git_repo(&dir) {
        return Ok(vec![]);
    }

    let mut daily = Vec::new();

    for i in (0..days).rev() {
        let date = (chrono::Utc::now() - chrono::Duration::days(i as i64))
            .format("%Y-%m-%d")
            .to_string();

        let since = format!("{} 00:00", date);
        let until = format!("{} 23:59", date);

        let count = get_commit_count(&dir, &since, &until)?;
        daily.push((date, count));
    }

    Ok(daily)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_git_stats_calculations() {
        let stats = GitStats {
            commits: 10,
            insertions: 100,
            deletions: 30,
            files_touched: 5,
            ..Default::default()
        };

        assert_eq!(stats.net_lines(), 70);
    }

    #[test]
    fn test_peak_hour() {
        let mut stats = GitStats::default();
        stats.by_hour.insert(9, 5);
        stats.by_hour.insert(14, 10);
        stats.by_hour.insert(16, 3);

        let peak = stats.peak_hour().unwrap();
        assert_eq!(peak, (14, 10));
    }
}
