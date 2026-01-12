//! Template storage and management
//!
//! Handles discovering, listing, and instantiating templates.
//! Templates are directories with files that can contain {{PLACEHOLDER}} variables.

use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use walkdir::WalkDir;

use crate::variables::{is_binary_file, Variables};

/// Template metadata from template.json
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TemplateMetadata {
    /// Template name
    #[serde(default)]
    pub name: String,

    /// Template description
    #[serde(default)]
    pub description: String,

    /// Template version
    #[serde(default)]
    pub version: String,

    /// Template author
    #[serde(default)]
    pub author: String,

    /// Variables used by this template
    #[serde(default)]
    pub variables: Vec<String>,

    /// Next steps to show after creation
    #[serde(default)]
    pub next_steps: Vec<String>,
}

/// A template definition
#[derive(Debug, Clone)]
pub struct Template {
    /// Template name
    pub name: String,
    /// Path to template directory
    pub path: PathBuf,
    /// Whether this is a built-in template
    pub builtin: bool,
    /// Template metadata (if template.json exists)
    pub metadata: Option<TemplateMetadata>,
}

impl Template {
    /// Load a template from a directory
    pub fn from_path(path: &Path, builtin: bool) -> Result<Self> {
        let name = path
            .file_name()
            .and_then(|n| n.to_str())
            .ok_or_else(|| anyhow::anyhow!("Invalid template path"))?
            .to_string();

        let metadata_path = path.join("template.json");
        let metadata = if metadata_path.exists() {
            let content = fs::read_to_string(&metadata_path)
                .with_context(|| format!("Failed to read template.json: {}", metadata_path.display()))?;
            Some(serde_json::from_str(&content).context("Failed to parse template.json")?)
        } else {
            None
        };

        Ok(Self {
            name,
            path: path.to_path_buf(),
            builtin,
            metadata,
        })
    }

    /// Get the template description
    pub fn description(&self) -> &str {
        self.metadata
            .as_ref()
            .map(|m| m.description.as_str())
            .filter(|d| !d.is_empty())
            .unwrap_or("No description")
    }

    /// Get all variables used in this template
    pub fn find_variables(&self) -> Result<Vec<String>> {
        let mut all_vars = Vec::new();

        for entry in WalkDir::new(&self.path)
            .into_iter()
            .filter_map(|e| e.ok())
        {
            let path = entry.path();

            // Check filename for variables
            if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                all_vars.extend(Variables::find_used_variables(name));
            }

            // Check file content for variables
            if path.is_file() && path.file_name().map(|n| n != "template.json").unwrap_or(false) {
                if let Ok(content) = fs::read(path) {
                    if !is_binary_file(&content) {
                        if let Ok(text) = String::from_utf8(content) {
                            all_vars.extend(Variables::find_used_variables(&text));
                        }
                    }
                }
            }
        }

        all_vars.sort();
        all_vars.dedup();
        Ok(all_vars)
    }

    /// Create a new project from this template
    pub fn instantiate(
        &self,
        dest: &Path,
        vars: &Variables,
        init_git: bool,
    ) -> Result<()> {
        if dest.exists() {
            bail!("Destination already exists: {}", dest.display());
        }

        fs::create_dir_all(dest)
            .with_context(|| format!("Failed to create destination: {}", dest.display()))?;

        // Walk the template and copy files
        for entry in WalkDir::new(&self.path)
            .into_iter()
            .filter_map(|e| e.ok())
        {
            let src_path = entry.path();

            // Get relative path from template root
            let rel_path = src_path.strip_prefix(&self.path)?;

            // Skip template.json
            if rel_path.as_os_str() == "template.json" {
                continue;
            }

            // Skip empty relative path (the root directory)
            if rel_path.as_os_str().is_empty() {
                continue;
            }

            // Substitute variables in path components
            let mut dest_rel = PathBuf::new();
            for component in rel_path.components() {
                let component_str = component.as_os_str().to_string_lossy();
                let substituted = vars.substitute(&component_str);
                dest_rel.push(substituted);
            }

            let dest_path = dest.join(&dest_rel);

            if src_path.is_dir() {
                fs::create_dir_all(&dest_path)
                    .with_context(|| format!("Failed to create directory: {}", dest_path.display()))?;
            } else {
                // Ensure parent directory exists
                if let Some(parent) = dest_path.parent() {
                    fs::create_dir_all(parent)?;
                }

                // Read source file
                let content = fs::read(src_path)
                    .with_context(|| format!("Failed to read: {}", src_path.display()))?;

                if is_binary_file(&content) {
                    // Binary file - copy as-is
                    fs::write(&dest_path, &content)
                        .with_context(|| format!("Failed to write: {}", dest_path.display()))?;
                } else {
                    // Text file - substitute variables
                    let text = String::from_utf8_lossy(&content);
                    let substituted = vars.substitute(&text);
                    fs::write(&dest_path, substituted)
                        .with_context(|| format!("Failed to write: {}", dest_path.display()))?;
                }

                // Preserve executable permission
                #[cfg(unix)]
                {
                    use std::os::unix::fs::PermissionsExt;
                    if let Ok(src_meta) = src_path.metadata() {
                        let src_mode = src_meta.permissions().mode();
                        if src_mode & 0o111 != 0 {
                            // Source is executable
                            let mut perms = fs::metadata(&dest_path)?.permissions();
                            perms.set_mode(src_mode);
                            fs::set_permissions(&dest_path, perms)?;
                        }
                    }
                }
            }
        }

        // Initialize git if requested
        if init_git {
            let _ = Command::new("git")
                .args(["init", "-q"])
                .current_dir(dest)
                .output();

            let _ = Command::new("git")
                .args(["add", "."])
                .current_dir(dest)
                .output();
        }

        Ok(())
    }
}

