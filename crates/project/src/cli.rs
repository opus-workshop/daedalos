//! CLI command definitions and handlers

use std::path::PathBuf;

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};

use crate::cache::CacheManager;
use crate::database::ProjectDatabase;
use crate::detectors::{detect_architecture, detect_project_type, get_project_description};

/// project - Pre-computed codebase intelligence
///
/// Index and query project structure, dependencies, and conventions.
#[derive(Parser)]
#[command(name = "project")]
#[command(version = "1.0.0")]
#[command(about = "Pre-computed codebase intelligence - understand any codebase instantly")]
#[command(after_help = "\
TRIGGER:
    Use project info when entering a new project or needing architectural context.
    Saves massive context by providing pre-indexed information.

EXAMPLES:
    project info                Show project summary and architecture
    project tree                Display project structure as a tree
    project tree --depth 5      Deeper tree view
    project symbols             List all functions, classes, structs
    project symbols -t function List only functions
    project deps src/api.rs     Show what this file imports
    project dependents auth.rs  Show what imports this file
    project index               Build/rebuild the project index
    project stats               File counts, symbol counts, line counts

PHILOSOPHY:
    Agents waste massive context reading files to understand architecture.
    project provides that understanding in seconds, pre-computed.")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Show project information and architecture
    #[command(about = "Show project summary with type, architecture, and key modules")]
    Info {
        /// Path to project (default: current directory)
        #[arg(default_value = ".")]
        path: PathBuf,

        /// Output as JSON
        #[arg(long, default_value = "false")]
        json: bool,

        /// Force re-index
        #[arg(long, default_value = "false")]
        refresh: bool,
    },

    /// Show project file tree
    #[command(about = "Display project structure as a tree")]
    Tree {
        /// Path to project (default: current directory)
        #[arg(default_value = ".")]
        path: PathBuf,

        /// Maximum depth to display
        #[arg(short, long, default_value = "3")]
        depth: usize,

        /// Output as JSON
        #[arg(long, default_value = "false")]
        json: bool,

        /// Force re-index
        #[arg(long, default_value = "false")]
        refresh: bool,
    },

    /// List all symbols in the project
    #[command(about = "List functions, classes, structs, etc.")]
    Symbols {
        /// Path to project (default: current directory)
        #[arg(default_value = ".")]
        path: PathBuf,

        /// Filter by symbol type (function, struct, enum, trait, class, etc.)
        #[arg(short = 't', long = "type")]
        type_filter: Option<String>,

        /// Output as JSON
        #[arg(long, default_value = "false")]
        json: bool,

        /// Maximum results
        #[arg(short = 'n', long, default_value = "50")]
        limit: usize,

        /// Force re-index
        #[arg(long, default_value = "false")]
        refresh: bool,
    },

    /// Show dependencies of a file
    #[command(about = "List what a file imports/depends on")]
    Deps {
        /// File path to analyze
        file_path: String,

        /// Project root
        #[arg(short, long, default_value = ".")]
        project: PathBuf,

        /// Output as JSON
        #[arg(long, default_value = "false")]
        json: bool,

        /// Force re-index
        #[arg(long, default_value = "false")]
        refresh: bool,
    },

    /// Show files that depend on a file
    #[command(about = "List what imports/depends on this file")]
    Dependents {
        /// File path to analyze
        file_path: String,

        /// Project root
        #[arg(short, long, default_value = ".")]
        project: PathBuf,

        /// Output as JSON
        #[arg(long, default_value = "false")]
        json: bool,

        /// Force re-index
        #[arg(long, default_value = "false")]
        refresh: bool,
    },

    /// Index or re-index the project
    #[command(about = "Build or rebuild the project index")]
    Index {
        /// Path to project (default: current directory)
        #[arg(default_value = ".")]
        path: PathBuf,

        /// Full re-index (clear cache first)
        #[arg(long, default_value = "false")]
        full: bool,
    },

    /// Show project statistics
    #[command(about = "Display file counts, symbol counts, and line counts")]
    Stats {
        /// Path to project (default: current directory)
        #[arg(default_value = ".")]
        path: PathBuf,

        /// Output as JSON
        #[arg(long, default_value = "false")]
        json: bool,

        /// Force re-index
        #[arg(long, default_value = "false")]
        refresh: bool,
    },
}

