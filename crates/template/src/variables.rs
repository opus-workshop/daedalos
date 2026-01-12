//! Template variable handling
//!
//! Provides variable substitution using {{PLACEHOLDER}} syntax.
//! Variables are replaced in both file content and file/directory names.

use chrono::Local;
use regex::Regex;
use std::collections::HashMap;
use std::process::Command;

/// Template variables container
#[derive(Debug, Clone)]
pub struct Variables {
    /// Variable name -> value mapping
    vars: HashMap<String, String>,
}

impl Variables {
    /// Create a new Variables container with standard variables populated
    pub fn new(project_name: &str) -> Self {
        let mut vars = HashMap::new();

        // Standard variables
        vars.insert("NAME".to_string(), project_name.to_string());
        vars.insert("AUTHOR".to_string(), Self::get_author());
        vars.insert("EMAIL".to_string(), Self::get_email());
        vars.insert("DATE".to_string(), Local::now().format("%Y-%m-%d").to_string());
        vars.insert("YEAR".to_string(), Local::now().format("%Y").to_string());
        vars.insert("DESCRIPTION".to_string(), "A new project".to_string());

        Self { vars }
    }

    /// Get the author name from git config or environment
    fn get_author() -> String {
        // Try git config first
        if let Ok(output) = Command::new("git")
            .args(["config", "user.name"])
            .output()
        {
            if output.status.success() {
                let name = String::from_utf8_lossy(&output.stdout).trim().to_string();
                if !name.is_empty() {
                    return name;
                }
            }
        }

        // Fallback to USER environment variable
        std::env::var("USER").unwrap_or_else(|_| "unknown".to_string())
    }

    /// Get the email from git config
    fn get_email() -> String {
        if let Ok(output) = Command::new("git")
            .args(["config", "user.email"])
            .output()
        {
            if output.status.success() {
                let email = String::from_utf8_lossy(&output.stdout).trim().to_string();
                if !email.is_empty() {
                    return email;
                }
            }
        }

        // Fallback
        let user = std::env::var("USER").unwrap_or_else(|_| "user".to_string());
        format!("{}@localhost", user)
    }

    /// Set a variable value
    pub fn set(&mut self, key: &str, value: &str) {
        self.vars.insert(key.to_uppercase(), value.to_string());
    }

    /// Get a variable value
    pub fn get(&self, key: &str) -> Option<&String> {
        self.vars.get(&key.to_uppercase())
    }

    /// Set the description variable
    pub fn set_description(&mut self, desc: &str) {
        self.set("DESCRIPTION", desc);
    }

    /// Parse KEY=VALUE strings and add them as variables
    pub fn add_from_pairs(&mut self, pairs: &[String]) {
        for pair in pairs {
            if let Some((key, value)) = pair.split_once('=') {
                self.set(key.trim(), value.trim());
            }
        }
    }

    /// Replace all {{PLACEHOLDER}} patterns in a string
    pub fn substitute(&self, content: &str) -> String {
        let re = Regex::new(r"\{\{([A-Z_][A-Z0-9_]*)\}\}").unwrap();

        re.replace_all(content, |caps: &regex::Captures| {
            let key = &caps[1];
            self.vars.get(key).cloned().unwrap_or_else(|| format!("{{{{{}}}}}", key))
        }).to_string()
    }

    /// Find all variables used in a string
    pub fn find_used_variables(content: &str) -> Vec<String> {
        let re = Regex::new(r"\{\{([A-Z_][A-Z0-9_]*)\}\}").unwrap();

        let mut vars: Vec<String> = re
            .captures_iter(content)
            .map(|cap| cap[1].to_string())
            .collect();

        vars.sort();
        vars.dedup();
        vars
    }

    /// Get all defined variable names
    pub fn names(&self) -> Vec<&String> {
        let mut names: Vec<_> = self.vars.keys().collect();
        names.sort();
        names
    }

    /// Get all variable entries
    pub fn entries(&self) -> impl Iterator<Item = (&String, &String)> {
        self.vars.iter()
    }
}

/// Check if a file is likely binary (should not have variable substitution)
pub fn is_binary_file(content: &[u8]) -> bool {
    // Check for null bytes in first 8KB
    let check_len = content.len().min(8192);
    content[..check_len].contains(&0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_substitute_basic() {
        let mut vars = Variables::new("myproject");
        vars.set("CUSTOM", "custom_value");

        let input = "Project: {{NAME}}, Custom: {{CUSTOM}}";
        let output = vars.substitute(input);

        assert_eq!(output, "Project: myproject, Custom: custom_value");
    }

    #[test]
    fn test_substitute_missing_var() {
        let vars = Variables::new("test");

        let input = "Value: {{UNKNOWN}}";
        let output = vars.substitute(input);

        // Unknown variables should be left as-is
        assert_eq!(output, "Value: {{UNKNOWN}}");
    }

    #[test]
    fn test_find_used_variables() {
        let content = "Name: {{NAME}}, Author: {{AUTHOR}}, Name again: {{NAME}}";
        let vars = Variables::find_used_variables(content);

        assert_eq!(vars, vec!["AUTHOR".to_string(), "NAME".to_string()]);
    }

    #[test]
    fn test_add_from_pairs() {
        let mut vars = Variables::new("test");
        vars.add_from_pairs(&[
            "FOO=bar".to_string(),
            "BAZ=qux".to_string(),
        ]);

        assert_eq!(vars.get("FOO"), Some(&"bar".to_string()));
        assert_eq!(vars.get("BAZ"), Some(&"qux".to_string()));
    }

    #[test]
    fn test_is_binary() {
        assert!(!is_binary_file(b"Hello, world!"));
        assert!(is_binary_file(b"Hello\x00world"));
    }
}
