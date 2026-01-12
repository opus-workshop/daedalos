//! Display and formatting utilities
//!
//! ASCII charts, progress bars, and formatted output.

use crate::db::DailyMetrics;
use serde::{Deserialize, Serialize};

/// ANSI color codes (only used when terminal supports it)
pub struct Colors {
    pub red: &'static str,
    pub green: &'static str,
    pub yellow: &'static str,
    pub blue: &'static str,
    pub cyan: &'static str,
    pub magenta: &'static str,
    pub bold: &'static str,
    pub dim: &'static str,
    pub reset: &'static str,
}

impl Colors {
    /// Get colors for terminal output
    pub fn new(color_enabled: bool) -> Self {
        if color_enabled {
            Self {
                red: "\x1b[0;31m",
                green: "\x1b[0;32m",
                yellow: "\x1b[0;33m",
                blue: "\x1b[0;34m",
                cyan: "\x1b[0;36m",
                magenta: "\x1b[0;35m",
                bold: "\x1b[1m",
                dim: "\x1b[2m",
                reset: "\x1b[0m",
            }
        } else {
            Self {
                red: "",
                green: "",
                yellow: "",
                blue: "",
                cyan: "",
                magenta: "",
                bold: "",
                dim: "",
                reset: "",
            }
        }
    }

    /// Check if stdout is a TTY (terminal)
    pub fn is_tty() -> bool {
        atty_check()
    }

    /// Get colors based on TTY detection
    pub fn auto() -> Self {
        Self::new(Self::is_tty())
    }
}

/// Check if stdout is a TTY
fn atty_check() -> bool {
    // Simple check using libc on unix
    #[cfg(unix)]
    unsafe {
        libc::isatty(libc::STDOUT_FILENO) != 0
    }

    #[cfg(not(unix))]
    true
}

/// Draw a simple bar chart
pub fn draw_bar(value: u32, max: u32, width: usize) -> String {
    let max = if max == 0 { 1 } else { max };
    let filled = ((value as usize) * width / (max as usize)).min(width);
    let empty = width - filled;

    format!("{}{}", "\u{2588}".repeat(filled), "\u{2591}".repeat(empty))
}

/// Draw a colored bar
pub fn draw_bar_colored(value: u32, max: u32, width: usize, colors: &Colors) -> String {
    let bar = draw_bar(value, max, width);
    format!("{}{}{}", colors.green, bar, colors.reset)
}

/// Format duration as "Xh Ym"
pub fn format_duration(minutes: u32) -> String {
    let hours = minutes / 60;
    let mins = minutes % 60;
    if hours > 0 {
        format!("{}h {}m", hours, mins)
    } else {
        format!("{}m", mins)
    }
}

/// Format a signed number with + or - prefix
pub fn format_signed(value: i32) -> String {
    if value >= 0 {
        format!("+{}", value)
    } else {
        format!("{}", value)
    }
}

/// Format a percentage change with arrow
pub fn format_trend(current: u32, previous: u32, colors: &Colors) -> String {
    if previous == 0 {
        if current > 0 {
            format!("{}^ new{}", colors.green, colors.reset)
        } else {
            format!("{}={}", colors.dim, colors.reset)
        }
    } else {
        let change = ((current as i32 - previous as i32) * 100) / previous as i32;
        if change > 10 {
            format!("{}^ {}%{}", colors.green, change, colors.reset)
        } else if change < -10 {
            format!("{}v {}%{}", colors.red, change.abs(), colors.reset)
        } else {
            format!("{}= {}%{}", colors.yellow, change, colors.reset)
        }
    }
}

/// Productivity score calculation and display
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProductivityScore {
    pub score: u32,
    pub breakdown: Vec<(String, u32)>,
}

impl ProductivityScore {
    /// Calculate productivity score from daily metrics
    pub fn from_metrics(metrics: &DailyMetrics) -> Self {
        let mut score = 0u32;
        let mut breakdown = Vec::new();

        // Commits today
        if metrics.commits > 0 {
            score += 20;
            breakdown.push(("Activity (commits > 0)".to_string(), 20));
        }
        if metrics.commits > 5 {
            score += 10;
            breakdown.push(("Sustained activity (commits > 5)".to_string(), 10));
        }

        // Focus sessions
        if metrics.focus_completed > 0 {
            score += 20;
            breakdown.push(("Intentional work (focus completed)".to_string(), 20));
        }
        if metrics.focus_minutes > 60 {
            score += 20;
            breakdown.push(("Substantial depth (60+ min focus)".to_string(), 20));
        }
        if metrics.focus_minutes > 180 {
            score += 10;
            breakdown.push(("Exceptional depth (180+ min focus)".to_string(), 10));
        }

        // Errors
        if metrics.errors < 5 {
            score += 10;
            breakdown.push(("Smooth workflow (< 5 errors)".to_string(), 10));
        }

        // Net positive lines
        if metrics.insertions > metrics.deletions {
            score += 10;
            breakdown.push(("Building (net positive lines)".to_string(), 10));
        }

        Self {
            score: score.min(100),
            breakdown,
        }
    }

    /// Get color for score
    pub fn color<'a>(&self, colors: &'a Colors) -> &'a str {
        if self.score >= 70 {
            colors.green
        } else if self.score >= 40 {
            colors.yellow
        } else {
            colors.red
        }
    }

    /// Format as string with color
    pub fn format(&self, colors: &Colors) -> String {
        format!(
            "{}{}/100{}",
            self.color(colors),
            self.score,
            colors.reset
        )
    }
}

/// Format a section header
pub fn section_header(title: &str, colors: &Colors) -> String {
    format!("{}{}{}", colors.cyan, title, colors.reset)
}

/// Format a bold title
pub fn title(text: &str, colors: &Colors) -> String {
    format!("{}{}{}", colors.bold, text, colors.reset)
}

/// Format today's date nicely
pub fn format_today() -> String {
    chrono::Local::now().format("%A, %B %d, %Y").to_string()
}

/// Format a list of top items with counts
pub fn format_top_list(items: &[(String, u32)], limit: usize, _colors: &Colors) -> String {
    let mut result = String::new();
    for (name, count) in items.iter().take(limit) {
        result.push_str(&format!("  {}: {} commits\n", name, count));
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_draw_bar() {
        let bar = draw_bar(5, 10, 10);
        assert_eq!(bar.chars().count(), 10);
    }

    #[test]
    fn test_format_duration() {
        assert_eq!(format_duration(45), "45m");
        assert_eq!(format_duration(90), "1h 30m");
        assert_eq!(format_duration(120), "2h 0m");
    }

    #[test]
    fn test_format_signed() {
        assert_eq!(format_signed(10), "+10");
        assert_eq!(format_signed(-5), "-5");
        assert_eq!(format_signed(0), "+0");
    }

    #[test]
    fn test_productivity_score() {
        let metrics = DailyMetrics {
            commits: 10,
            insertions: 100,
            deletions: 30,
            focus_sessions: 3,
            focus_minutes: 90,
            focus_completed: 2,
            errors: 2,
            ..Default::default()
        };

        let score = ProductivityScore::from_metrics(&metrics);
        // commits > 0: 20, commits > 5: 10, focus_completed > 0: 20,
        // focus > 60: 20, errors < 5: 10, net positive: 10 = 90
        assert_eq!(score.score, 90);
    }
}
