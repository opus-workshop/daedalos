//! Orchestrator - Coordinates gap analysis, prompt generation, and loop execution

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::process::Command;

/// Gap analysis results
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GapAnalysis {
    /// Files missing tests entirely
    pub test_gaps: Vec<TestGap>,

    /// Files with tests but incomplete coverage
    pub coverage_gaps: Vec<CoverageGap>,

    /// General implementation gaps from evolve
    pub implementation_gaps: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestGap {
    pub file: String,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CoverageGap {
    pub file: String,
    pub description: String,
    pub lines: Option<Vec<u32>>,
}

/// Result of running the fill loop
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoopResult {
    pub success: bool,
    pub iterations: u32,
    pub error: Option<String>,
    pub loop_id: Option<String>,
}

/// Orchestrates the analyze workflow
pub struct AnalyzeOrchestrator {
    path: PathBuf,
    working_dir: PathBuf,
}

impl AnalyzeOrchestrator {
    pub fn new(path: &str) -> Result<Self> {
        let path = PathBuf::from(path);
        let working_dir = if path.is_absolute() {
            path.clone()
        } else {
            std::env::current_dir()?.join(&path)
        };

        Ok(Self { path, working_dir })
    }

    /// Get gaps by running evolve gaps and analyzing the project
    pub fn get_gaps(&self) -> Result<GapAnalysis> {
        let mut analysis = GapAnalysis {
            test_gaps: Vec::new(),
            coverage_gaps: Vec::new(),
            implementation_gaps: Vec::new(),
        };

        // Try running evolve gaps --json
        if let Ok(output) = Command::new("evolve")
            .arg("gaps")
            .arg(&self.path)
            .arg("--json")
            .current_dir(&self.working_dir)
            .output()
        {
            if output.status.success() {
                let stdout = String::from_utf8_lossy(&output.stdout);
                if let Ok(evolve_gaps) = serde_json::from_str::<serde_json::Value>(&stdout) {
                    // Extract missing_tests flag
                    if evolve_gaps.get("missing_tests").and_then(|v| v.as_bool()) == Some(true) {
                        analysis.test_gaps.push(TestGap {
                            file: self.path.display().to_string(),
                            reason: "No test files found by evolve".to_string(),
                        });
                    }

                    // Extract missing fundamentals related to testing
                    if let Some(fundamentals) = evolve_gaps.get("missing_fundamentals") {
                        if let Some(arr) = fundamentals.as_array() {
                            for item in arr {
                                if let Some(cat) = item.get("category").and_then(|c| c.as_str()) {
                                    if cat == "testing" {
                                        if let Some(desc) =
                                            item.get("description").and_then(|d| d.as_str())
                                        {
                                            analysis.implementation_gaps.push(desc.to_string());
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        // Scan for source files without corresponding test files
        self.find_untested_files(&mut analysis)?;

        Ok(analysis)
    }

    /// Find source files that don't have corresponding test files
    fn find_untested_files(&self, analysis: &mut GapAnalysis) -> Result<()> {
        let scan_path = if self.working_dir.is_dir() {
            &self.working_dir
        } else {
            self.working_dir
                .parent()
                .unwrap_or(&self.working_dir)
        };

        // Collect source files and test files
        let mut source_files: Vec<PathBuf> = Vec::new();
        let mut test_files: Vec<PathBuf> = Vec::new();

        self.walk_for_files(scan_path, &mut source_files, &mut test_files)?;

        // Find source files without tests
        for source in &source_files {
            let stem = source.file_stem().and_then(|s| s.to_str()).unwrap_or("");

            // Skip if it's already a test file
            if is_test_file(source) {
                continue;
            }

            // Check if there's a corresponding test file
            let has_test = test_files.iter().any(|test| {
                let test_stem = test.file_stem().and_then(|s| s.to_str()).unwrap_or("");
                test_stem.contains(stem) || test_stem.contains(&format!("{}_test", stem))
            });

            if !has_test {
                let relative = source.strip_prefix(scan_path).unwrap_or(source);
                analysis.test_gaps.push(TestGap {
                    file: relative.display().to_string(),
                    reason: "No corresponding test file found".to_string(),
                });
            }
        }

        Ok(())
    }

    fn walk_for_files(
        &self,
        path: &Path,
        sources: &mut Vec<PathBuf>,
        tests: &mut Vec<PathBuf>,
    ) -> Result<()> {
        if !path.is_dir() {
            return Ok(());
        }

        // Skip common directories
        let skip_dirs = [
            "node_modules",
            "target",
            "build",
            "dist",
            ".git",
            "__pycache__",
            "venv",
            ".venv",
        ];

        for entry in std::fs::read_dir(path)? {
            let entry = entry?;
            let path = entry.path();

            if path.is_dir() {
                let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
                if skip_dirs.contains(&name) {
                    continue;
                }
                self.walk_for_files(&path, sources, tests)?;
            } else if path.is_file() {
                if is_source_file(&path) {
                    if is_test_file(&path) {
                        tests.push(path);
                    } else {
                        sources.push(path);
                    }
                }
            }
        }

        Ok(())
    }

    /// Generate a prompt for the AI agent based on gaps
    pub fn generate_prompt(&self, gaps: &GapAnalysis) -> Result<String> {
        let mut prompt = String::new();

        prompt.push_str("Write tests for the following gaps in the codebase:\n\n");

        if !gaps.test_gaps.is_empty() {
            prompt.push_str("## Files Missing Tests\n\n");
            for gap in &gaps.test_gaps {
                prompt.push_str(&format!("- `{}`: {}\n", gap.file, gap.reason));
            }
            prompt.push('\n');
        }

        if !gaps.coverage_gaps.is_empty() {
            prompt.push_str("## Coverage Gaps\n\n");
            for gap in &gaps.coverage_gaps {
                prompt.push_str(&format!("- `{}`: {}\n", gap.file, gap.description));
            }
            prompt.push('\n');
        }

        if !gaps.implementation_gaps.is_empty() {
            prompt.push_str("## Other Testing Gaps\n\n");
            for gap in &gaps.implementation_gaps {
                prompt.push_str(&format!("- {}\n", gap));
            }
            prompt.push('\n');
        }

        prompt.push_str("## Requirements\n\n");
        prompt.push_str("1. Write focused unit tests for the identified gaps\n");
        prompt.push_str("2. Follow existing test patterns in the codebase\n");
        prompt.push_str("3. Tests should be meaningful, not just for coverage\n");
        prompt.push_str("4. Ensure all tests pass with the verification command\n");

        Ok(prompt)
    }

    /// Run the loop to fill gaps
    pub async fn run_loop(
        &self,
        prompt: &str,
        promise: &str,
        max_iterations: u32,
    ) -> Result<LoopResult> {
        // Run: loop start "<prompt>" --promise "<promise>" -n <max>
        let output = Command::new("loop")
            .arg("start")
            .arg(prompt)
            .arg("--promise")
            .arg(promise)
            .arg("-n")
            .arg(max_iterations.to_string())
            .arg("--json")
            .current_dir(&self.working_dir)
            .output()
            .context("Failed to run loop command - is loop installed?")?;

        let stdout = String::from_utf8_lossy(&output.stdout);

        // Try to parse the JSON output from loop
        if let Ok(result) = serde_json::from_str::<serde_json::Value>(&stdout) {
            let success = result.get("status").and_then(|s| s.as_str()) == Some("completed");
            let iterations = result
                .get("current_iteration")
                .and_then(|i| i.as_u64())
                .unwrap_or(0) as u32;
            let loop_id = result.get("id").and_then(|i| i.as_str()).map(|s| s.to_string());
            let error = result
                .get("error_message")
                .and_then(|e| e.as_str())
                .map(|s| s.to_string());

            return Ok(LoopResult {
                success,
                iterations,
                loop_id,
                error,
            });
        }

        // Fallback: check exit code
        Ok(LoopResult {
            success: output.status.success(),
            iterations: 0,
            loop_id: None,
            error: if output.status.success() {
                None
            } else {
                Some(String::from_utf8_lossy(&output.stderr).to_string())
            },
        })
    }
}

fn is_source_file(path: &Path) -> bool {
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
    matches!(
        ext,
        "rs" | "py" | "js" | "ts" | "jsx" | "tsx" | "go" | "rb" | "swift" | "kt" | "java"
    )
}

fn is_test_file(path: &Path) -> bool {
    let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
    let lower = name.to_lowercase();

    // Common test file patterns
    lower.contains("test")
        || lower.contains("spec")
        || lower.starts_with("test_")
        || lower.ends_with("_test.rs")
        || lower.ends_with("_test.py")
        || lower.ends_with(".test.js")
        || lower.ends_with(".test.ts")
        || lower.ends_with(".test.tsx")
        || lower.ends_with(".spec.js")
        || lower.ends_with(".spec.ts")
        || lower.ends_with("_test.go")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_test_file() {
        assert!(is_test_file(Path::new("foo_test.rs")));
        assert!(is_test_file(Path::new("foo.test.ts")));
        assert!(is_test_file(Path::new("test_foo.py")));
        assert!(is_test_file(Path::new("foo.spec.js")));
        assert!(!is_test_file(Path::new("foo.rs")));
        assert!(!is_test_file(Path::new("main.ts")));
    }

    #[test]
    fn test_is_source_file() {
        assert!(is_source_file(Path::new("foo.rs")));
        assert!(is_source_file(Path::new("foo.py")));
        assert!(is_source_file(Path::new("foo.ts")));
        assert!(!is_source_file(Path::new("foo.txt")));
        assert!(!is_source_file(Path::new("foo.md")));
    }
}