/// Template store - manages template discovery and creation
pub struct TemplateStore {
    /// Built-in templates directory
    builtin_dir: PathBuf,
    /// User templates directory
    user_dir: PathBuf,
}

impl TemplateStore {
    /// Create a new template store
    pub fn new() -> Result<Self> {
        let data_dir = dirs::data_dir()
            .unwrap_or_else(|| PathBuf::from("~/.local/share"))
            .join("daedalos/template");

        let user_dir = data_dir.join("templates");
        fs::create_dir_all(&user_dir)?;

        // Built-in templates: look relative to executable or use env var
        let builtin_dir = std::env::var("DAEDALOS_TEMPLATE_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|_| {
                // Try to find built-in templates relative to the executable
                if let Ok(exe_path) = std::env::current_exe() {
                    if let Some(parent) = exe_path.parent() {
                        let templates_path = parent.join("../share/daedalos/templates");
                        if templates_path.exists() {
                            return templates_path;
                        }
                    }
                }
                // Fallback to common location
                PathBuf::from("/usr/share/daedalos/templates")
            });

        Ok(Self {
            builtin_dir,
            user_dir,
        })
    }

    /// Create with custom directories (for testing)
    pub fn with_dirs(builtin_dir: PathBuf, user_dir: PathBuf) -> Result<Self> {
        fs::create_dir_all(&user_dir)?;
        Ok(Self {
            builtin_dir,
            user_dir,
        })
    }

    /// List all available templates
    pub fn list(&self) -> Result<Vec<Template>> {
        let mut templates = Vec::new();

        // Load built-in templates
        if self.builtin_dir.exists() {
            for entry in fs::read_dir(&self.builtin_dir)? {
                let entry = entry?;
                let path = entry.path();
                if path.is_dir() {
                    if let Ok(template) = Template::from_path(&path, true) {
                        templates.push(template);
                    }
                }
            }
        }

        // Load user templates (can override built-in)
        if self.user_dir.exists() {
            for entry in fs::read_dir(&self.user_dir)? {
                let entry = entry?;
                let path = entry.path();
                if path.is_dir() {
                    if let Ok(template) = Template::from_path(&path, false) {
                        // Remove built-in template with same name
                        templates.retain(|t| t.name != template.name);
                        templates.push(template);
                    }
                }
            }
        }

        // Sort by name
        templates.sort_by(|a, b| a.name.cmp(&b.name));

        Ok(templates)
    }

    /// Find a template by name
    pub fn find(&self, name: &str) -> Result<Option<Template>> {
        // User templates take precedence
        let user_path = self.user_dir.join(name);
        if user_path.is_dir() {
            return Ok(Some(Template::from_path(&user_path, false)?));
        }

        // Check built-in templates
        let builtin_path = self.builtin_dir.join(name);
        if builtin_path.is_dir() {
            return Ok(Some(Template::from_path(&builtin_path, true)?));
        }

        Ok(None)
    }

    /// Add a directory as a user template
    pub fn add(&self, source: &Path, name: Option<&str>) -> Result<Template> {
        if !source.is_dir() {
            bail!("Source is not a directory: {}", source.display());
        }

        let template_name = name
            .map(String::from)
            .unwrap_or_else(|| {
                source
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("template")
                    .to_string()
            });

        let dest = self.user_dir.join(&template_name);
        if dest.exists() {
            bail!("Template already exists: {}", template_name);
        }

        // Copy the directory
        copy_dir_recursive(source, &dest)?;

        // Create template.json if missing
        let metadata_path = dest.join("template.json");
        if !metadata_path.exists() {
            let metadata = TemplateMetadata {
                name: template_name.clone(),
                description: "User template".to_string(),
                version: "1.0.0".to_string(),
                author: Variables::new("").get("AUTHOR").cloned().unwrap_or_default(),
                ..Default::default()
            };

            let json = serde_json::to_string_pretty(&metadata)?;
            fs::write(&metadata_path, json)?;
        }

        Template::from_path(&dest, false)
    }

    /// Remove a user template
    pub fn remove(&self, name: &str) -> Result<()> {
        let path = self.user_dir.join(name);
        if !path.exists() {
            bail!("User template not found: {}", name);
        }

        fs::remove_dir_all(&path)
            .with_context(|| format!("Failed to remove template: {}", name))?;

        Ok(())
    }

    /// Initialize a template.json in the current directory
    pub fn init(name: Option<&str>) -> Result<PathBuf> {
        let cwd = std::env::current_dir()?;
        let template_name = name
            .map(String::from)
            .unwrap_or_else(|| {
                cwd.file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("template")
                    .to_string()
            });

        let metadata_path = cwd.join("template.json");
        if metadata_path.exists() {
            bail!("template.json already exists");
        }

        let metadata = TemplateMetadata {
            name: template_name,
            description: "{{DESCRIPTION}}".to_string(),
            version: "1.0.0".to_string(),
            author: Variables::new("").get("AUTHOR").cloned().unwrap_or_default(),
            variables: vec!["NAME".to_string(), "DESCRIPTION".to_string()],
            next_steps: vec![
                "Review the generated files".to_string(),
                "Run the setup script".to_string(),
            ],
        };

        let json = serde_json::to_string_pretty(&metadata)?;
        fs::write(&metadata_path, json)?;

        Ok(metadata_path)
    }
}

