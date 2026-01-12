//! Finding specs and project roots

use anyhow::{Context, Result};
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

/// Find the project root by looking for .daedalos or .git
pub fn find_project_root() -> Result<PathBuf> {
    let cwd = std::env::current_dir().context("Failed to get current directory")?;
    let mut dir = cwd.as_path();

    loop {
        if dir.join(".daedalos").exists() || dir.join(".git").exists() {
            return Ok(dir.to_path_buf());
        }

        match dir.parent() {
            Some(parent) => dir = parent,
            None => break,
        }
    }

    // Fallback to current directory
    Ok(cwd)
}

/// Find a spec file for a component
pub fn find_spec(project_root: &Path, component: &str) -> Result<Option<PathBuf>> {
    // First, check index if it exists
    let index_file = project_root.join(".daedalos/specs/index.yaml");
    if index_file.exists() {
        if let Ok(content) = std::fs::read_to_string(&index_file) {
            if let Ok(index) = serde_yaml::from_str::<serde_yaml::Value>(&content) {
                if let Some(components) = index.get("components") {
                    if let Some(path) = components.get(component) {
                        if let Some(path_str) = path.as_str() {
                            let full_path = project_root.join(path_str);
                            if full_path.exists() {
                                return Ok(Some(full_path));
                            }
                        }
                    }
                }
            }
        }
    }

    // Search common locations
    let search_paths = [
        project_root.join(format!("daedalos-tools/{}/{}.spec.yaml", component, component)),
        project_root.join(format!("tools/{}/{}.spec.yaml", component, component)),
        project_root.join(format!("{}/{}.spec.yaml", component, component)),
        project_root.join(format!("{}.spec.yaml", component)),
    ];

    for path in &search_paths {
        if path.exists() {
            return Ok(Some(path.clone()));
        }
    }

    // Glob search as fallback
    let spec_name = format!("{}.spec.yaml", component);
    for entry in WalkDir::new(project_root)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        if entry.file_name().to_string_lossy() == spec_name {
            return Ok(Some(entry.path().to_path_buf()));
        }
    }

    Ok(None)
}

/// Find all spec files in the project
pub fn find_all_specs(project_root: &Path) -> Result<Vec<PathBuf>> {
    let mut specs = Vec::new();

    for entry in WalkDir::new(project_root)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        let path = entry.path();
        if path.extension().map(|e| e == "yaml").unwrap_or(false) {
            if let Some(name) = path.file_name() {
                if name.to_string_lossy().ends_with(".spec.yaml") {
                    specs.push(path.to_path_buf());
                }
            }
        }
    }

    specs.sort();
    Ok(specs)
}
