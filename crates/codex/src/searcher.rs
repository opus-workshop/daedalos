//! Search functionality for the code index.
//!
//! Uses SQLite FTS5 for keyword-based search as a fallback when
//! semantic embeddings are not available.

use anyhow::Result;
use rusqlite::{params, Connection};

use crate::indexer::CodeIndex;

/// A search result with ranking information
#[derive(Debug, Clone)]
pub struct SearchResult {
    /// Path relative to project root
    pub file_path: String,
    /// Starting line number
    pub start_line: usize,
    /// Ending line number
    pub end_line: usize,
    /// The code content
    pub content: String,
    /// Type of chunk
    pub chunk_type: String,
    /// Name of the function/class/etc
    pub name: String,
    /// Relevance score (higher is better)
    pub score: f64,
}

impl SearchResult {
    /// Get file:line location string
    pub fn location(&self) -> String {
        format!("{}:{}", self.file_path, self.start_line)
    }

    /// Get a preview of the content (first line, truncated)
    pub fn preview(&self) -> String {
        let first_line = self.content.lines().next().unwrap_or("").trim();
        if first_line.len() > 100 {
            format!("{}...", &first_line[..97])
        } else {
            first_line.to_string()
        }
    }
}

/// Search the code index
pub struct CodeSearcher<'a> {
    index: &'a mut CodeIndex,
}

impl<'a> CodeSearcher<'a> {
    /// Create a new searcher for the given index
    pub fn new(index: &'a mut CodeIndex) -> Self {
        Self { index }
    }

    /// Search for code matching the query
    pub fn search(
        &mut self,
        query: &str,
        limit: usize,
        file_filter: Option<&str>,
        type_filter: Option<&str>,
    ) -> Result<Vec<SearchResult>> {
        let conn = Connection::open(&self.index.db_path)?;

        // Build the FTS5 query
        // Escape special characters and add wildcards for partial matching
        let fts_query = self.build_fts_query(query);

        let mut sql = String::from(
            r#"
            SELECT
                c.file_path,
                c.start_line,
                c.end_line,
                c.content,
                c.chunk_type,
                c.name,
                bm25(chunks_fts) as score
            FROM chunks_fts
            JOIN chunks c ON chunks_fts.rowid = c.id
            WHERE chunks_fts MATCH ?
            "#,
        );

        let mut params_vec: Vec<Box<dyn rusqlite::ToSql>> = vec![Box::new(fts_query.clone())];

        if let Some(filter) = file_filter {
            sql.push_str(" AND c.file_path LIKE ?");
            params_vec.push(Box::new(format!("%{}%", filter)));
        }

        if let Some(filter) = type_filter {
            sql.push_str(" AND c.chunk_type = ?");
            params_vec.push(Box::new(filter.to_string()));
        }

        sql.push_str(" ORDER BY score LIMIT ?");
        params_vec.push(Box::new(limit as i64));

        let params_refs: Vec<&dyn rusqlite::ToSql> = params_vec.iter().map(|p| p.as_ref()).collect();

        let mut stmt = conn.prepare(&sql)?;
        let results = stmt
            .query_map(params_refs.as_slice(), |row| {
                Ok(SearchResult {
                    file_path: row.get(0)?,
                    start_line: row.get::<_, i64>(1)? as usize,
                    end_line: row.get::<_, i64>(2)? as usize,
                    content: row.get(3)?,
                    chunk_type: row.get(4)?,
                    name: row.get(5)?,
                    score: row.get::<_, f64>(6)?.abs(), // BM25 returns negative scores
                })
            })?
            .filter_map(|r| r.ok())
            .collect();

        Ok(results)
    }

    /// Build an FTS5 query from a natural language query
    fn build_fts_query(&self, query: &str) -> String {
        // Split query into words and create an OR query with prefix matching
        let words: Vec<&str> = query
            .split(|c: char| !c.is_alphanumeric() && c != '_')
            .filter(|w| w.len() > 1)
            .collect();

        if words.is_empty() {
            return query.to_string();
        }

        // Create prefix match for each word
        let terms: Vec<String> = words
            .iter()
            .map(|w| format!("\"{}\"*", w))
            .collect();

        // Use OR to match any of the terms
        terms.join(" OR ")
    }

