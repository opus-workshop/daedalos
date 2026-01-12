//! Language parsers for extracting symbols and dependencies

use std::path::Path;

use regex::Regex;

/// Represents a code symbol (function, class, etc.)
#[derive(Debug, Clone)]
pub struct Symbol {
    pub name: String,
    pub symbol_type: String,
    pub line_start: i64,
    pub line_end: i64,
    pub signature: Option<String>,
    pub visibility: Option<String>,
}

/// Represents an import/dependency
#[derive(Debug, Clone)]
pub struct Dependency {
    pub target_path: String,
    pub import_type: String,
}

/// Result of parsing a file
#[derive(Debug, Clone)]
pub struct ParseResult {
    pub file_type: String,
    pub symbols: Vec<Symbol>,
    pub dependencies: Vec<Dependency>,
}

/// Get list of supported file extensions
pub fn supported_extensions() -> Vec<&'static str> {
    vec![
        ".rs", ".py", ".ts", ".tsx", ".js", ".jsx", ".swift", ".go",
        ".java", ".kt", ".rb", ".php", ".ex", ".exs", ".c", ".cpp",
        ".h", ".hpp", ".cs",
    ]
}

/// Get appropriate parser for a file
pub fn get_parser(file_path: &Path) -> Option<Box<dyn Parser>> {
    let ext = file_path.extension()?.to_str()?.to_lowercase();

    match ext.as_str() {
        "rs" => Some(Box::new(RustParser)),
        "py" => Some(Box::new(PythonParser)),
        "ts" | "tsx" | "js" | "jsx" => Some(Box::new(TypeScriptParser)),
        "swift" => Some(Box::new(SwiftParser)),
        "go" => Some(Box::new(GoParser)),
        "java" | "kt" => Some(Box::new(JavaParser)),
        "rb" => Some(Box::new(RubyParser)),
        "c" | "cpp" | "h" | "hpp" => Some(Box::new(CParser)),
        _ => None,
    }
}

/// Parser trait for language-specific parsing
pub trait Parser {
    fn parse(&self, content: &str) -> ParseResult;
}

/// Find end of code block by matching braces
fn find_block_end(lines: &[&str], start: usize, open_char: char, close_char: char) -> usize {
    let mut depth = 0;
    let mut found_open = false;

    for (i, line) in lines.iter().enumerate().skip(start.saturating_sub(1)) {
        for char in line.chars() {
            if char == open_char {
                depth += 1;
                found_open = true;
            } else if char == close_char {
                depth -= 1;
                if found_open && depth == 0 {
                    return i + 1;
                }
            }
        }
    }

    start
}

/// Clean up a signature line
fn clean_signature(line: &str) -> String {
    line.trim().trim_end_matches('{').trim().to_string()
}

// ============================================================================
// Rust Parser
// ============================================================================

struct RustParser;

