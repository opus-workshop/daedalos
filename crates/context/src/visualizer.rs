//! Output formatting for context status
//!
//! Visual progress bars, color-coded thresholds, and formatted output.
//! Green < 50%, Yellow < 70%, Orange < 85%, Red >= 85%

#![allow(dead_code)]

use crate::estimator::TokenEstimator;
use crate::tracker::{
    CheckpointSummary, CompactionSuggestion, ContextStatus, FileInContext, WarningLevel,
};

/// ANSI color codes
mod colors {
    pub const RESET: &str = "\x1b[0m";
    pub const BOLD: &str = "\x1b[1m";
    pub const DIM: &str = "\x1b[2m";
    pub const GREEN: &str = "\x1b[32m";
    pub const YELLOW: &str = "\x1b[33m";
    pub const ORANGE: &str = "\x1b[38;5;208m";
    pub const RED: &str = "\x1b[31m";
    pub const BLUE: &str = "\x1b[34m";
    pub const CYAN: &str = "\x1b[36m";
}

/// Get color based on usage percentage
fn get_usage_color(percentage: f64, use_color: bool) -> &'static str {
    if !use_color {
        return "";
    }
    if percentage < 50.0 {
        colors::GREEN
    } else if percentage < 70.0 {
        colors::YELLOW
    } else if percentage < 85.0 {
        colors::ORANGE
    } else {
        colors::RED
    }
}

/// Create a progress bar
fn format_bar(percentage: f64, width: usize) -> String {
    let filled = ((width as f64 * percentage.min(100.0) / 100.0) as usize).min(width);
    let empty = width - filled;
    format!("{}{}", "█".repeat(filled), "░".repeat(empty))
}

/// Format context status for display
pub fn format_status(status: &ContextStatus, use_color: bool) -> String {
    let c = if use_color { colors::RESET } else { "" };

    let pct = status.percentage;
    let color = get_usage_color(pct, use_color);
    let bar = format_bar(pct, 40);

    let used_str = TokenEstimator::format_count(status.used);
    let max_str = TokenEstimator::format_count(status.max);
    let remaining_str = TokenEstimator::format_count(status.remaining);

    let mut lines = vec![
        "+-----------------------------------------------------------+".to_string(),
        "| CONTEXT BUDGET                                            |".to_string(),
        "+-----------------------------------------------------------+".to_string(),
        format!(
            "| {}{}{} {:5.1}%            |",
            color, bar, c, pct
        ),
        format!(
            "| Used: {:>6} / {:>6} tokens                        |",
            used_str, max_str
        ),
        format!(
            "| Remaining: {:>6} tokens                                |",
            remaining_str
        ),
        "|                                                           |".to_string(),
    ];

    // Warning based on level
    match status.warning_level {
        WarningLevel::Critical => {
            let warn_color = if use_color { colors::RED } else { "" };
            lines.push(format!(
                "| {}!  CRITICAL: Consider starting fresh session{}            |",
                warn_color, c
            ));
        }
        WarningLevel::High => {
            let warn_color = if use_color { colors::ORANGE } else { "" };
            lines.push(format!(
                "| {}!  HIGH: Consider running 'context compact'{}             |",
                warn_color, c
            ));
        }
        WarningLevel::Moderate => {
            let warn_color = if use_color { colors::YELLOW } else { "" };
            lines.push(format!(
                "| {}i  Moderate usage - monitor as you continue{}             |",
                warn_color, c
            ));
        }
        WarningLevel::Ok => {}
    }

    lines.push("+-----------------------------------------------------------+".to_string());

    lines.join("\n")
}

/// Format detailed breakdown for display
pub fn format_breakdown(status: &ContextStatus, use_color: bool) -> String {
    let c = if use_color { colors::RESET } else { "" };
    let bold = if use_color { colors::BOLD } else { "" };

    let breakdown = &status.breakdown;
    let total = status.used.max(1);

    let mut lines = vec![
        "+-----------------------------------------------------------+".to_string(),
        "| CONTEXT BREAKDOWN                                         |".to_string(),
        "+-----------------------------------------------------------+".to_string(),
    ];

    // Create sorted list of categories
    let mut categories: Vec<(&str, usize)> = vec![
        ("system", breakdown.system),
        ("user", breakdown.user),
        ("assistant", breakdown.assistant),
        ("tool_calls", breakdown.tool_calls),
        ("tool_results", breakdown.tool_results),
        ("files_read", breakdown.files_read),
    ];
    categories.sort_by(|a, b| b.1.cmp(&a.1));

    for (category, count) in categories {
        if count == 0 {
            continue;
        }

        let pct = count as f64 / total as f64 * 100.0;
        let bar_width = 20;
        let bar_filled = ((bar_width as f64 * pct / 100.0) as usize).min(bar_width);
        let bar = format!(
            "{}{}",
            "\u{2593}".repeat(bar_filled),
            "░".repeat(bar_width - bar_filled)
        );

        let count_str = TokenEstimator::format_count(count);
        lines.push(format!(
            "| {:<15} {} {:>6} ({:4.1}%) |",
            category, bar, count_str, pct
        ));
    }

    lines.push("|                                                           |".to_string());
    lines.push(format!(
        "| {}Total:{} {:>44} tokens |",
        bold,
        c,
        TokenEstimator::format_count(total)
    ));
    lines.push("+-----------------------------------------------------------+".to_string());

    lines.join("\n")
}

