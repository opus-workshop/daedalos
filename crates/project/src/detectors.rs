//! Project type and architecture detection

use std::collections::{HashMap, HashSet};
use std::path::Path;

use anyhow::Result;

use crate::database::ProjectDatabase;

/// Project type indicator files
const PROJECT_INDICATORS: &[(&str, &[&str])] = &[
    ("swift", &["*.xcodeproj", "*.xcworkspace", "Package.swift"]),
    ("typescript", &["tsconfig.json"]),
    ("nodejs", &["package.json"]),
    ("rust", &["Cargo.toml"]),
    ("go", &["go.mod"]),
    ("python", &["pyproject.toml", "setup.py", "requirements.txt"]),
    ("java-maven", &["pom.xml"]),
    ("java-gradle", &["build.gradle", "build.gradle.kts"]),
    ("ruby", &["Gemfile"]),
    ("php", &["composer.json"]),
    ("elixir", &["mix.exs"]),
    ("deno", &["deno.json", "deno.jsonc"]),
    ("dotnet", &["*.csproj", "*.fsproj", "*.sln"]),
    ("c-cmake", &["CMakeLists.txt"]),
    ("c-make", &["Makefile"]),
];

/// Extension to project type fallbacks
const EXTENSION_FALLBACKS: &[(&str, &str)] = &[
    (".py", "python"),
    (".js", "nodejs"),
    (".ts", "typescript"),
    (".swift", "swift"),
    (".rs", "rust"),
    (".go", "go"),
    (".rb", "ruby"),
    (".php", "php"),
    (".ex", "elixir"),
    (".exs", "elixir"),
    (".java", "java"),
    (".cs", "dotnet"),
    (".c", "c"),
    (".cpp", "cpp"),
    (".h", "c"),
];

/// Detect project type from indicator files
pub fn detect_project_type(path: &Path) -> String {
    // Check indicator files in priority order
    for (project_type, indicators) in PROJECT_INDICATORS {
        for indicator in *indicators {
            if indicator.contains('*') {
                // Glob pattern
                let pattern = path.join(indicator).to_string_lossy().to_string();
                if glob::glob(&pattern).ok().map_or(false, |mut g| g.next().is_some()) {
                    return project_type.to_string();
                }
            } else if path.join(indicator).exists() {
                return project_type.to_string();
            }
        }
    }

    // Fallback: detect from most common file extension
    detect_from_extensions(path)
}

fn detect_from_extensions(path: &Path) -> String {
    let mut extensions: HashMap<String, usize> = HashMap::new();

    if let Ok(entries) = std::fs::read_dir(path) {
        for entry in entries.flatten() {
            count_extensions_recursive(&entry.path(), &mut extensions, 0);
        }
    }

    if extensions.is_empty() {
        return "unknown".to_string();
    }

    let top_ext = extensions.iter().max_by_key(|(_, count)| *count).map(|(ext, _)| ext.clone());

    if let Some(ext) = top_ext {
        for (pattern, project_type) in EXTENSION_FALLBACKS {
            if ext == *pattern {
                return project_type.to_string();
            }
        }
    }

    "unknown".to_string()
}

fn count_extensions_recursive(path: &Path, extensions: &mut HashMap<String, usize>, depth: usize) {
    // Limit recursion depth
    if depth > 5 {
        return;
    }

    // Skip ignored directories
    if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
        let ignored = [
            "node_modules",
            ".git",
            "__pycache__",
            "build",
            "dist",
            "target",
            "venv",
            ".venv",
            "vendor",
            "Pods",
        ];
        if ignored.contains(&name) {
            return;
        }
    }

    if path.is_file() {
        if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
            let ext = format!(".{}", ext.to_lowercase());
            *extensions.entry(ext).or_default() += 1;
        }
    } else if path.is_dir() {
        if let Ok(entries) = std::fs::read_dir(path) {
            for entry in entries.flatten() {
                count_extensions_recursive(&entry.path(), extensions, depth + 1);
            }
        }
    }
}

