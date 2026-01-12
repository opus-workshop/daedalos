//! Token estimation for context tracking
//!
//! Uses character heuristic (~4 chars per token) as fallback.
//! Reasonably accurate (< 10% error) for English text and code.

#![allow(dead_code)]

/// Token estimator using character heuristics
pub struct TokenEstimator {
    /// Chars per token ratio (default ~4 for English)
    chars_per_token: f64,
}

impl Default for TokenEstimator {
    fn default() -> Self {
        Self::new()
    }
}

impl TokenEstimator {
    /// Create a new token estimator
    pub fn new() -> Self {
        Self {
            chars_per_token: 4.0,
        }
    }

    /// Count tokens in text using character heuristic
    ///
    /// The ~4 characters per token ratio is reasonably accurate for
    /// English text. Code tends to have more tokens per character.
    pub fn count(&self, text: &str) -> usize {
        if text.is_empty() {
            return 0;
        }

        (text.len() as f64 / self.chars_per_token).ceil() as usize
    }

    /// Count tokens in file content with type-specific adjustments
    ///
    /// Code files tend to have more tokens per character due to syntax.
    pub fn count_file(&self, content: &str, file_type: &str) -> usize {
        let base_count = self.count(content);

        // Code tends to have more tokens per character due to syntax
        let multiplier = match file_type {
            ".py" | ".js" | ".ts" | ".tsx" | ".jsx" | ".swift" | ".rs" | ".go" | ".java" | ".c"
            | ".cpp" | ".h" => 1.1,
            _ => 1.0,
        };

        (base_count as f64 * multiplier).ceil() as usize
    }

    /// Estimate remaining tokens available
    pub fn estimate_remaining(&self, used: usize, max_context: usize) -> usize {
        max_context.saturating_sub(used)
    }

    /// Format token count for display (e.g., "45.2K", "1.2M")
    pub fn format_count(count: usize) -> String {
        if count >= 1_000_000 {
            format!("{:.1}M", count as f64 / 1_000_000.0)
        } else if count >= 1_000 {
            format!("{:.1}K", count as f64 / 1_000.0)
        } else {
            count.to_string()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_empty_text() {
        let estimator = TokenEstimator::new();
        assert_eq!(estimator.count(""), 0);
    }

    #[test]
    fn test_short_text() {
        let estimator = TokenEstimator::new();
        // "hello" = 5 chars, ~1.25 tokens, rounds to 2
        assert_eq!(estimator.count("hello"), 2);
    }

    #[test]
    fn test_longer_text() {
        let estimator = TokenEstimator::new();
        // 100 chars -> ~25 tokens
        let text = "a".repeat(100);
        assert_eq!(estimator.count(&text), 25);
    }

    #[test]
    fn test_code_file_adjustment() {
        let estimator = TokenEstimator::new();
        let content = "fn main() { println!(\"hello\"); }";
        let base = estimator.count(content);
        let adjusted = estimator.count_file(content, ".rs");
        assert!(adjusted > base);
    }

    #[test]
    fn test_format_count() {
        assert_eq!(TokenEstimator::format_count(500), "500");
        assert_eq!(TokenEstimator::format_count(1500), "1.5K");
        assert_eq!(TokenEstimator::format_count(45200), "45.2K");
        assert_eq!(TokenEstimator::format_count(1_200_000), "1.2M");
    }
}
