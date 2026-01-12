//! Intent analysis for the resolve tool
//!
//! Analyzes questions to determine if they're about intent (WHAT)
//! or implementation (HOW), and how clear the intent is.

use anyhow::Result;
use regex::Regex;
use serde::{Deserialize, Serialize};

/// Type of question being asked
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum QuestionType {
    /// Implementation decision - should be resolved, not asked
    Implementation,
    /// Intent/goal question - may warrant asking
    Intent,
    /// Task/feature request - may need details
    Task,
    /// General question - analyze further
    General,
}

impl QuestionType {
    pub fn as_str(&self) -> &'static str {
        match self {
            QuestionType::Implementation => "implementation",
            QuestionType::Intent => "intent",
            QuestionType::Task => "task",
            QuestionType::General => "general",
        }
    }
}

/// How clear the intent is
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum IntentClarity {
    /// Intent is clear
    Clear,
    /// Intent is likely clear (implementation question)
    LikelyClear,
    /// May need some details
    NeedsDetail,
    /// May need clarification
    Unclear,
}

impl IntentClarity {
    pub fn as_str(&self) -> &'static str {
        match self {
            IntentClarity::Clear => "clear",
            IntentClarity::LikelyClear => "likely_clear",
            IntentClarity::NeedsDetail => "needs_detail",
            IntentClarity::Unclear => "unclear",
        }
    }
}

/// Result of intent analysis
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IntentAnalysis {
    /// The surface question (what was literally asked)
    pub surface: String,
    /// The root problem (if different from surface)
    pub root: Option<String>,
    /// Type of question
    pub question_type: QuestionType,
    /// How clear the intent is
    pub clarity: IntentClarity,
    /// Hint about what to do next
    pub hint: Option<String>,
}

/// Analyze the intent of a question
pub fn analyze_intent(question: &str) -> Result<IntentAnalysis> {
    let question_lower = question.to_lowercase();

    // Implementation questions: should/how/which + approach/method/way/implement/handle/use
    let implementation_re =
        Regex::new(r"^(should|how|which|what).*(approach|method|way|implement|handle|use)")?;

    // Goal/outcome questions: what/why + goal/want/need/trying/purpose/outcome
    let intent_re = Regex::new(r"^(what|why).*(goal|want|need|trying|purpose|outcome)")?;

    // Task/feature questions: build/create/add/implement/make
    let task_re = Regex::new(r"(build|create|add|implement|make)")?;

    let (question_type, clarity, hint) = if implementation_re.is_match(&question_lower) {
        (
            QuestionType::Implementation,
            IntentClarity::LikelyClear,
            Some("This question can likely be resolved through context gathering. If context is insufficient, the root problem may need clarification.".to_string()),
        )
    } else if intent_re.is_match(&question_lower) {
        (
            QuestionType::Intent,
            IntentClarity::Unclear,
            Some("Consider asking a targeted question about the desired outcome.".to_string()),
        )
    } else if task_re.is_match(&question_lower) {
        (
            QuestionType::Task,
            IntentClarity::NeedsDetail,
            Some("Consider: audience, priority, scope before implementation.".to_string()),
        )
    } else {
        (
            QuestionType::General,
            IntentClarity::Unclear,
            Some("Analyze further to determine if this is a WHAT or HOW question.".to_string()),
        )
    };

    Ok(IntentAnalysis {
        surface: question.to_string(),
        root: None, // Would need more sophisticated analysis to determine root
        question_type,
        clarity,
        hint,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_implementation_question() {
        let result = analyze_intent("should we use approach A or approach B?").unwrap();
        assert_eq!(result.question_type, QuestionType::Implementation);
        assert_eq!(result.clarity, IntentClarity::LikelyClear);
    }

    #[test]
    fn test_how_question() {
        let result = analyze_intent("how should we handle authentication?").unwrap();
        assert_eq!(result.question_type, QuestionType::Implementation);
    }

    #[test]
    fn test_task_question() {
        let result = analyze_intent("build a scheduling feature").unwrap();
        assert_eq!(result.question_type, QuestionType::Task);
        assert_eq!(result.clarity, IntentClarity::NeedsDetail);
    }

    #[test]
    fn test_intent_question() {
        let result = analyze_intent("what are you trying to achieve?").unwrap();
        assert_eq!(result.question_type, QuestionType::Intent);
        assert_eq!(result.clarity, IntentClarity::Unclear);
    }

    #[test]
    fn test_general_question() {
        let result = analyze_intent("timezone settings").unwrap();
        assert_eq!(result.question_type, QuestionType::General);
    }
}
