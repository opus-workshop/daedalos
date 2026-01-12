//! Spec validation

use anyhow::Result;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

use crate::spec::Spec;

/// Required sections for a valid spec
const REQUIRED_SECTIONS: &[&str] = &["name", "intent", "constraints", "interface"];

/// Recommended sections (warning if missing)
const RECOMMENDED_SECTIONS: &[&str] = &["examples", "decisions", "anti_patterns"];

/// Minimum intent length
const MIN_INTENT_LENGTH: usize = 50;

/// Validation result for a spec
#[derive(Debug)]
pub enum ValidationResult {
    Ok {
        path: PathBuf,
    },
    Warning {
        path: PathBuf,
        messages: Vec<String>,
    },
    Error {
        path: PathBuf,
        messages: Vec<String>,
    },
}

/// Validate all specs in a path
pub fn validate_all_specs(path: &Path) -> Result<Vec<ValidationResult>> {
    let mut results = Vec::new();

    for entry in WalkDir::new(path)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        let entry_path = entry.path();
        if let Some(name) = entry_path.file_name() {
            if name.to_string_lossy().ends_with(".spec.yaml") {
                results.push(validate_single_spec(entry_path));
            }
        }
    }

    Ok(results)
}

/// Validate a single spec file
pub fn validate_single_spec(path: &Path) -> ValidationResult {
    let mut errors = Vec::new();
    let mut warnings = Vec::new();

    // Try to parse the YAML
    let content = match std::fs::read_to_string(path) {
        Ok(c) => c,
        Err(e) => {
            return ValidationResult::Error {
                path: path.to_path_buf(),
                messages: vec![format!("Failed to read file: {}", e)],
            };
        }
    };

    let spec: Spec = match serde_yaml::from_str(&content) {
        Ok(s) => s,
        Err(e) => {
            return ValidationResult::Error {
                path: path.to_path_buf(),
                messages: vec![format!("Invalid YAML syntax: {}", e)],
            };
        }
    };

    // Check required sections
    for section in REQUIRED_SECTIONS {
        if !spec.has_section(section) {
            errors.push(format!("Missing required section: {}", section));
        }
    }

    // Check recommended sections
    for section in RECOMMENDED_SECTIONS {
        if !spec.has_section(section) {
            warnings.push(format!("Missing recommended section: {}", section));
        }
    }

    // Check intent quality
    if let Some(ref intent) = spec.intent {
        if intent.len() < MIN_INTENT_LENGTH {
            warnings.push("Intent seems too short (should explain WHY)".to_string());
        }
    }

    // Return appropriate result
    if !errors.is_empty() {
        ValidationResult::Error {
            path: path.to_path_buf(),
            messages: errors,
        }
    } else if !warnings.is_empty() {
        ValidationResult::Warning {
            path: path.to_path_buf(),
            messages: warnings,
        }
    } else {
        ValidationResult::Ok {
            path: path.to_path_buf(),
        }
    }
}
