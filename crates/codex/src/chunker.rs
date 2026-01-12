//! Code chunking for semantic search.
//!
//! Chunks code files into searchable segments based on semantic units
//! (functions, classes, structs) rather than fixed-size blocks.

use regex::Regex;
use std::path::Path;

/// A chunk of code for indexing
#[derive(Debug, Clone)]
pub struct CodeChunk {
    /// Path relative to project root
    pub file_path: String,
    /// Starting line number (1-indexed)
    pub start_line: usize,
    /// Ending line number (1-indexed)
    pub end_line: usize,
    /// The code content
    pub content: String,
    /// Type of chunk (function, class, struct, file, block)
    pub chunk_type: String,
    /// Name of the function/class/etc
    pub name: String,
}

impl CodeChunk {
    /// Get the file:line location string
    pub fn location(&self) -> String {
        format!("{}:{}", self.file_path, self.start_line)
    }
}

/// Chunker that splits code into semantic units
pub struct CodeChunker {
    // Compiled patterns for different languages
    python_pattern: Regex,
    rust_fn_pattern: Regex,
    rust_struct_pattern: Regex,
    rust_enum_pattern: Regex,
    rust_trait_pattern: Regex,
    rust_impl_pattern: Regex,
    js_fn_pattern: Regex,
    js_class_pattern: Regex,
    js_const_fn_pattern: Regex,
    go_fn_pattern: Regex,
    go_method_pattern: Regex,
    go_struct_pattern: Regex,
    go_interface_pattern: Regex,
}

impl Default for CodeChunker {
    fn default() -> Self {
        Self::new()
    }
}

impl CodeChunker {
    pub fn new() -> Self {
        Self {
            python_pattern: Regex::new(r"^(class|def|async def)\s+(\w+)").unwrap(),
            rust_fn_pattern: Regex::new(r"^\s*(pub\s+)?fn\s+(\w+)").unwrap(),
            rust_struct_pattern: Regex::new(r"^\s*(pub\s+)?struct\s+(\w+)").unwrap(),
            rust_enum_pattern: Regex::new(r"^\s*(pub\s+)?enum\s+(\w+)").unwrap(),
            rust_trait_pattern: Regex::new(r"^\s*(pub\s+)?trait\s+(\w+)").unwrap(),
            rust_impl_pattern: Regex::new(r"^\s*impl\s+(?:<[^>]+>\s+)?(\w+)").unwrap(),
            js_fn_pattern: Regex::new(r"^(export\s+)?(async\s+)?function\s+(\w+)").unwrap(),
            js_class_pattern: Regex::new(r"^(export\s+)?class\s+(\w+)").unwrap(),
            js_const_fn_pattern: Regex::new(r"^(export\s+)?const\s+(\w+)\s*=\s*(async\s+)?(\(|function)").unwrap(),
            go_fn_pattern: Regex::new(r"^func\s+(\w+)").unwrap(),
            go_method_pattern: Regex::new(r"^func\s+\([^)]+\)\s+(\w+)").unwrap(),
            go_struct_pattern: Regex::new(r"^type\s+(\w+)\s+struct").unwrap(),
            go_interface_pattern: Regex::new(r"^type\s+(\w+)\s+interface").unwrap(),
        }
    }

    /// Chunk a file into searchable segments
    pub fn chunk_file(&self, path: &str, content: &str) -> Vec<CodeChunk> {
        let ext = Path::new(path)
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_lowercase();

        let mut chunks = match ext.as_str() {
            "py" | "pyw" => self.chunk_python(path, content),
            "rs" => self.chunk_rust(path, content),
            "js" | "jsx" | "mjs" | "cjs" | "ts" | "tsx" | "mts" | "cts" => {
                self.chunk_javascript(path, content)
            }
            "go" => self.chunk_go(path, content),
            _ => self.chunk_generic(path, content),
        };

        // Always include whole file as a chunk for context
        if !content.trim().is_empty() {
            let line_count = content.lines().count();
            let truncated = if content.len() > 2000 {
                &content[..2000]
            } else {
                content
            };
            chunks.push(CodeChunk {
                file_path: path.to_string(),
                start_line: 1,
                end_line: line_count.max(1),
                content: truncated.to_string(),
                chunk_type: "file".to_string(),
                name: Path::new(path)
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or(path)
                    .to_string(),
            });
        }

        chunks
    }