impl Parser for RustParser {
    fn parse(&self, content: &str) -> ParseResult {
        let mut symbols = Vec::new();
        let mut dependencies = Vec::new();
        let lines: Vec<&str> = content.lines().collect();

        let use_re = Regex::new(r"^use\s+([\w:]+)").unwrap();
        let extern_re = Regex::new(r"^extern\s+crate\s+(\w+)").unwrap();
        let mod_re = Regex::new(r"^(pub(?:\(.+?\))?\s+)?mod\s+(\w+)").unwrap();
        let struct_re = Regex::new(r"^(pub(?:\(.+?\))?\s+)?struct\s+(\w+)").unwrap();
        let enum_re = Regex::new(r"^(pub(?:\(.+?\))?\s+)?enum\s+(\w+)").unwrap();
        let trait_re = Regex::new(r"^(pub(?:\(.+?\))?\s+)?trait\s+(\w+)").unwrap();
        let impl_re = Regex::new(r"^impl(?:<.+?>)?\s+(?:(\w+)\s+for\s+)?(\w+)").unwrap();
        let fn_re = Regex::new(r"^(pub(?:\(.+?\))?\s+)?(async\s+)?(unsafe\s+)?fn\s+(\w+)").unwrap();
        let const_re = Regex::new(r"^(pub(?:\(.+?\))?\s+)?const\s+(\w+)").unwrap();
        let static_re = Regex::new(r"^(pub(?:\(.+?\))?\s+)?static\s+(\w+)").unwrap();

        for (i, line) in lines.iter().enumerate() {
            let line_num = i + 1;
            let trimmed = line.trim();

            // Skip comments
            if trimmed.starts_with("//") || trimmed.starts_with("/*") {
                continue;
            }

            // Use statements
            if let Some(caps) = use_re.captures(trimmed) {
                dependencies.push(Dependency {
                    target_path: caps[1].to_string(),
                    import_type: "use".to_string(),
                });
                continue;
            }

            // Extern crate
            if let Some(caps) = extern_re.captures(trimmed) {
                dependencies.push(Dependency {
                    target_path: caps[1].to_string(),
                    import_type: "extern".to_string(),
                });
                continue;
            }

            // Modules
            if let Some(caps) = mod_re.captures(trimmed) {
                let visibility = if caps.get(1).is_some() { "public" } else { "private" };
                let end = if trimmed.contains('{') {
                    find_block_end(&lines, line_num, '{', '}')
                } else {
                    line_num
                };
                symbols.push(Symbol {
                    name: caps[2].to_string(),
                    symbol_type: "module".to_string(),
                    line_start: line_num as i64,
                    line_end: end as i64,
                    signature: Some(clean_signature(trimmed)),
                    visibility: Some(visibility.to_string()),
                });
                continue;
            }

            // Structs
            if let Some(caps) = struct_re.captures(trimmed) {
                let visibility = if caps.get(1).is_some() { "public" } else { "private" };
                let end = find_block_end(&lines, line_num, '{', '}');
                symbols.push(Symbol {
                    name: caps[2].to_string(),
                    symbol_type: "struct".to_string(),
                    line_start: line_num as i64,
                    line_end: end as i64,
                    signature: Some(clean_signature(trimmed)),
                    visibility: Some(visibility.to_string()),
                });
                continue;
            }

            // Enums
            if let Some(caps) = enum_re.captures(trimmed) {
                let visibility = if caps.get(1).is_some() { "public" } else { "private" };
                let end = find_block_end(&lines, line_num, '{', '}');
                symbols.push(Symbol {
                    name: caps[2].to_string(),
                    symbol_type: "enum".to_string(),
                    line_start: line_num as i64,
                    line_end: end as i64,
                    signature: Some(clean_signature(trimmed)),
                    visibility: Some(visibility.to_string()),
                });
                continue;
            }

            // Traits
            if let Some(caps) = trait_re.captures(trimmed) {
                let visibility = if caps.get(1).is_some() { "public" } else { "private" };
                let end = find_block_end(&lines, line_num, '{', '}');
                symbols.push(Symbol {
                    name: caps[2].to_string(),
                    symbol_type: "trait".to_string(),
                    line_start: line_num as i64,
                    line_end: end as i64,
                    signature: Some(clean_signature(trimmed)),
                    visibility: Some(visibility.to_string()),
                });
                continue;
            }

            // Impl blocks
            if let Some(caps) = impl_re.captures(trimmed) {
                let trait_name = caps.get(1).map(|m| m.as_str());
                let type_name = &caps[2];
                let name = if let Some(trait_name) = trait_name {
                    format!("{} for {}", trait_name, type_name)
                } else {
                    type_name.to_string()
                };
                let end = find_block_end(&lines, line_num, '{', '}');
                symbols.push(Symbol {
                    name,
                    symbol_type: "impl".to_string(),
                    line_start: line_num as i64,
                    line_end: end as i64,
                    signature: Some(clean_signature(trimmed)),
                    visibility: Some("internal".to_string()),
                });
                continue;
            }

            // Functions
            if let Some(caps) = fn_re.captures(trimmed) {
                let visibility = if caps.get(1).is_some() { "public" } else { "private" };
                let end = find_block_end(&lines, line_num, '{', '}');
                symbols.push(Symbol {
                    name: caps[4].to_string(),
                    symbol_type: "function".to_string(),
                    line_start: line_num as i64,
                    line_end: end as i64,
                    signature: Some(clean_signature(trimmed)),
                    visibility: Some(visibility.to_string()),
                });
                continue;
            }

            // Constants
            if let Some(caps) = const_re.captures(trimmed) {
                let visibility = if caps.get(1).is_some() { "public" } else { "private" };
                symbols.push(Symbol {
                    name: caps[2].to_string(),
                    symbol_type: "constant".to_string(),
                    line_start: line_num as i64,
                    line_end: line_num as i64,
                    signature: Some(clean_signature(trimmed)),
                    visibility: Some(visibility.to_string()),
                });
                continue;
            }

            // Statics
            if let Some(caps) = static_re.captures(trimmed) {
                let visibility = if caps.get(1).is_some() { "public" } else { "private" };
                symbols.push(Symbol {
                    name: caps[2].to_string(),
                    symbol_type: "static".to_string(),
                    line_start: line_num as i64,
                    line_end: line_num as i64,
                    signature: Some(clean_signature(trimmed)),
                    visibility: Some(visibility.to_string()),
                });
            }
        }

        ParseResult {
            file_type: "rust".to_string(),
            symbols,
            dependencies,
        }
    }
}

