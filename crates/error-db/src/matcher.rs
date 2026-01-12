//! Fuzzy pattern matching for error messages
//!
//! Supports:
//! - Variable placeholders (X, Y, Z match any text)
//! - Fuzzy string matching using word overlap
//! - Normalization (removes line numbers, paths, quoted values)

use crate::db::{ErrorDatabase, Pattern, Solution};
use anyhow::Result;
use regex::Regex;
use std::collections::HashMap;

/// Match result with score
#[derive(Debug, Clone)]
pub struct Match {
    pub pattern: Pattern,
    pub score: f64,
    pub solutions: Vec<Solution>,
}

/// Pattern matcher for error messages
pub struct PatternMatcher<'a> {
    db: &'a ErrorDatabase,
}

impl<'a> PatternMatcher<'a> {
    /// Create a new pattern matcher
    pub fn new(db: &'a ErrorDatabase) -> Self {
        Self { db }
    }

    /// Search for the best matching pattern
    pub fn search(&self, error_text: &str) -> Result<Option<Match>> {
        let matches = self.find_matches(error_text, 0.4)?;
        Ok(matches.into_iter().next())
    }

    /// Find all patterns matching the error text above threshold
    pub fn find_matches(&self, error_text: &str, threshold: f64) -> Result<Vec<Match>> {
        let patterns = self.db.get_all_patterns()?;
        let normalized_error = normalize(error_text);

        let mut matches = Vec::new();

        for pattern in patterns {
            // Try variable matching first (highest priority)
            let score = if let Some(s) = variable_match(&pattern.pattern, error_text) {
                s
            } else {
                // Fall back to fuzzy matching
                let normalized_pattern = normalize(&pattern.pattern);
                fuzzy_match(&normalized_pattern, &normalized_error)
            };

            if score >= threshold {
                let solutions = self.db.get_solutions(&pattern.id)?;
                matches.push(Match {
                    pattern,
                    score,
                    solutions,
                });
            }
        }

        // Sort by score descending
        matches.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));

        Ok(matches)
    }
}

