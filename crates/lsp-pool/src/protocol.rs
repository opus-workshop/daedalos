//! LSP Protocol implementation
//!
//! Handles encoding/decoding of LSP messages over stdin/stdout.

// Allow unused code - these types are kept for API completeness
#![allow(dead_code)]

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::io::{BufRead, BufReader, Read, Write};

/// LSP message with headers and content
#[derive(Debug, Clone)]
pub struct LspMessage {
    pub content: Value,
}

impl LspMessage {
    pub fn new(content: Value) -> Self {
        Self { content }
    }

    /// Encode message with LSP headers (Content-Length)
    pub fn encode(&self) -> Result<Vec<u8>> {
        let body = serde_json::to_string(&self.content)?;
        let header = format!("Content-Length: {}\r\n\r\n", body.len());
        let mut message = header.into_bytes();
        message.extend(body.into_bytes());
        Ok(message)
    }

    /// Read a message from a buffered reader
    pub fn read_from<R: Read>(reader: &mut BufReader<R>) -> Result<Option<Self>> {
        // Read headers
        let mut content_length = 0usize;
        loop {
            let mut line = String::new();
            let bytes_read = reader.read_line(&mut line)?;
            if bytes_read == 0 {
                return Ok(None); // EOF
            }

            let line = line.trim();
            if line.is_empty() {
                break; // End of headers
            }

            if let Some(len) = line
                .to_lowercase()
                .strip_prefix("content-length:")
                .map(|s| s.trim())
            {
                content_length = len.parse().context("Invalid Content-Length")?;
            }
        }

        if content_length == 0 {
            return Ok(None);
        }

        // Read body
        let mut body = vec![0u8; content_length];
        reader.read_exact(&mut body)?;

        let content: Value = serde_json::from_slice(&body)?;
        Ok(Some(Self { content }))
    }

    /// Write message to a writer
    pub fn write_to<W: Write>(&self, writer: &mut W) -> Result<()> {
        let encoded = self.encode()?;
        writer.write_all(&encoded)?;
        writer.flush()?;
        Ok(())
    }
}

/// LSP Request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Request {
    pub jsonrpc: String,
    pub id: RequestId,
    pub method: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub params: Option<Value>,
}

impl Request {
    pub fn new(id: i64, method: &str, params: Option<Value>) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            id: RequestId::Number(id),
            method: method.to_string(),
            params,
        }
    }
}

/// LSP Response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Response {
    pub jsonrpc: String,
    pub id: RequestId,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<ResponseError>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResponseError {
    pub code: i64,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<Value>,
}

/// LSP Notification (no id, no response expected)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Notification {
    pub jsonrpc: String,
    pub method: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub params: Option<Value>,
}

impl Notification {
    pub fn new(method: &str, params: Option<Value>) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            method: method.to_string(),
            params,
        }
    }
}

/// Request ID can be string or number
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(untagged)]
pub enum RequestId {
    Number(i64),
    String(String),
}

impl From<i64> for RequestId {
    fn from(n: i64) -> Self {
        RequestId::Number(n)
    }
}

impl From<String> for RequestId {
    fn from(s: String) -> Self {
        RequestId::String(s)
    }
}