/// Show project info
pub fn cmd_info(path: PathBuf, json_output: bool, refresh: bool) -> Result<()> {
    let path = path
        .canonicalize()
        .context("Failed to resolve project path")?;

    let mut cache = CacheManager::new(&path)?;

    if refresh || cache.is_stale() {
        index_project(&path, &mut cache)?;
    }

    let db = cache.get_database()?;
    let summary = build_summary(&path, &db)?;

    if json_output {
        println!("{}", serde_json::to_string_pretty(&summary)?);
    } else {
        print_summary(&summary);
    }

    Ok(())
}

/// Show project tree
pub fn cmd_tree(path: PathBuf, depth: usize, json_output: bool, refresh: bool) -> Result<()> {
    let path = path
        .canonicalize()
        .context("Failed to resolve project path")?;

    let mut cache = CacheManager::new(&path)?;

    if refresh || cache.is_stale() {
        index_project(&path, &mut cache)?;
    }

    let db = cache.get_database()?;
    let files = db.get_all_files()?;

    if json_output {
        let tree = build_tree_json(&files);
        println!("{}", serde_json::to_string_pretty(&tree)?);
    } else {
        print_tree(&files, depth);
    }

    Ok(())
}

/// List symbols
pub fn cmd_symbols(
    path: PathBuf,
    type_filter: Option<String>,
    json_output: bool,
    limit: usize,
    refresh: bool,
) -> Result<()> {
    let path = path
        .canonicalize()
        .context("Failed to resolve project path")?;

    let mut cache = CacheManager::new(&path)?;

    if refresh || cache.is_stale() {
        index_project(&path, &mut cache)?;
    }

    let db = cache.get_database()?;
    let mut symbols = db.get_all_symbols()?;

    // Filter by type if specified
    if let Some(ref filter) = type_filter {
        symbols.retain(|s| s.symbol_type.eq_ignore_ascii_case(filter));
    }

    // Limit results
    symbols.truncate(limit);

    if json_output {
        println!("{}", serde_json::to_string_pretty(&symbols)?);
    } else if symbols.is_empty() {
        println!("No symbols found");
    } else {
        println!(
            "{:<30} {:<12} {:<40} {}",
            "NAME", "TYPE", "FILE", "LINE"
        );
        println!("{}", "-".repeat(90));
        for sym in &symbols {
            let name = if sym.name.len() > 28 {
                format!("{}...", &sym.name[..28])
            } else {
                sym.name.clone()
            };
            let file = if sym.file_path.len() > 38 {
                format!("{}...", &sym.file_path[..38])
            } else {
                sym.file_path.clone()
            };
            println!(
                "{:<30} {:<12} {:<40} {}",
                name, sym.symbol_type, file, sym.line_start
            );
        }
        println!("\nTotal: {} symbols", symbols.len());
    }

    Ok(())
}

/// Show file dependencies
pub fn cmd_deps(file_path: String, project: PathBuf, json_output: bool, refresh: bool) -> Result<()> {
    let project = project
        .canonicalize()
        .context("Failed to resolve project path")?;

    let mut cache = CacheManager::new(&project)?;

    if refresh || cache.is_stale() {
        index_project(&project, &mut cache)?;
    }

    let db = cache.get_database()?;
    let deps = db.get_file_dependencies(&file_path)?;

    if json_output {
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({
                "file": file_path,
                "imports": deps
            }))?
        );
    } else {
        println!("Dependencies of {}:", file_path);
        if deps.is_empty() {
            println!("  (none)");
        } else {
            for dep in &deps {
                println!("  -> {}", dep);
            }
        }
    }

    Ok(())
}