    fn chunk_python(&self, path: &str, content: &str) -> Vec<CodeChunk> {
        let lines: Vec<&str> = content.lines().collect();
        let mut chunks = Vec::new();
        let mut current_chunk: Option<(usize, String, String)> = None; // (start_line, type, name)

        for (i, line) in lines.iter().enumerate() {
            let line_num = i + 1;
            if let Some(captures) = self.python_pattern.captures(line) {
                // Save previous chunk
                if let Some((start, chunk_type, name)) = current_chunk.take() {
                    let chunk_content = lines[start - 1..i].join("\n");
                    chunks.push(CodeChunk {
                        file_path: path.to_string(),
                        start_line: start,
                        end_line: i,
                        content: chunk_content,
                        chunk_type,
                        name,
                    });
                }

                let keyword = captures.get(1).map(|m| m.as_str()).unwrap_or("");
                let name = captures.get(2).map(|m| m.as_str()).unwrap_or("unknown");
                let chunk_type = if keyword == "class" {
                    "class"
                } else {
                    "function"
                };

                current_chunk = Some((line_num, chunk_type.to_string(), name.to_string()));
            }
        }

        // Don't forget the last chunk
        if let Some((start, chunk_type, name)) = current_chunk {
            let chunk_content = lines[start - 1..].join("\n");
            chunks.push(CodeChunk {
                file_path: path.to_string(),
                start_line: start,
                end_line: lines.len(),
                content: chunk_content,
                chunk_type,
                name,
            });
        }

        chunks
    }

    fn chunk_rust(&self, path: &str, content: &str) -> Vec<CodeChunk> {
        let lines: Vec<&str> = content.lines().collect();
        let mut chunks = Vec::new();
        let mut current_chunk: Option<(usize, String, String)> = None;

        for (i, line) in lines.iter().enumerate() {
            let line_num = i + 1;

            // Try each pattern
            let match_result = if let Some(captures) = self.rust_fn_pattern.captures(line) {
                Some(("function", captures.get(2).map(|m| m.as_str()).unwrap_or("unknown")))
            } else if let Some(captures) = self.rust_struct_pattern.captures(line) {
                Some(("struct", captures.get(2).map(|m| m.as_str()).unwrap_or("unknown")))
            } else if let Some(captures) = self.rust_enum_pattern.captures(line) {
                Some(("enum", captures.get(2).map(|m| m.as_str()).unwrap_or("unknown")))
            } else if let Some(captures) = self.rust_trait_pattern.captures(line) {
                Some(("trait", captures.get(2).map(|m| m.as_str()).unwrap_or("unknown")))
            } else if let Some(captures) = self.rust_impl_pattern.captures(line) {
                Some(("impl", captures.get(1).map(|m| m.as_str()).unwrap_or("unknown")))
            } else {
                None
            };

            if let Some((chunk_type, name)) = match_result {
                // Save previous chunk
                if let Some((start, prev_type, prev_name)) = current_chunk.take() {
                    let chunk_content = lines[start - 1..i].join("\n");
                    chunks.push(CodeChunk {
                        file_path: path.to_string(),
                        start_line: start,
                        end_line: i,
                        content: chunk_content,
                        chunk_type: prev_type,
                        name: prev_name,
                    });
                }

                current_chunk = Some((line_num, chunk_type.to_string(), name.to_string()));
            }
        }

        // Don't forget the last chunk
        if let Some((start, chunk_type, name)) = current_chunk {
            let chunk_content = lines[start - 1..].join("\n");
            chunks.push(CodeChunk {
                file_path: path.to_string(),
                start_line: start,
                end_line: lines.len(),
                content: chunk_content,
                chunk_type,
                name,
            });
        }

        chunks
    }