    /// Search within a specific file
    pub fn search_file(&mut self, query: &str, file_path: &str, limit: usize) -> Result<Vec<SearchResult>> {
        self.search(query, limit, Some(file_path), None)
    }

    /// Find code similar to a specific location
    pub fn find_similar(
        &mut self,
        file_path: &str,
        line: usize,
        limit: usize,
    ) -> Result<Vec<SearchResult>> {
        let conn = Connection::open(&self.index.db_path)?;

        // Find the chunk at the specified location
        let reference_content: Option<String> = conn
            .query_row(
                r#"
                SELECT content FROM chunks
                WHERE file_path = ? AND start_line <= ? AND end_line >= ?
                LIMIT 1
                "#,
                params![file_path, line as i64, line as i64],
                |row| row.get(0),
            )
            .ok();

        let reference = match reference_content {
            Some(c) => c,
            None => return Ok(Vec::new()),
        };

        // Extract keywords from the reference content
        let keywords: Vec<&str> = reference
            .split(|c: char| !c.is_alphanumeric() && c != '_')
            .filter(|w| w.len() > 2)
            .take(20)
            .collect();

        if keywords.is_empty() {
            return Ok(Vec::new());
        }

        // Build FTS query from keywords
        let fts_query: String = keywords
            .iter()
            .map(|w| format!("\"{}\"", w))
            .collect::<Vec<_>>()
            .join(" OR ");

        // Search for similar content, excluding the reference chunk
        let mut stmt = conn.prepare(
            r#"
            SELECT
                c.file_path,
                c.start_line,
                c.end_line,
                c.content,
                c.chunk_type,
                c.name,
                bm25(chunks_fts) as score
            FROM chunks_fts
            JOIN chunks c ON chunks_fts.rowid = c.id
            WHERE chunks_fts MATCH ?
            AND NOT (c.file_path = ? AND c.start_line <= ? AND c.end_line >= ?)
            ORDER BY score
            LIMIT ?
            "#,
        )?;

        let results = stmt
            .query_map(
                params![fts_query, file_path, line as i64, line as i64, limit as i64],
                |row| {
                    Ok(SearchResult {
                        file_path: row.get(0)?,
                        start_line: row.get::<_, i64>(1)? as usize,
                        end_line: row.get::<_, i64>(2)? as usize,
                        content: row.get(3)?,
                        chunk_type: row.get(4)?,
                        name: row.get(5)?,
                        score: row.get::<_, f64>(6)?.abs(),
                    })
                },
            )?
            .filter_map(|r| r.ok())
            .collect();

        Ok(results)
    }
}

/// Format search results for display
pub fn format_results(results: &[SearchResult], show_content: bool) -> String {
    if results.is_empty() {
        return "No results found.".to_string();
    }

    let mut lines = Vec::new();

    for (i, result) in results.iter().enumerate() {
        let score_pct = (result.score * 100.0).min(100.0);
        lines.push(format!("{}. [{:.0}] {}", i + 1, score_pct, result.location()));
        lines.push(format!("   {}: {}", result.chunk_type, result.name));

        if show_content {
            for line in result.content.lines().take(5) {
                let truncated = if line.len() > 80 {
                    format!("{}...", &line[..77])
                } else {
                    line.to_string()
                };
                lines.push(format!("   | {}", truncated));
            }
            if result.content.lines().count() > 5 {
                lines.push("   | ...".to_string());
            }
        }

        lines.push(String::new());
    }

    lines.join("\n")
}

/// Format search results as JSON
pub fn format_results_json(results: &[SearchResult]) -> Result<String> {
    use serde_json::json;

    let items: Vec<_> = results
        .iter()
        .map(|r| {
            json!({
                "file_path": r.file_path,
                "start_line": r.start_line,
                "end_line": r.end_line,
                "chunk_type": r.chunk_type,
                "name": r.name,
                "score": r.score,
                "content": r.content,
            })
        })
        .collect();

    Ok(serde_json::to_string_pretty(&json!({
        "results": items,
        "count": results.len(),
    }))?)
}