// ============================================================================
// Python Parser
// ============================================================================

struct PythonParser;

impl Parser for PythonParser {
    fn parse(&self, content: &str) -> ParseResult {
        let mut symbols = Vec::new();
        let mut dependencies = Vec::new();
        let lines: Vec<&str> = content.lines().collect();

        let import_re = Regex::new(r"^import\s+(\S+)").unwrap();
        let from_re = Regex::new(r"^from\s+(\S+)\s+import").unwrap();
        let class_re = Regex::new(r"^class\s+(\w+)").unwrap();
        let func_re = Regex::new(r"^(\s*)def\s+(\w+)").unwrap();
        let async_func_re = Regex::new(r"^(\s*)async\s+def\s+(\w+)").unwrap();

        for (i, line) in lines.iter().enumerate() {
            let line_num = i + 1;

            // Import statements
            if let Some(caps) = import_re.captures(line) {
                dependencies.push(Dependency {
                    target_path: caps[1].to_string(),
                    import_type: "import".to_string(),
                });
                continue;
            }

            if let Some(caps) = from_re.captures(line) {
                dependencies.push(Dependency {
                    target_path: caps[1].to_string(),
                    import_type: "from".to_string(),
                });
                continue;
            }

            // Classes
            if let Some(caps) = class_re.captures(line) {
                symbols.push(Symbol {
                    name: caps[1].to_string(),
                    symbol_type: "class".to_string(),
                    line_start: line_num as i64,
                    line_end: line_num as i64,
                    signature: Some(clean_signature(line)),
                    visibility: None,
                });
                continue;
            }

            // Async functions
            if let Some(caps) = async_func_re.captures(line) {
                let indent = caps[1].len();
                let visibility = if caps[2].starts_with('_') {
                    "private"
                } else {
                    "public"
                };
                let sym_type = if indent > 0 { "method" } else { "function" };
                symbols.push(Symbol {
                    name: caps[2].to_string(),
                    symbol_type: sym_type.to_string(),
                    line_start: line_num as i64,
                    line_end: line_num as i64,
                    signature: Some(clean_signature(line)),
                    visibility: Some(visibility.to_string()),
                });
                continue;
            }

            // Functions
            if let Some(caps) = func_re.captures(line) {
                let indent = caps[1].len();
                let visibility = if caps[2].starts_with('_') {
                    "private"
                } else {
                    "public"
                };
                let sym_type = if indent > 0 { "method" } else { "function" };
                symbols.push(Symbol {
                    name: caps[2].to_string(),
                    symbol_type: sym_type.to_string(),
                    line_start: line_num as i64,
                    line_end: line_num as i64,
                    signature: Some(clean_signature(line)),
                    visibility: Some(visibility.to_string()),
                });
            }
        }

        ParseResult {
            file_type: "python".to_string(),
            symbols,
            dependencies,
        }
    }
}

// ============================================================================
// TypeScript/JavaScript Parser
// ============================================================================

struct TypeScriptParser;