/// Recursively copy a directory
fn copy_dir_recursive(src: &Path, dst: &Path) -> Result<()> {
    fs::create_dir_all(dst)?;

    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let src_path = entry.path();
        let dst_path = dst.join(entry.file_name());

        if src_path.is_dir() {
            copy_dir_recursive(&src_path, &dst_path)?;
        } else {
            fs::copy(&src_path, &dst_path)?;
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;

    fn create_test_template(dir: &Path) -> Result<()> {
        fs::create_dir_all(dir)?;

        // Create template.json
        let metadata = TemplateMetadata {
            name: "test-template".to_string(),
            description: "A test template".to_string(),
            version: "1.0.0".to_string(),
            author: "Test".to_string(),
            variables: vec!["NAME".to_string()],
            next_steps: vec!["Do something".to_string()],
        };
        fs::write(
            dir.join("template.json"),
            serde_json::to_string_pretty(&metadata)?,
        )?;

        // Create a simple file with variables
        fs::write(dir.join("README.txt"), "# {{NAME}}\n\nA project.")?;

        // Create a subdirectory with variable name
        let subdir = dir.join("src").join("{{NAME}}");
        fs::create_dir_all(&subdir)?;
        fs::write(subdir.join("main.py"), "# {{NAME}} main file\nprint('Hello from {{NAME}}')")?;

        Ok(())
    }

    #[test]
    fn test_template_from_path() {
        let temp_dir = env::temp_dir().join("template_test_from_path");
        let _ = fs::remove_dir_all(&temp_dir);

        create_test_template(&temp_dir).unwrap();

        let template = Template::from_path(&temp_dir, true).unwrap();
        assert!(template.metadata.is_some());
        assert_eq!(template.description(), "A test template");

        // Cleanup
        let _ = fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_find_variables() {
        let temp_dir = env::temp_dir().join("template_test_find_vars");
        let _ = fs::remove_dir_all(&temp_dir);

        create_test_template(&temp_dir).unwrap();

        let template = Template::from_path(&temp_dir, true).unwrap();
        let vars = template.find_variables().unwrap();

        assert!(vars.contains(&"NAME".to_string()));

        // Cleanup
        let _ = fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_instantiate() {
        let temp_dir = env::temp_dir().join("template_test_instantiate");
        let template_dir = temp_dir.join("template");
        let project_dir = temp_dir.join("myproject");
        let _ = fs::remove_dir_all(&temp_dir);

        create_test_template(&template_dir).unwrap();

        let template = Template::from_path(&template_dir, true).unwrap();
        let vars = Variables::new("myproject");

        template.instantiate(&project_dir, &vars, false).unwrap();

        // Check that files were created with substituted content
        let readme = fs::read_to_string(project_dir.join("README.txt")).unwrap();
        assert!(readme.contains("# myproject"));

        let main = fs::read_to_string(project_dir.join("src/myproject/main.py")).unwrap();
        assert!(main.contains("myproject main file"));

        // Cleanup
        let _ = fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_store_list_empty() {
        let temp_dir = env::temp_dir().join("template_test_store");
        let builtin_dir = temp_dir.join("builtin");
        let user_dir = temp_dir.join("user");
        let _ = fs::remove_dir_all(&temp_dir);
        fs::create_dir_all(&builtin_dir).unwrap();

        let store = TemplateStore::with_dirs(builtin_dir, user_dir).unwrap();
        let templates = store.list().unwrap();
        assert!(templates.is_empty());

        // Cleanup
        let _ = fs::remove_dir_all(&temp_dir);
    }
}