    fn chunk_javascript(&self, path: &str, content: &str) -> Vec<CodeChunk> {
        let lines: Vec<&str> = content.lines().collect();
        let mut chunks = Vec::new();
        let mut current_chunk: Option<(usize, String, String)> = None;

        for (i, line) in lines.iter().enumerate() {
            let line_num = i + 1;
            let trimmed = line.trim();

            // Try each pattern
            let match_result = if let Some(captures) = self.js_fn_pattern.captures(trimmed) {
                Some(("function", captures.get(3).map(|m| m.as_str()).unwrap_or("unknown")))
            } else if let Some(captures) = self.js_class_pattern.captures(trimmed) {
                Some(("class", captures.get(2).map(|m| m.as_str()).unwrap_or("unknown")))
            } else if let Some(captures) = self.js_const_fn_pattern.captures(trimmed) {
                Some(("function", captures.get(2).map(|m| m.as_str()).unwrap_or("unknown")))
            } else {
                None
            };

            if let Some((chunk_type, name)) = match_result {
                // Save previous chunk
                if let Some((start, prev_type, prev_name)) = current_chunk.take() {
                    let chunk_content = lines[start - 1..i].join("\n");
                    chunks.push(CodeChunk {
                        file_path: path.to_string(),
                        start_line: start,
                        end_line: i,
                        content: chunk_content,
                        chunk_type: prev_type,
                        name: prev_name,
                    });
                }

                current_chunk = Some((line_num, chunk_type.to_string(), name.to_string()));
            }
        }

        // Don't forget the last chunk
        if let Some((start, chunk_type, name)) = current_chunk {
            let chunk_content = lines[start - 1..].join("\n");
            chunks.push(CodeChunk {
                file_path: path.to_string(),
                start_line: start,
                end_line: lines.len(),
                content: chunk_content,
                chunk_type,
                name,
            });
        }

        chunks
    }

    fn chunk_go(&self, path: &str, content: &str) -> Vec<CodeChunk> {
        let lines: Vec<&str> = content.lines().collect();
        let mut chunks = Vec::new();
        let mut current_chunk: Option<(usize, String, String)> = None;

        for (i, line) in lines.iter().enumerate() {
            let line_num = i + 1;

            // Try each pattern
            let match_result = if let Some(captures) = self.go_fn_pattern.captures(line) {
                Some(("function", captures.get(1).map(|m| m.as_str()).unwrap_or("unknown")))
            } else if let Some(captures) = self.go_method_pattern.captures(line) {
                Some(("method", captures.get(1).map(|m| m.as_str()).unwrap_or("unknown")))
            } else if let Some(captures) = self.go_struct_pattern.captures(line) {
                Some(("struct", captures.get(1).map(|m| m.as_str()).unwrap_or("unknown")))
            } else if let Some(captures) = self.go_interface_pattern.captures(line) {
                Some(("interface", captures.get(1).map(|m| m.as_str()).unwrap_or("unknown")))
            } else {
                None
            };

            if let Some((chunk_type, name)) = match_result {
                // Save previous chunk
                if let Some((start, prev_type, prev_name)) = current_chunk.take() {
                    let chunk_content = lines[start - 1..i].join("\n");
                    chunks.push(CodeChunk {
                        file_path: path.to_string(),
                        start_line: start,
                        end_line: i,
                        content: chunk_content,
                        chunk_type: prev_type,
                        name: prev_name,
                    });
                }

                current_chunk = Some((line_num, chunk_type.to_string(), name.to_string()));
            }
        }

        // Don't forget the last chunk
        if let Some((start, chunk_type, name)) = current_chunk {
            let chunk_content = lines[start - 1..].join("\n");
            chunks.push(CodeChunk {
                file_path: path.to_string(),
                start_line: start,
                end_line: lines.len(),
                content: chunk_content,
                chunk_type,
                name,
            });
        }

        chunks
    }

    fn chunk_generic(&self, path: &str, content: &str) -> Vec<CodeChunk> {
        // Split into blocks of ~50 lines for unknown file types
        let lines: Vec<&str> = content.lines().collect();
        let chunk_size = 50;
        let mut chunks = Vec::new();

        for (block_num, chunk_lines) in lines.chunks(chunk_size).enumerate() {
            let start = block_num * chunk_size + 1;
            let end = start + chunk_lines.len() - 1;

            chunks.push(CodeChunk {
                file_path: path.to_string(),
                start_line: start,
                end_line: end,
                content: chunk_lines.join("\n"),
                chunk_type: "block".to_string(),
                name: format!("block_{}", block_num + 1),
            });
        }

        chunks
    }
}