/// Format files in context for display
pub fn format_files(files: &[FileInContext], limit: usize, use_color: bool) -> String {
    let c = if use_color { colors::RESET } else { "" };
    let bold = if use_color { colors::BOLD } else { "" };
    let dim = if use_color { colors::DIM } else { "" };

    if files.is_empty() {
        return format!("{}No files tracked in current context{}", dim, c);
    }

    let mut lines = vec![
        "+-----------------------------------------------------------+".to_string(),
        "| FILES IN CONTEXT                                          |".to_string(),
        "+-----------------------------------------------------------+".to_string(),
    ];

    for f in files.iter().take(limit) {
        let mut path = f.path.clone();

        // Truncate path if needed
        if path.len() > 40 {
            path = format!("...{}", &path[path.len() - 37..]);
        }

        let tokens_str = TokenEstimator::format_count(f.tokens);
        lines.push(format!("| {:<40} {:>8} |", path, tokens_str));
    }

    if files.len() > limit {
        lines.push(format!(
            "| {}... and {} more files{}                                 |",
            dim,
            files.len() - limit,
            c
        ));
    }

    let total_tokens: usize = files.iter().map(|f| f.tokens).sum();
    lines.push("|                                                           |".to_string());
    lines.push(format!(
        "| {}Total file tokens:{} {:>32} |",
        bold,
        c,
        TokenEstimator::format_count(total_tokens)
    ));
    lines.push("+-----------------------------------------------------------+".to_string());

    lines.join("\n")
}

/// Format compaction suggestions for display
pub fn format_suggestions(suggestions: &[CompactionSuggestion], use_color: bool) -> String {
    let c = if use_color { colors::RESET } else { "" };
    let green = if use_color { colors::GREEN } else { "" };

    if suggestions.is_empty() {
        return format!(
            "{}OK No compaction suggestions - context usage is healthy{}",
            green, c
        );
    }

    let mut lines = vec![
        "+-----------------------------------------------------------+".to_string(),
        "| COMPACTION SUGGESTIONS                                    |".to_string(),
        "+-----------------------------------------------------------+".to_string(),
    ];

    for s in suggestions {
        let mut desc = s.description.clone();
        if desc.len() > 50 {
            desc = format!("{}...", &desc[..47]);
        }

        let savings_str = TokenEstimator::format_count(s.savings);
        lines.push(format!("| * {:<52} |", desc));
        lines.push(format!(
            "|   {}Potential savings: ~{} tokens{}                |",
            green, savings_str, c
        ));
        lines.push("|                                                           |".to_string());
    }

    lines.push("+-----------------------------------------------------------+".to_string());

    lines.join("\n")
}

/// Format checkpoint list for display
pub fn format_checkpoints(checkpoints: &[CheckpointSummary], use_color: bool) -> String {
    let c = if use_color { colors::RESET } else { "" };
    let bold = if use_color { colors::BOLD } else { "" };
    let dim = if use_color { colors::DIM } else { "" };

    if checkpoints.is_empty() {
        return format!("{}No checkpoints saved{}", dim, c);
    }

    let mut lines = vec![
        format!(
            "{}{:<20} {:<20} {:>10}{}",
            bold, "NAME", "CREATED", "TOKENS", c
        ),
        "-".repeat(52),
    ];

    for cp in checkpoints {
        let name = if cp.name.len() > 18 {
            format!("{}...", &cp.name[..15])
        } else {
            cp.name.clone()
        };

        let created = if cp.created.len() > 19 {
            cp.created[..19].to_string()
        } else {
            cp.created.clone()
        };

        let tokens_str = TokenEstimator::format_count(cp.tokens);

        lines.push(format!("{:<20} {:<20} {:>10}", name, created, tokens_str));
    }

    lines.join("\n")
}