/// Show file dependents
pub fn cmd_dependents(
    file_path: String,
    project: PathBuf,
    json_output: bool,
    refresh: bool,
) -> Result<()> {
    let project = project
        .canonicalize()
        .context("Failed to resolve project path")?;

    let mut cache = CacheManager::new(&project)?;

    if refresh || cache.is_stale() {
        index_project(&project, &mut cache)?;
    }

    let db = cache.get_database()?;
    let dependents = db.get_file_dependents(&file_path)?;

    if json_output {
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({
                "file": file_path,
                "imported_by": dependents
            }))?
        );
    } else {
        println!("Files that import {}:", file_path);
        if dependents.is_empty() {
            println!("  (none)");
        } else {
            for dep in &dependents {
                println!("  <- {}", dep);
            }
        }
    }

    Ok(())
}

/// Index the project
pub fn cmd_index(path: PathBuf, full: bool) -> Result<()> {
    let path = path
        .canonicalize()
        .context("Failed to resolve project path")?;

    println!("Indexing {}...", path.display());

    let mut cache = CacheManager::new(&path)?;

    if full {
        cache.clear()?;
    }

    index_project(&path, &mut cache)?;

    let db = cache.get_database()?;
    let stats = db.get_stats()?;

    println!(
        "Indexed {} files, {} symbols",
        stats.file_count, stats.symbol_count
    );

    Ok(())
}

/// Show project statistics
pub fn cmd_stats(path: PathBuf, json_output: bool, refresh: bool) -> Result<()> {
    let path = path
        .canonicalize()
        .context("Failed to resolve project path")?;

    let mut cache = CacheManager::new(&path)?;

    if refresh || cache.is_stale() {
        index_project(&path, &mut cache)?;
    }

    let db = cache.get_database()?;
    let stats = db.get_stats()?;

    if json_output {
        println!("{}", serde_json::to_string_pretty(&stats)?);
    } else {
        println!("Project Statistics");
        println!("{}", "=".repeat(40));
        println!("Total Files: {}", stats.file_count);
        println!("Total Symbols: {}", stats.symbol_count);
        println!("Total Dependencies: {}", stats.dependency_count);

        if !stats.lines_by_type.is_empty() {
            println!();
            println!("Lines by Type:");
            let mut sorted: Vec<_> = stats.lines_by_type.iter().collect();
            sorted.sort_by(|a, b| b.1.cmp(a.1));
            for (file_type, count) in sorted {
                println!("  {}: {}", file_type, count);
            }
        }
    }

    Ok(())
}

// Helper functions