/// LSP Initialize params
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InitializeParams {
    pub process_id: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub root_uri: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub root_path: Option<String>,
    pub capabilities: ClientCapabilities,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClientCapabilities {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text_document: Option<TextDocumentClientCapabilities>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TextDocumentClientCapabilities {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hover: Option<HoverCapability>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub completion: Option<CompletionCapability>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub definition: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub references: Option<Value>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HoverCapability {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content_format: Option<Vec<String>>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CompletionCapability {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub completion_item: Option<CompletionItemCapability>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CompletionItemCapability {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub snippet_support: Option<bool>,
}

/// Text document identifier
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TextDocumentIdentifier {
    pub uri: String,
}

impl TextDocumentIdentifier {
    pub fn new(path: &std::path::Path) -> Self {
        Self {
            uri: format!("file://{}", path.display()),
        }
    }
}

/// Position in a text document (0-indexed)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Position {
    pub line: u32,
    pub character: u32,
}

impl Position {
    /// Create from 1-indexed line and column
    pub fn from_1indexed(line: u32, col: u32) -> Self {
        Self {
            line: line.saturating_sub(1),
            character: col.saturating_sub(1),
        }
    }
}

/// Text document position params
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TextDocumentPositionParams {
    pub text_document: TextDocumentIdentifier,
    pub position: Position,
}

/// Range in a text document
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Range {
    pub start: Position,
    pub end: Position,
}

/// Location (uri + range)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Location {
    pub uri: String,
    pub range: Range,
}

impl Location {
    /// Format as path:line:col
    pub fn format(&self) -> String {
        let path = self.uri.strip_prefix("file://").unwrap_or(&self.uri);
        let line = self.range.start.line + 1;
        let col = self.range.start.character + 1;
        format!("{}:{}:{}", path, line, col)
    }
}

/// Hover result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Hover {
    pub contents: HoverContents,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub range: Option<Range>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum HoverContents {
    String(String),
    MarkupContent(MarkupContent),
    Array(Vec<MarkedString>),
}

impl HoverContents {
    pub fn to_string(&self) -> String {
        match self {
            HoverContents::String(s) => s.clone(),
            HoverContents::MarkupContent(m) => m.value.clone(),
            HoverContents::Array(arr) => arr
                .iter()
                .map(|m| match m {
                    MarkedString::String(s) => s.clone(),
                    MarkedString::Object { value, .. } => value.clone(),
                })
                .collect::<Vec<_>>()
                .join("\n"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarkupContent {
    pub kind: String,
    pub value: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum MarkedString {
    String(String),
    Object { language: String, value: String },
}

/// Completion item
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CompletionItem {
    pub label: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub kind: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub documentation: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub insert_text: Option<String>,
}

impl CompletionItem {
    /// Get kind name
    pub fn kind_name(&self) -> &'static str {
        match self.kind {
            Some(1) => "Text",
            Some(2) => "Method",
            Some(3) => "Function",
            Some(4) => "Constructor",
            Some(5) => "Field",
            Some(6) => "Variable",
            Some(7) => "Class",
            Some(8) => "Interface",
            Some(9) => "Module",
            Some(10) => "Property",
            Some(11) => "Unit",
            Some(12) => "Value",
            Some(13) => "Enum",
            Some(14) => "Keyword",
            Some(15) => "Snippet",
            Some(16) => "Color",
            Some(17) => "File",
            Some(18) => "Reference",
            Some(19) => "Folder",
            Some(20) => "EnumMember",
            Some(21) => "Constant",
            Some(22) => "Struct",
            Some(23) => "Event",
            Some(24) => "Operator",
            Some(25) => "TypeParameter",
            _ => "",
        }
    }
}

/// Completion list
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CompletionList {
    pub is_incomplete: bool,
    pub items: Vec<CompletionItem>,
}

/// Diagnostic
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Diagnostic {
    pub range: Range,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub severity: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub code: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
    pub message: String,
}

impl Diagnostic {
    pub fn severity_str(&self) -> &'static str {
        match self.severity {
            Some(1) => "error",
            Some(2) => "warning",
            Some(3) => "info",
            Some(4) => "hint",
            _ => "unknown",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_message_encode() {
        let content = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "initialize"
        });
        let msg = LspMessage::new(content);
        let encoded = msg.encode().unwrap();
        let encoded_str = String::from_utf8(encoded).unwrap();
        assert!(encoded_str.starts_with("Content-Length:"));
        assert!(encoded_str.contains("\"jsonrpc\":\"2.0\""));
    }

    #[test]
    fn test_position_from_1indexed() {
        let pos = Position::from_1indexed(10, 5);
        assert_eq!(pos.line, 9);
        assert_eq!(pos.character, 4);
    }

    #[test]
    fn test_location_format() {
        let loc = Location {
            uri: "file:///home/user/test.rs".to_string(),
            range: Range {
                start: Position {
                    line: 9,
                    character: 4,
                },
                end: Position {
                    line: 9,
                    character: 10,
                },
            },
        };
        assert_eq!(loc.format(), "/home/user/test.rs:10:5");
    }
}
