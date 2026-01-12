//! Project type detection

use std::path::Path;

/// Supported project types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProjectType {
    Rust,
    Python,
    TypeScript,
    JavaScript,
    Go,
    Swift,
    Shell,
    Unknown,
}

impl ProjectType {
    /// Get the name of the project type
    pub fn name(&self) -> &'static str {
        match self {
            ProjectType::Rust => "rust",
            ProjectType::Python => "python",
            ProjectType::TypeScript => "typescript",
            ProjectType::JavaScript => "javascript",
            ProjectType::Go => "go",
            ProjectType::Swift => "swift",
            ProjectType::Shell => "shell",
            ProjectType::Unknown => "unknown",
        }
    }

    /// Create from name string
    pub fn from_name(name: &str) -> Self {
        match name.to_lowercase().as_str() {
            "rust" => ProjectType::Rust,
            "python" => ProjectType::Python,
            "typescript" => ProjectType::TypeScript,
            "javascript" | "js" => ProjectType::JavaScript,
            "go" | "golang" => ProjectType::Go,
            "swift" => ProjectType::Swift,
            "shell" | "bash" | "sh" => ProjectType::Shell,
            _ => ProjectType::Unknown,
        }
    }
}

/// Detect project type from directory contents
pub fn detect_project_type(path: &Path) -> ProjectType {
    // Check for project indicator files (in priority order)

    // Swift/Xcode
    if has_xcode_project(path) || path.join("Package.swift").exists() {
        return ProjectType::Swift;
    }

    // Rust
    if path.join("Cargo.toml").exists() {
        return ProjectType::Rust;
    }

    // Go
    if path.join("go.mod").exists() {
        return ProjectType::Go;
    }

    // TypeScript/JavaScript
    if path.join("package.json").exists() {
        if path.join("tsconfig.json").exists() {
            return ProjectType::TypeScript;
        }
        return ProjectType::JavaScript;
    }

    // Python
    if path.join("pyproject.toml").exists()
        || path.join("setup.py").exists()
        || path.join("requirements.txt").exists()
    {
        return ProjectType::Python;
    }

    // Shell scripts
    if has_shell_scripts(path) {
        return ProjectType::Shell;
    }

    ProjectType::Unknown
}

/// Check if directory has Xcode project files
fn has_xcode_project(path: &Path) -> bool {
    if let Ok(entries) = std::fs::read_dir(path) {
        for entry in entries.flatten() {
            let name = entry.file_name();
            let name_str = name.to_string_lossy();
            if name_str.ends_with(".xcodeproj") || name_str.ends_with(".xcworkspace") {
                return true;
            }
        }
    }
    false
}

/// Check if directory has shell scripts
fn has_shell_scripts(path: &Path) -> bool {
    if let Ok(entries) = std::fs::read_dir(path) {
        for entry in entries.flatten() {
            let name = entry.file_name();
            let name_str = name.to_string_lossy();
            if name_str.ends_with(".sh") {
                return true;
            }
        }
    }
    false
}

/// Detect Node.js package manager
pub fn detect_package_manager(path: &Path) -> &'static str {
    if path.join("pnpm-lock.yaml").exists() {
        "pnpm"
    } else if path.join("yarn.lock").exists() {
        "yarn"
    } else if path.join("bun.lockb").exists() {
        "bun"
    } else {
        "npm"
    }
}

/// Detect Xcode scheme
pub fn detect_xcode_scheme(path: &Path) -> Option<String> {
    // Find xcodeproj or xcworkspace
    let mut project_path = None;

    if let Ok(entries) = std::fs::read_dir(path) {
        for entry in entries.flatten() {
            let name = entry.file_name();
            let name_str = name.to_string_lossy();
            if name_str.ends_with(".xcworkspace") {
                project_path = Some(entry.path());
                break;
            } else if name_str.ends_with(".xcodeproj") && project_path.is_none() {
                project_path = Some(entry.path());
            }
        }
    }

    let project = project_path?;

    // Try to get schemes using xcodebuild
    let flag = if project.extension().map_or(false, |e| e == "xcworkspace") {
        "-workspace"
    } else {
        "-project"
    };

    let output = std::process::Command::new("xcodebuild")
        .arg(flag)
        .arg(&project)
        .arg("-list")
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    let stdout = String::from_utf8_lossy(&output.stdout);

    // Parse schemes from output
    let mut in_schemes = false;
    for line in stdout.lines() {
        if line.contains("Schemes:") {
            in_schemes = true;
            continue;
        }
        if in_schemes {
            let trimmed = line.trim();
            if trimmed.is_empty() || !line.starts_with(char::is_whitespace) {
                break;
            }
            return Some(trimmed.to_string());
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn test_detect_rust() {
        let dir = tempdir().unwrap();
        fs::write(dir.path().join("Cargo.toml"), "[package]").unwrap();
        assert_eq!(detect_project_type(dir.path()), ProjectType::Rust);
    }

    #[test]
    fn test_detect_python() {
        let dir = tempdir().unwrap();
        fs::write(dir.path().join("pyproject.toml"), "[tool]").unwrap();
        assert_eq!(detect_project_type(dir.path()), ProjectType::Python);
    }

    #[test]
    fn test_detect_typescript() {
        let dir = tempdir().unwrap();
        fs::write(dir.path().join("package.json"), "{}").unwrap();
        fs::write(dir.path().join("tsconfig.json"), "{}").unwrap();
        assert_eq!(detect_project_type(dir.path()), ProjectType::TypeScript);
    }

    #[test]
    fn test_detect_javascript() {
        let dir = tempdir().unwrap();
        fs::write(dir.path().join("package.json"), "{}").unwrap();
        assert_eq!(detect_project_type(dir.path()), ProjectType::JavaScript);
    }

    #[test]
    fn test_detect_go() {
        let dir = tempdir().unwrap();
        fs::write(dir.path().join("go.mod"), "module test").unwrap();
        assert_eq!(detect_project_type(dir.path()), ProjectType::Go);
    }

    #[test]
    fn test_project_type_from_name() {
        assert_eq!(ProjectType::from_name("rust"), ProjectType::Rust);
        assert_eq!(ProjectType::from_name("PYTHON"), ProjectType::Python);
        assert_eq!(ProjectType::from_name("golang"), ProjectType::Go);
        assert_eq!(ProjectType::from_name("unknown_type"), ProjectType::Unknown);
    }
}