fn index_project(path: &std::path::Path, cache: &mut CacheManager) -> Result<()> {
    use crate::parsers::{get_parser, supported_extensions};
    use ignore::WalkBuilder;
    use sha2::{Digest, Sha256};
    use std::collections::HashSet;

    // Detect and store project type first
    let project_type = detect_project_type(path);
    cache.set_project_type(&project_type)?;

    // Open a separate database connection to avoid borrow issues
    let db = cache.open_database()?;

    // Get supported extensions
    let extensions: HashSet<_> = supported_extensions().into_iter().collect();

    // Walk the directory respecting .gitignore
    let walker = WalkBuilder::new(path)
        .hidden(false)
        .git_ignore(true)
        .git_global(true)
        .git_exclude(true)
        .build();

    for entry in walker.flatten() {
        let entry_path = entry.path();

        if !entry_path.is_file() {
            continue;
        }

        // Check extension
        let ext = entry_path
            .extension()
            .and_then(|e| e.to_str())
            .map(|e| format!(".{}", e.to_lowercase()));

        if let Some(ref ext) = ext {
            if !extensions.contains(ext.as_str()) {
                continue;
            }
        } else {
            continue;
        }

        // Read file content
        let content = match std::fs::read_to_string(entry_path) {
            Ok(c) => c,
            Err(_) => continue,
        };

        // Calculate hash
        let mut hasher = Sha256::new();
        hasher.update(content.as_bytes());
        let hash = hex::encode(&hasher.finalize()[..8]);

        // Get relative path
        let rel_path = entry_path
            .strip_prefix(path)
            .unwrap_or(entry_path)
            .to_string_lossy()
            .to_string();

        // Check if file is unchanged
        if let Ok(Some(existing)) = db.get_file(&rel_path) {
            if existing.hash == hash {
                continue;
            }
        }

        // Get parser and parse
        if let Some(parser) = get_parser(entry_path) {
            let result = parser.parse(&content);

            // Get file metadata
            let metadata = entry_path.metadata().ok();
            let size = metadata.as_ref().map(|m| m.len()).unwrap_or(0) as i64;
            let lines = content.lines().count() as i64;
            let modified = metadata
                .and_then(|m| m.modified().ok())
                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                .map(|d| d.as_secs_f64())
                .unwrap_or(0.0);

            // Upsert file
            let file_id = db.upsert_file(&rel_path, &result.file_type, size, lines, modified, &hash)?;

            // Clear old symbols and dependencies
            db.clear_symbols_for_file(file_id)?;
            db.clear_dependencies_for_file(file_id)?;

            // Add symbols
            for sym in &result.symbols {
                db.add_symbol(
                    file_id,
                    &sym.name,
                    &sym.symbol_type,
                    sym.line_start,
                    sym.line_end,
                    sym.signature.as_deref(),
                    sym.visibility.as_deref(),
                )?;
            }

            // Add dependencies
            for dep in &result.dependencies {
                db.add_dependency(file_id, &dep.target_path, &dep.import_type)?;
            }
        }
    }

    cache.mark_fresh()?;

    Ok(())
}

fn build_summary(
    path: &std::path::Path,
    db: &ProjectDatabase,
) -> Result<serde_json::Value> {
    let project_type = detect_project_type(path);
    let architecture = detect_architecture(db)?;
    let stats = db.get_stats()?;

    // Find entry points
    let entry_points = find_entry_points(db)?;

    // Find key modules
    let modules = find_key_modules(db)?;

    // Find external dependencies
    let external_deps = find_external_deps(db)?;

    Ok(serde_json::json!({
        "name": path.file_name().and_then(|n| n.to_str()).unwrap_or("unknown"),
        "path": path.to_string_lossy(),
        "type": project_type,
        "type_description": get_project_description(&project_type),
        "architecture": architecture,
        "entry_points": entry_points,
        "modules": modules,
        "dependencies": external_deps,
        "stats": stats
    }))
}

fn find_entry_points(db: &ProjectDatabase) -> Result<Vec<String>> {
    let files = db.get_all_files()?;
    let patterns = [
        "main.py",
        "__main__.py",
        "app.py",
        "main.ts",
        "index.ts",
        "app.ts",
        "main.js",
        "index.js",
        "app.js",
        "main.swift",
        "App.swift",
        "main.rs",
        "lib.rs",
        "main.go",
    ];

    let mut entry_points = Vec::new();
    for file in &files {
        let name = file.path.rsplit('/').next().unwrap_or(&file.path);
        if patterns.contains(&name) {
            entry_points.push(file.path.clone());
        }
    }

    entry_points.truncate(5);
    Ok(entry_points)
}

fn find_key_modules(db: &ProjectDatabase) -> Result<Vec<serde_json::Value>> {
    use std::collections::HashMap;

    let files = db.get_all_files()?;
    let mut dir_counts: HashMap<String, i32> = HashMap::new();

    for file in &files {
        let parts: Vec<_> = file.path.split('/').collect();
        if parts.len() > 1 {
            *dir_counts.entry(parts[0].to_string()).or_default() += 1;
        }
    }

    let mut sorted: Vec<_> = dir_counts.into_iter().collect();
    sorted.sort_by(|a, b| b.1.cmp(&a.1));

    let descriptions = [
        ("src", "Source code"),
        ("lib", "Library code"),
        ("test", "Tests"),
        ("tests", "Tests"),
        ("spec", "Specifications/Tests"),
        ("docs", "Documentation"),
        ("config", "Configuration"),
        ("scripts", "Scripts"),
        ("components", "UI Components"),
        ("views", "Views"),
        ("models", "Data models"),
        ("services", "Services"),
        ("utils", "Utilities"),
        ("helpers", "Helper functions"),
        ("api", "API handlers"),
        ("crates", "Rust crates"),
        ("packages", "Packages"),
    ];

    let desc_map: HashMap<_, _> = descriptions.into_iter().collect();

    Ok(sorted
        .into_iter()
        .take(5)
        .map(|(name, count)| {
            let description = desc_map
                .get(name.to_lowercase().as_str())
                .map(|s| s.to_string())
                .unwrap_or_else(|| format!("{} files", count));
            serde_json::json!({
                "name": name,
                "description": description,
                "file_count": count
            })
        })
        .collect())
}