/// Normalize text for comparison
///
/// Removes variable parts that differ between runs:
/// - Line numbers
/// - File paths
/// - Quoted values
fn normalize(text: &str) -> String {
    let mut result = text.to_string();

    // Remove line:column numbers
    let line_col_re = Regex::new(r":\d+:\d+").unwrap();
    result = line_col_re.replace_all(&result, ":N:N").to_string();

    // Remove "line N" patterns
    let line_re = Regex::new(r" line \d+").unwrap();
    result = line_re.replace_all(&result, " line N").to_string();

    // Remove Unix paths
    let unix_path_re = Regex::new(r"/[\w/.-]+").unwrap();
    result = unix_path_re.replace_all(&result, "/PATH").to_string();

    // Remove Windows paths
    let win_path_re = Regex::new(r"[A-Za-z]:\\[\w\\.-]+").unwrap();
    result = win_path_re.replace_all(&result, "PATH").to_string();

    // Remove single-quoted values
    let sq_re = Regex::new(r"'[^']*'").unwrap();
    result = sq_re.replace_all(&result, "'X'").to_string();

    // Remove double-quoted values
    let dq_re = Regex::new(r#""[^"]*""#).unwrap();
    result = dq_re.replace_all(&result, "\"X\"").to_string();

    // Normalize whitespace
    let ws_re = Regex::new(r"\s+").unwrap();
    result = ws_re.replace_all(&result, " ").to_string();

    result.to_lowercase().trim().to_string()
}

/// Match pattern with variable placeholders (X, Y, Z)
///
/// Returns 1.0 if pattern matches with variables, None otherwise
fn variable_match(pattern: &str, error: &str) -> Option<f64> {
    // Escape the pattern for regex, then replace X, Y, Z with capture groups
    let mut regex_pattern = regex::escape(pattern);

    // Replace X, Y, Z with non-greedy match-all patterns
    // We need to handle them carefully to avoid issues with escaped characters
    regex_pattern = regex_pattern.replace("X", ".+?");
    regex_pattern = regex_pattern.replace("Y", ".+?");
    regex_pattern = regex_pattern.replace("Z", ".+?");

    // Try to compile and match
    match Regex::new(&format!("(?i){}", regex_pattern)) {
        Ok(re) => {
            if re.is_match(error) {
                Some(1.0)
            } else {
                None
            }
        }
        Err(_) => None,
    }
}

/// Fuzzy string matching using word overlap and substring matching
fn fuzzy_match(pattern: &str, error: &str) -> f64 {
    // Check if pattern is a substring (high confidence)
    if error.contains(pattern) {
        return 0.9;
    }

    // Check if error contains pattern
    if pattern.len() > 3 && error.contains(pattern) {
        return 0.85;
    }

    // Word-based matching
    let pattern_words: Vec<&str> = pattern.split_whitespace().collect();
    let error_words: Vec<&str> = error.split_whitespace().collect();

    if pattern_words.is_empty() {
        return 0.0;
    }

    // Count matching words
    let mut matches = 0;
    for pw in &pattern_words {
        if error_words.iter().any(|ew| ew.contains(pw) || pw.contains(ew)) {
            matches += 1;
        }
    }

    let word_ratio = matches as f64 / pattern_words.len() as f64;

    // Also compute character-level similarity (Jaccard on character n-grams)
    let char_sim = ngram_similarity(pattern, error, 3);

    // Return the better of the two
    f64::max(word_ratio, char_sim)
}

/// Compute Jaccard similarity using character n-grams
fn ngram_similarity(a: &str, b: &str, n: usize) -> f64 {
    let ngrams_a = get_ngrams(a, n);
    let ngrams_b = get_ngrams(b, n);

    if ngrams_a.is_empty() || ngrams_b.is_empty() {
        return 0.0;
    }

    let intersection: usize = ngrams_a
        .iter()
        .filter(|(k, _)| ngrams_b.contains_key(*k))
        .map(|(k, v)| v.min(ngrams_b.get(k).unwrap_or(&0)))
        .sum();

    let union: usize = {
        let mut all_keys: std::collections::HashSet<_> = ngrams_a.keys().collect();
        all_keys.extend(ngrams_b.keys());
        all_keys
            .iter()
            .map(|k| ngrams_a.get(*k).unwrap_or(&0).max(ngrams_b.get(*k).unwrap_or(&0)))
            .sum()
    };

    if union == 0 {
        return 0.0;
    }

    intersection as f64 / union as f64
}

/// Get character n-grams from a string
fn get_ngrams(s: &str, n: usize) -> HashMap<String, usize> {
    let chars: Vec<char> = s.chars().collect();
    let mut ngrams = HashMap::new();

    if chars.len() < n {
        return ngrams;
    }

    for window in chars.windows(n) {
        let ngram: String = window.iter().collect();
        *ngrams.entry(ngram).or_insert(0) += 1;
    }

    ngrams
}

/// Format a match result for display
pub fn format_match(m: &Match, verbose: bool) -> String {
    let mut lines = Vec::new();

    // Header with confidence indicator
    let confidence_pct = (m.score * 100.0) as i32;
    let indicator = if confidence_pct >= 80 {
        "HIGH"
    } else if confidence_pct >= 60 {
        "MED"
    } else {
        "LOW"
    };

    lines.push(format!("[{}] Match ({}%): {}", indicator, confidence_pct, m.pattern.pattern));

    if let Some(ref lang) = m.pattern.language {
        lines.push(format!("     Language: {}", lang));
    }

    lines.push(String::new());

    // Best solution
    if !m.solutions.is_empty() {
        let best = &m.solutions[0];
        lines.push("SOLUTION:".to_string());
        lines.push("-".repeat(50));
        for line in best.solution.lines() {
            lines.push(format!("  {}", line));
        }
        lines.push("-".repeat(50));

        // Stats
        let total = best.success_count + best.failure_count;
        if total > 0 {
            let success_rate = (best.success_count as f64 / total as f64) * 100.0;
            lines.push(format!(
                "Success rate: {:.0}% ({}/{})",
                success_rate, best.success_count, total
            ));
        }

        // Auto-fix command
        if let Some(ref cmd) = best.command {
            lines.push(format!("\nAuto-fix command: {}", cmd));
        }

        if verbose && m.solutions.len() > 1 {
            lines.push(format!("\n({} more solutions available)", m.solutions.len() - 1));
        }
    } else {
        lines.push("No solutions recorded for this pattern.".to_string());
    }

    lines.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_normalize() {
        let input = "Error at /home/user/project/file.rs:10:5: Cannot find 'foo'";
        let normalized = normalize(input);
        assert!(normalized.contains("/path"));
        assert!(normalized.contains(":n:n"));
        assert!(normalized.contains("'x'"));
    }

    #[test]
    fn test_variable_match() {
        assert!(variable_match("Cannot find module 'X'", "Cannot find module 'lodash'").is_some());
        assert!(variable_match("Type X is not assignable to type Y",
            "Type 'string' is not assignable to type 'number'").is_some());
        assert!(variable_match("Cannot find module 'X'", "Something completely different").is_none());
    }

    #[test]
    fn test_fuzzy_match() {
        // Substring match should get high score
        let score = fuzzy_match("module not found", "error: module not found in project");
        assert!(score >= 0.9);

        // Word overlap should get decent score
        let score = fuzzy_match("cannot find module", "module cannot be found");
        assert!(score >= 0.5);

        // Unrelated should get low score
        let score = fuzzy_match("syntax error", "network connection refused");
        assert!(score < 0.5);
    }

    #[test]
    fn test_ngram_similarity() {
        let sim = ngram_similarity("hello", "hello", 3);
        assert!((sim - 1.0).abs() < 0.01);

        // hello and hallo share "llo" trigram, so similarity should be > 0
        let sim = ngram_similarity("hello", "hallo", 3);
        assert!(sim > 0.0 && sim < 1.0);

        let sim = ngram_similarity("hello", "world", 3);
        assert!(sim < 0.5);
    }
}