/// Get human-readable description of project type
pub fn get_project_description(project_type: &str) -> &'static str {
    match project_type {
        "swift" => "Swift/Xcode Project",
        "typescript" => "TypeScript Project",
        "nodejs" => "Node.js Project",
        "rust" => "Rust/Cargo Project",
        "go" => "Go Project",
        "python" => "Python Project",
        "java-maven" => "Java (Maven) Project",
        "java-gradle" => "Java (Gradle) Project",
        "ruby" => "Ruby Project",
        "php" => "PHP Project",
        "elixir" => "Elixir Project",
        "deno" => "Deno Project",
        "dotnet" => ".NET Project",
        "c-cmake" => "C/C++ (CMake) Project",
        "c-make" => "C/C++ (Make) Project",
        "c" => "C Project",
        "cpp" => "C++ Project",
        "java" => "Java Project",
        _ => "Unknown Project Type",
    }
}

/// Architecture patterns
struct ArchPattern {
    dirs: &'static [&'static str],
    alt_dirs: &'static [&'static str],
    description: &'static str,
    weight: i32,
}

const ARCHITECTURE_PATTERNS: &[(&str, ArchPattern)] = &[
    (
        "mvc",
        ArchPattern {
            dirs: &["controllers", "models", "views"],
            alt_dirs: &[],
            description: "Model-View-Controller",
            weight: 3,
        },
    ),
    (
        "mvvm",
        ArchPattern {
            dirs: &["viewmodels", "views", "models"],
            alt_dirs: &["viewmodel", "view", "model"],
            description: "Model-View-ViewModel",
            weight: 3,
        },
    ),
    (
        "clean",
        ArchPattern {
            dirs: &["domain", "application", "infrastructure"],
            alt_dirs: &["core", "data", "presentation"],
            description: "Clean Architecture",
            weight: 3,
        },
    ),
    (
        "hexagonal",
        ArchPattern {
            dirs: &["adapters", "ports", "domain"],
            alt_dirs: &[],
            description: "Hexagonal Architecture",
            weight: 3,
        },
    ),
    (
        "feature-based",
        ArchPattern {
            dirs: &["features", "modules"],
            alt_dirs: &[],
            description: "Feature-based Organization",
            weight: 2,
        },
    ),
    (
        "component-based",
        ArchPattern {
            dirs: &["components", "pages"],
            alt_dirs: &["components", "screens"],
            description: "Component-based (React/Vue style)",
            weight: 2,
        },
    ),
    (
        "layered",
        ArchPattern {
            dirs: &["api", "services", "repositories"],
            alt_dirs: &["handlers", "service", "repository"],
            description: "Layered Architecture",
            weight: 2,
        },
    ),
    (
        "monolith",
        ArchPattern {
            dirs: &["src"],
            alt_dirs: &[],
            description: "Monolith (Single Source)",
            weight: 1,
        },
    ),
];

/// Detect architecture pattern from directory structure
pub fn detect_architecture(db: &ProjectDatabase) -> Result<serde_json::Value> {
    let dirs = collect_directories(db)?;

    let mut scores: HashMap<&str, f32> = HashMap::new();

    for (pattern_name, config) in ARCHITECTURE_PATTERNS {
        let score = score_pattern(&dirs, config);
        if score > 0.0 {
            scores.insert(pattern_name, score * config.weight as f32);
        }
    }

    if scores.is_empty() {
        return Ok(serde_json::json!({
            "type": "unknown",
            "description": "No recognized pattern",
            "confidence": "low"
        }));
    }

    let (best_match, best_score) = scores
        .iter()
        .max_by(|a, b| a.1.partial_cmp(b.1).unwrap_or(std::cmp::Ordering::Equal))
        .map(|(k, v)| (*k, *v))
        .unwrap();

    let description = ARCHITECTURE_PATTERNS
        .iter()
        .find(|(name, _)| *name == best_match)
        .map(|(_, config)| config.description)
        .unwrap_or("Unknown");

    let confidence = if best_score >= 3.0 {
        "high"
    } else if best_score >= 2.0 {
        "medium"
    } else {
        "low"
    };

    Ok(serde_json::json!({
        "type": best_match,
        "description": description,
        "confidence": confidence,
        "score": best_score
    }))
}

fn collect_directories(db: &ProjectDatabase) -> Result<HashSet<String>> {
    Ok(db.get_directories()?.into_iter().collect())
}

fn score_pattern(dirs: &HashSet<String>, config: &ArchPattern) -> f32 {
    let mut score = 0.0;

    // Check primary dirs
    for d in config.dirs {
        if dirs.contains(&d.to_lowercase()) {
            score += 1.0;
        }
    }

    // Check alternate dirs
    for d in config.alt_dirs {
        if dirs.contains(&d.to_lowercase()) {
            score += 0.5;
        }
    }

    score
}