fn find_external_deps(db: &ProjectDatabase) -> Result<Vec<String>> {
    let deps = db.get_external_dependencies()?;
    Ok(deps.into_iter().take(10).collect())
}

fn print_summary(summary: &serde_json::Value) {
    let width = 65;

    println!("+{}+", "-".repeat(width));
    let title = format!(
        " PROJECT SUMMARY: {} ",
        summary["name"].as_str().unwrap_or("unknown")
    );
    println!("|{:^width$}|", title);
    println!("+{}+", "-".repeat(width));

    // Type and architecture
    println!(
        "| Type: {:<w$} |",
        summary["type_description"].as_str().unwrap_or("Unknown"),
        w = width - 8
    );

    if let Some(arch) = summary["architecture"].as_object() {
        println!(
            "| Architecture: {:<w$} |",
            arch.get("description")
                .and_then(|v| v.as_str())
                .unwrap_or("Unknown"),
            w = width - 16
        );
    }

    // Entry points
    if let Some(entries) = summary["entry_points"].as_array() {
        if !entries.is_empty() {
            let entry_str: Vec<_> = entries
                .iter()
                .filter_map(|e| e.as_str())
                .take(3)
                .collect();
            let entry = entry_str.join(" -> ");
            let entry = if entry.len() > width - 9 {
                format!("{}...", &entry[..width - 12])
            } else {
                entry
            };
            println!("| Entry: {:<w$} |", entry, w = width - 9);
        }
    }

    println!("|{:width$}|", "");

    // Key modules
    if let Some(modules) = summary["modules"].as_array() {
        if !modules.is_empty() {
            println!("| Key Modules:{:w$}|", "", w = width - 13);
            for module in modules.iter().take(5) {
                let name = module["name"].as_str().unwrap_or("");
                let desc = module["description"].as_str().unwrap_or("");
                let line = format!("  - {}: {}", name, desc);
                let line = if line.len() > width - 2 {
                    format!("{}...", &line[..width - 5])
                } else {
                    line
                };
                println!("|{:<width$}|", line);
            }
        }
    }

    println!("|{:width$}|", "");

    // Dependencies
    if let Some(deps) = summary["dependencies"].as_array() {
        if !deps.is_empty() {
            let dep_strs: Vec<_> = deps.iter().filter_map(|d| d.as_str()).take(5).collect();
            let dep_str = dep_strs.join(", ");
            let dep_str = if dep_str.len() > width - 16 {
                format!("{}...", &dep_str[..width - 19])
            } else {
                dep_str
            };
            println!("| Dependencies: {:<w$} |", dep_str, w = width - 16);
        }
    }

    // Stats
    if let Some(stats) = summary["stats"].as_object() {
        println!("|{:width$}|", "");
        let stats_str = format!(
            "Files: {} | Symbols: {}",
            stats
                .get("file_count")
                .and_then(|v| v.as_i64())
                .unwrap_or(0),
            stats
                .get("symbol_count")
                .and_then(|v| v.as_i64())
                .unwrap_or(0)
        );
        println!("| {:<w$} |", stats_str, w = width - 2);
    }

    println!("+{}+", "-".repeat(width));
}