impl Parser for TypeScriptParser {
    fn parse(&self, content: &str) -> ParseResult {
        let mut symbols = Vec::new();
        let mut dependencies = Vec::new();
        let lines: Vec<&str> = content.lines().collect();

        let import_re = Regex::new(r#"import\s+.*from\s+['"]([\w@/.-]+)['"]"#).unwrap();
        let require_re = Regex::new(r#"require\(['"]([\w@/.-]+)['"]\)"#).unwrap();
        let func_re = Regex::new(r"^(export\s+)?(async\s+)?function\s+(\w+)").unwrap();
        let arrow_re = Regex::new(r"^(export\s+)?(const|let|var)\s+(\w+)\s*=\s*(async\s+)?\(").unwrap();
        let class_re = Regex::new(r"^(export\s+)?class\s+(\w+)").unwrap();
        let interface_re = Regex::new(r"^(export\s+)?interface\s+(\w+)").unwrap();
        let type_re = Regex::new(r"^(export\s+)?type\s+(\w+)").unwrap();
        let enum_re = Regex::new(r"^(export\s+)?enum\s+(\w+)").unwrap();

        for (i, line) in lines.iter().enumerate() {
            let line_num = i + 1;
            let trimmed = line.trim();

            // Import statements
            if let Some(caps) = import_re.captures(trimmed) {
                dependencies.push(Dependency {
                    target_path: caps[1].to_string(),
                    import_type: "import".to_string(),
                });
            }

            if let Some(caps) = require_re.captures(trimmed) {
                dependencies.push(Dependency {
                    target_path: caps[1].to_string(),
                    import_type: "require".to_string(),
                });
            }

            // Functions
            if let Some(caps) = func_re.captures(trimmed) {
                let visibility = if caps.get(1).is_some() { "public" } else { "private" };
                let end = find_block_end(&lines, line_num, '{', '}');
                symbols.push(Symbol {
                    name: caps[3].to_string(),
                    symbol_type: "function".to_string(),
                    line_start: line_num as i64,
                    line_end: end as i64,
                    signature: Some(clean_signature(trimmed)),
                    visibility: Some(visibility.to_string()),
                });
                continue;
            }

            // Arrow functions
            if let Some(caps) = arrow_re.captures(trimmed) {
                let visibility = if caps.get(1).is_some() { "public" } else { "private" };
                let end = find_block_end(&lines, line_num, '{', '}');
                symbols.push(Symbol {
                    name: caps[3].to_string(),
                    symbol_type: "function".to_string(),
                    line_start: line_num as i64,
                    line_end: end as i64,
                    signature: Some(clean_signature(trimmed)),
                    visibility: Some(visibility.to_string()),
                });
                continue;
            }

            // Classes
            if let Some(caps) = class_re.captures(trimmed) {
                let visibility = if caps.get(1).is_some() { "public" } else { "private" };
                let end = find_block_end(&lines, line_num, '{', '}');
                symbols.push(Symbol {
                    name: caps[2].to_string(),
                    symbol_type: "class".to_string(),
                    line_start: line_num as i64,
                    line_end: end as i64,
                    signature: Some(clean_signature(trimmed)),
                    visibility: Some(visibility.to_string()),
                });
                continue;
            }

            // Interfaces
            if let Some(caps) = interface_re.captures(trimmed) {
                let visibility = if caps.get(1).is_some() { "public" } else { "private" };
                let end = find_block_end(&lines, line_num, '{', '}');
                symbols.push(Symbol {
                    name: caps[2].to_string(),
                    symbol_type: "interface".to_string(),
                    line_start: line_num as i64,
                    line_end: end as i64,
                    signature: Some(clean_signature(trimmed)),
                    visibility: Some(visibility.to_string()),
                });
                continue;
            }

            // Type aliases
            if let Some(caps) = type_re.captures(trimmed) {
                let visibility = if caps.get(1).is_some() { "public" } else { "private" };
                symbols.push(Symbol {
                    name: caps[2].to_string(),
                    symbol_type: "type".to_string(),
                    line_start: line_num as i64,
                    line_end: line_num as i64,
                    signature: Some(clean_signature(trimmed)),
                    visibility: Some(visibility.to_string()),
                });
                continue;
            }

            // Enums
            if let Some(caps) = enum_re.captures(trimmed) {
                let visibility = if caps.get(1).is_some() { "public" } else { "private" };
                let end = find_block_end(&lines, line_num, '{', '}');
                symbols.push(Symbol {
                    name: caps[2].to_string(),
                    symbol_type: "enum".to_string(),
                    line_start: line_num as i64,
                    line_end: end as i64,
                    signature: Some(clean_signature(trimmed)),
                    visibility: Some(visibility.to_string()),
                });
            }
        }

        ParseResult {
            file_type: "typescript".to_string(),
            symbols,
            dependencies,
        }
    }
}

// ============================================================================
// Swift Parser
// ============================================================================

struct SwiftParser;

impl Parser for SwiftParser {
    fn parse(&self, content: &str) -> ParseResult {
        let mut symbols = Vec::new();
        let mut dependencies = Vec::new();
        let lines: Vec<&str> = content.lines().collect();

        let import_re = Regex::new(r"^import\s+(\w+)").unwrap();
        let class_re = Regex::new(r"^(public\s+|private\s+|internal\s+|open\s+|fileprivate\s+)?class\s+(\w+)").unwrap();
        let struct_re = Regex::new(r"^(public\s+|private\s+|internal\s+)?struct\s+(\w+)").unwrap();
        let enum_re = Regex::new(r"^(public\s+|private\s+|internal\s+)?enum\s+(\w+)").unwrap();
        let protocol_re = Regex::new(r"^(public\s+|private\s+|internal\s+)?protocol\s+(\w+)").unwrap();
        let func_re = Regex::new(r"^(public\s+|private\s+|internal\s+|open\s+|override\s+)*(func|init)\s+(\w+)").unwrap();

        for (i, line) in lines.iter().enumerate() {
            let line_num = i + 1;
            let trimmed = line.trim();

            // Import statements
            if let Some(caps) = import_re.captures(trimmed) {
                dependencies.push(Dependency {
                    target_path: caps[1].to_string(),
                    import_type: "import".to_string(),
                });
                continue;
            }

            // Classes
            if let Some(caps) = class_re.captures(trimmed) {
                let visibility = caps.get(1).map(|m| m.as_str().trim()).unwrap_or("internal");
                let end = find_block_end(&lines, line_num, '{', '}');
                symbols.push(Symbol {
                    name: caps[2].to_string(),
                    symbol_type: "class".to_string(),
                    line_start: line_num as i64,
                    line_end: end as i64,
                    signature: Some(clean_signature(trimmed)),
                    visibility: Some(visibility.to_string()),
                });
                continue;
            }

            // Structs
            if let Some(caps) = struct_re.captures(trimmed) {
                let visibility = caps.get(1).map(|m| m.as_str().trim()).unwrap_or("internal");
                let end = find_block_end(&lines, line_num, '{', '}');
                symbols.push(Symbol {
                    name: caps[2].to_string(),
                    symbol_type: "struct".to_string(),
                    line_start: line_num as i64,
                    line_end: end as i64,
                    signature: Some(clean_signature(trimmed)),
                    visibility: Some(visibility.to_string()),
                });
                continue;
            }

            // Enums
            if let Some(caps) = enum_re.captures(trimmed) {
                let visibility = caps.get(1).map(|m| m.as_str().trim()).unwrap_or("internal");
                let end = find_block_end(&lines, line_num, '{', '}');
                symbols.push(Symbol {
                    name: caps[2].to_string(),
                    symbol_type: "enum".to_string(),
                    line_start: line_num as i64,
                    line_end: end as i64,
                    signature: Some(clean_signature(trimmed)),
                    visibility: Some(visibility.to_string()),
                });
                continue;
            }

            // Protocols
            if let Some(caps) = protocol_re.captures(trimmed) {
                let visibility = caps.get(1).map(|m| m.as_str().trim()).unwrap_or("internal");
                let end = find_block_end(&lines, line_num, '{', '}');
                symbols.push(Symbol {
                    name: caps[2].to_string(),
                    symbol_type: "protocol".to_string(),
                    line_start: line_num as i64,
                    line_end: end as i64,
                    signature: Some(clean_signature(trimmed)),
                    visibility: Some(visibility.to_string()),
                });
                continue;
            }

            // Functions
            if let Some(caps) = func_re.captures(trimmed) {
                let visibility = caps.get(1).map(|m| m.as_str().trim()).unwrap_or("internal");
                let end = find_block_end(&lines, line_num, '{', '}');
                symbols.push(Symbol {
                    name: caps[3].to_string(),
                    symbol_type: "function".to_string(),
                    line_start: line_num as i64,
                    line_end: end as i64,
                    signature: Some(clean_signature(trimmed)),
                    visibility: Some(visibility.to_string()),
                });
            }
        }

        ParseResult {
            file_type: "swift".to_string(),
            symbols,
            dependencies,
        }
    }
}

// ============================================================================
// Go Parser
// ============================================================================

struct GoParser;

impl Parser for GoParser {
    fn parse(&self, content: &str) -> ParseResult {
        let mut symbols = Vec::new();
        let mut dependencies = Vec::new();
        let lines: Vec<&str> = content.lines().collect();

        let import_re = Regex::new(r#"import\s+(?:\w+\s+)?["']([^"']+)["']"#).unwrap();
        let import_block_re = Regex::new(r#"^\s*(?:\w+\s+)?["']([^"']+)["']"#).unwrap();
        let func_re = Regex::new(r"^func\s+(?:\([^)]+\)\s+)?(\w+)").unwrap();
        let type_re = Regex::new(r"^type\s+(\w+)\s+(struct|interface)").unwrap();
        let const_re = Regex::new(r"^const\s+(\w+)").unwrap();
        let var_re = Regex::new(r"^var\s+(\w+)").unwrap();

        let mut in_import_block = false;

        for (i, line) in lines.iter().enumerate() {
            let line_num = i + 1;
            let trimmed = line.trim();

            // Track import blocks
            if trimmed == "import (" {
                in_import_block = true;
                continue;
            }
            if in_import_block {
                if trimmed == ")" {
                    in_import_block = false;
                    continue;
                }
                if let Some(caps) = import_block_re.captures(trimmed) {
                    dependencies.push(Dependency {
                        target_path: caps[1].to_string(),
                        import_type: "import".to_string(),
                    });
                }
                continue;
            }

            // Single import
            if let Some(caps) = import_re.captures(trimmed) {
                dependencies.push(Dependency {
                    target_path: caps[1].to_string(),
                    import_type: "import".to_string(),
                });
                continue;
            }

            // Functions
            if let Some(caps) = func_re.captures(trimmed) {
                let name = &caps[1];
                let visibility = if name.chars().next().map(|c| c.is_uppercase()).unwrap_or(false) {
                    "public"
                } else {
                    "private"
                };
                let end = find_block_end(&lines, line_num, '{', '}');
                symbols.push(Symbol {
                    name: name.to_string(),
                    symbol_type: "function".to_string(),
                    line_start: line_num as i64,
                    line_end: end as i64,
                    signature: Some(clean_signature(trimmed)),
                    visibility: Some(visibility.to_string()),
                });
                continue;
            }

            // Types (struct/interface)
            if let Some(caps) = type_re.captures(trimmed) {
                let name = &caps[1];
                let type_kind = &caps[2];
                let visibility = if name.chars().next().map(|c| c.is_uppercase()).unwrap_or(false) {
                    "public"
                } else {
                    "private"
                };
                let end = find_block_end(&lines, line_num, '{', '}');
                symbols.push(Symbol {
                    name: name.to_string(),
                    symbol_type: type_kind.to_string(),
                    line_start: line_num as i64,
                    line_end: end as i64,
                    signature: Some(clean_signature(trimmed)),
                    visibility: Some(visibility.to_string()),
                });
                continue;
            }

            // Constants
            if let Some(caps) = const_re.captures(trimmed) {
                let name = &caps[1];
                let visibility = if name.chars().next().map(|c| c.is_uppercase()).unwrap_or(false) {
                    "public"
                } else {
                    "private"
                };
                symbols.push(Symbol {
                    name: name.to_string(),
                    symbol_type: "constant".to_string(),
                    line_start: line_num as i64,
                    line_end: line_num as i64,
                    signature: Some(clean_signature(trimmed)),
                    visibility: Some(visibility.to_string()),
                });
                continue;
            }

            // Variables
            if let Some(caps) = var_re.captures(trimmed) {
                let name = &caps[1];
                let visibility = if name.chars().next().map(|c| c.is_uppercase()).unwrap_or(false) {
                    "public"
                } else {
                    "private"
                };
                symbols.push(Symbol {
                    name: name.to_string(),
                    symbol_type: "variable".to_string(),
                    line_start: line_num as i64,
                    line_end: line_num as i64,
                    signature: Some(clean_signature(trimmed)),
                    visibility: Some(visibility.to_string()),
                });
            }
        }

        ParseResult {
            file_type: "go".to_string(),
            symbols,
            dependencies,
        }
    }
}

// ============================================================================
// Java/Kotlin Parser
// ============================================================================

struct JavaParser;

impl Parser for JavaParser {
    fn parse(&self, content: &str) -> ParseResult {
        let mut symbols = Vec::new();
        let mut dependencies = Vec::new();
        let lines: Vec<&str> = content.lines().collect();

        let import_re = Regex::new(r"^import\s+([\w.]+)").unwrap();
        let class_re = Regex::new(r"^(public\s+|private\s+|protected\s+)?(abstract\s+)?class\s+(\w+)").unwrap();
        let interface_re = Regex::new(r"^(public\s+|private\s+|protected\s+)?interface\s+(\w+)").unwrap();
        let method_re = Regex::new(r"^\s*(public|private|protected)?\s*(static\s+)?([\w<>\[\]]+)\s+(\w+)\s*\(").unwrap();

        for (i, line) in lines.iter().enumerate() {
            let line_num = i + 1;
            let trimmed = line.trim();

            // Import statements
            if let Some(caps) = import_re.captures(trimmed) {
                dependencies.push(Dependency {
                    target_path: caps[1].to_string(),
                    import_type: "import".to_string(),
                });
                continue;
            }

            // Classes
            if let Some(caps) = class_re.captures(trimmed) {
                let visibility = caps.get(1).map(|m| m.as_str().trim()).unwrap_or("package");
                let end = find_block_end(&lines, line_num, '{', '}');
                symbols.push(Symbol {
                    name: caps[3].to_string(),
                    symbol_type: "class".to_string(),
                    line_start: line_num as i64,
                    line_end: end as i64,
                    signature: Some(clean_signature(trimmed)),
                    visibility: Some(visibility.to_string()),
                });
                continue;
            }

            // Interfaces
            if let Some(caps) = interface_re.captures(trimmed) {
                let visibility = caps.get(1).map(|m| m.as_str().trim()).unwrap_or("package");
                let end = find_block_end(&lines, line_num, '{', '}');
                symbols.push(Symbol {
                    name: caps[2].to_string(),
                    symbol_type: "interface".to_string(),
                    line_start: line_num as i64,
                    line_end: end as i64,
                    signature: Some(clean_signature(trimmed)),
                    visibility: Some(visibility.to_string()),
                });
                continue;
            }

            // Methods
            if let Some(caps) = method_re.captures(trimmed) {
                let visibility = caps.get(1).map(|m| m.as_str()).unwrap_or("package");
                let end = find_block_end(&lines, line_num, '{', '}');
                symbols.push(Symbol {
                    name: caps[4].to_string(),
                    symbol_type: "method".to_string(),
                    line_start: line_num as i64,
                    line_end: end as i64,
                    signature: Some(clean_signature(trimmed)),
                    visibility: Some(visibility.to_string()),
                });
            }
        }

        ParseResult {
            file_type: "java".to_string(),
            symbols,
            dependencies,
        }
    }
}

// ============================================================================
// Ruby Parser
// ============================================================================

struct RubyParser;

impl Parser for RubyParser {
    fn parse(&self, content: &str) -> ParseResult {
        let mut symbols = Vec::new();
        let mut dependencies = Vec::new();
        let lines: Vec<&str> = content.lines().collect();

        let require_re = Regex::new(r#"require\s+['"]([\w/.-]+)['"]"#).unwrap();
        let require_relative_re = Regex::new(r#"require_relative\s+['"]([\w/.-]+)['"]"#).unwrap();
        let class_re = Regex::new(r"^class\s+(\w+)").unwrap();
        let module_re = Regex::new(r"^module\s+(\w+)").unwrap();
        let def_re = Regex::new(r"^\s*def\s+(self\.)?(\w+[!?=]?)").unwrap();

        for (i, line) in lines.iter().enumerate() {
            let line_num = i + 1;
            let trimmed = line.trim();

            // Require statements
            if let Some(caps) = require_re.captures(trimmed) {
                dependencies.push(Dependency {
                    target_path: caps[1].to_string(),
                    import_type: "require".to_string(),
                });
            }

            if let Some(caps) = require_relative_re.captures(trimmed) {
                dependencies.push(Dependency {
                    target_path: caps[1].to_string(),
                    import_type: "require_relative".to_string(),
                });
            }

            // Classes
            if let Some(caps) = class_re.captures(trimmed) {
                symbols.push(Symbol {
                    name: caps[1].to_string(),
                    symbol_type: "class".to_string(),
                    line_start: line_num as i64,
                    line_end: line_num as i64,
                    signature: Some(clean_signature(trimmed)),
                    visibility: Some("public".to_string()),
                });
                continue;
            }

            // Modules
            if let Some(caps) = module_re.captures(trimmed) {
                symbols.push(Symbol {
                    name: caps[1].to_string(),
                    symbol_type: "module".to_string(),
                    line_start: line_num as i64,
                    line_end: line_num as i64,
                    signature: Some(clean_signature(trimmed)),
                    visibility: Some("public".to_string()),
                });
                continue;
            }

            // Methods
            if let Some(caps) = def_re.captures(trimmed) {
                let is_class_method = caps.get(1).is_some();
                let name = &caps[2];
                let visibility = if name.starts_with('_') { "private" } else { "public" };
                let sym_type = if is_class_method { "class_method" } else { "method" };
                symbols.push(Symbol {
                    name: name.to_string(),
                    symbol_type: sym_type.to_string(),
                    line_start: line_num as i64,
                    line_end: line_num as i64,
                    signature: Some(clean_signature(trimmed)),
                    visibility: Some(visibility.to_string()),
                });
            }
        }

        ParseResult {
            file_type: "ruby".to_string(),
            symbols,
            dependencies,
        }
    }
}

// ============================================================================
// C/C++ Parser
// ============================================================================

struct CParser;

impl Parser for CParser {
    fn parse(&self, content: &str) -> ParseResult {
        let mut symbols = Vec::new();
        let mut dependencies = Vec::new();
        let lines: Vec<&str> = content.lines().collect();

        let include_re = Regex::new(r#"#include\s*[<"]([\w/.-]+)[>"]"#).unwrap();
        let func_re = Regex::new(r"^\s*([\w*]+)\s+(\w+)\s*\([^)]*\)\s*\{?").unwrap();
        let struct_re = Regex::new(r"^(typedef\s+)?struct\s+(\w+)").unwrap();
        let enum_re = Regex::new(r"^(typedef\s+)?enum\s+(\w+)").unwrap();
        let class_re = Regex::new(r"^class\s+(\w+)").unwrap();

        for (i, line) in lines.iter().enumerate() {
            let line_num = i + 1;
            let trimmed = line.trim();

            // Skip comments
            if trimmed.starts_with("//") || trimmed.starts_with("/*") {
                continue;
            }

            // Include statements
            if let Some(caps) = include_re.captures(trimmed) {
                dependencies.push(Dependency {
                    target_path: caps[1].to_string(),
                    import_type: "include".to_string(),
                });
                continue;
            }

            // Structs
            if let Some(caps) = struct_re.captures(trimmed) {
                let end = find_block_end(&lines, line_num, '{', '}');
                symbols.push(Symbol {
                    name: caps[2].to_string(),
                    symbol_type: "struct".to_string(),
                    line_start: line_num as i64,
                    line_end: end as i64,
                    signature: Some(clean_signature(trimmed)),
                    visibility: Some("public".to_string()),
                });
                continue;
            }

            // Enums
            if let Some(caps) = enum_re.captures(trimmed) {
                let end = find_block_end(&lines, line_num, '{', '}');
                symbols.push(Symbol {
                    name: caps[2].to_string(),
                    symbol_type: "enum".to_string(),
                    line_start: line_num as i64,
                    line_end: end as i64,
                    signature: Some(clean_signature(trimmed)),
                    visibility: Some("public".to_string()),
                });
                continue;
            }

            // Classes (C++)
            if let Some(caps) = class_re.captures(trimmed) {
                let end = find_block_end(&lines, line_num, '{', '}');
                symbols.push(Symbol {
                    name: caps[1].to_string(),
                    symbol_type: "class".to_string(),
                    line_start: line_num as i64,
                    line_end: end as i64,
                    signature: Some(clean_signature(trimmed)),
                    visibility: Some("public".to_string()),
                });
                continue;
            }

            // Functions
            if let Some(caps) = func_re.captures(trimmed) {
                let return_type = &caps[1];
                // Skip if it looks like a control statement or common keywords
                if ["if", "else", "while", "for", "switch", "return", "sizeof", "typedef"].contains(&return_type) {
                    continue;
                }
                let end = find_block_end(&lines, line_num, '{', '}');
                symbols.push(Symbol {
                    name: caps[2].to_string(),
                    symbol_type: "function".to_string(),
                    line_start: line_num as i64,
                    line_end: end as i64,
                    signature: Some(clean_signature(trimmed)),
                    visibility: Some("public".to_string()),
                });
            }
        }

        ParseResult {
            file_type: "c".to_string(),
            symbols,
            dependencies,
        }
    }
}