fn build_tree_json(files: &[crate::database::FileRecord]) -> serde_json::Value {
    use std::collections::BTreeMap;

    let mut tree: BTreeMap<String, serde_json::Value> = BTreeMap::new();

    for file in files {
        let parts: Vec<_> = file.path.split('/').collect();
        insert_path(&mut tree, &parts, 0);
    }

    fn insert_path(tree: &mut BTreeMap<String, serde_json::Value>, parts: &[&str], depth: usize) {
        if depth >= parts.len() {
            return;
        }
        let part = parts[depth].to_string();
        if depth == parts.len() - 1 {
            tree.insert(part, serde_json::Value::Null);
        } else {
            let entry = tree
                .entry(part)
                .or_insert_with(|| serde_json::json!({}));
            if let serde_json::Value::Object(map) = entry {
                let mut inner: BTreeMap<String, serde_json::Value> = map
                    .iter()
                    .map(|(k, v)| (k.clone(), v.clone()))
                    .collect();
                insert_path(&mut inner, parts, depth + 1);
                *entry = serde_json::json!(inner);
            }
        }
    }

    serde_json::json!(tree)
}

fn print_tree(files: &[crate::database::FileRecord], max_depth: usize) {
    use std::collections::BTreeMap;

    // Build tree structure
    let mut tree: BTreeMap<String, BTreeMap<String, BTreeMap<String, Vec<String>>>> =
        BTreeMap::new();

    for file in files {
        let parts: Vec<_> = file.path.split('/').collect();
        match parts.len() {
            1 => {
                tree.entry(parts[0].to_string())
                    .or_default()
                    .entry(String::new())
                    .or_default();
            }
            2 => {
                tree.entry(parts[0].to_string())
                    .or_default()
                    .entry(parts[1].to_string())
                    .or_default();
            }
            _ if parts.len() > 2 => {
                tree.entry(parts[0].to_string())
                    .or_default()
                    .entry(parts[1].to_string())
                    .or_default()
                    .entry(parts[2].to_string())
                    .or_default()
                    .push(parts[3..].join("/"));
            }
            _ => {}
        }
    }

    // Print tree
    let dirs: Vec<_> = tree.keys().cloned().collect();
    for (i, dir) in dirs.iter().enumerate() {
        let is_last_dir = i == dirs.len() - 1;
        let prefix = if is_last_dir { "\\-- " } else { "|-- " };
        println!("{}{}", prefix, dir);

        if max_depth > 1 {
            if let Some(subdirs) = tree.get(dir) {
                let subdir_names: Vec<_> = subdirs.keys().filter(|k| !k.is_empty()).cloned().collect();
                for (j, subdir) in subdir_names.iter().enumerate() {
                    let is_last_subdir = j == subdir_names.len() - 1;
                    let prefix2 = if is_last_dir { "    " } else { "|   " };
                    let connector = if is_last_subdir { "\\-- " } else { "|-- " };
                    println!("{}{}{}", prefix2, connector, subdir);

                    if max_depth > 2 {
                        if let Some(subsubdirs) = subdirs.get(subdir) {
                            let subsubdir_names: Vec<_> = subsubdirs.keys().cloned().collect();
                            for (k, subsubdir) in subsubdir_names.iter().enumerate() {
                                let is_last_subsubdir = k == subsubdir_names.len() - 1;
                                let prefix3 = if is_last_subdir { "    " } else { "|   " };
                                let prefix4 = if is_last_subdir { "    " } else { "|   " };
                                let connector2 = if is_last_subsubdir {
                                    "\\-- "
                                } else {
                                    "|-- "
                                };
                                println!("{}{}{}{}", prefix2, prefix3, connector2, subsubdir);

                                // Show files count if any
                                if let Some(inner_files) = subsubdirs.get(subsubdir) {
                                    if !inner_files.is_empty() {
                                        let prefix5 = if is_last_subsubdir { "    " } else { "|   " };
                                        println!(
                                            "{}{}{}    ({} more files)",
                                            prefix2, prefix4, prefix5, inner_files.len()
                                        );
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}
