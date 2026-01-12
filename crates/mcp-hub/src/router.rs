//! Request router - routes MCP requests to appropriate servers
//!
//! The router handles tool discovery and request routing, enabling clients
//! to call tools without knowing which server provides them.

#![allow(dead_code)]

use std::collections::HashMap;

use crate::registry::{ServerRegistry, BUILTIN_SERVERS};

/// Tool routing information
#[derive(Debug, Clone)]
pub struct ToolRoute {
    pub tool_name: String,
    pub server_name: String,
    pub description: String,
}

/// Router for MCP requests
pub struct Router {
    /// Map from tool name to server name
    tool_map: HashMap<String, String>,
}

impl Router {
    /// Create a new router
    pub fn new() -> Self {
        let mut tool_map = HashMap::new();

        // Build tool -> server mapping from builtin servers
        for server in BUILTIN_SERVERS {
            for tool in server.tools {
                tool_map.insert(tool.to_string(), server.name.to_string());
            }
        }

        Self { tool_map }
    }

    /// Create a router from a registry
    pub fn from_registry(registry: &ServerRegistry) -> Self {
        let mut tool_map = HashMap::new();

        // Build tool -> server mapping
        for tool_info in registry.get_tools() {
            tool_map.insert(tool_info.name, tool_info.server);
        }

        Self { tool_map }
    }

    /// Find which server provides a tool
    pub fn find_server(&self, tool_name: &str) -> Option<&str> {
        self.tool_map.get(tool_name).map(|s| s.as_str())
    }

    /// Get all tools with their routes
    pub fn get_routes(&self) -> Vec<ToolRoute> {
        self.tool_map
            .iter()
            .map(|(tool, server)| ToolRoute {
                tool_name: tool.clone(),
                server_name: server.clone(),
                description: format!("{} from {}", tool, server),
            })
            .collect()
    }

    /// Check if a tool exists
    pub fn has_tool(&self, tool_name: &str) -> bool {
        self.tool_map.contains_key(tool_name)
    }

    /// Get number of registered tools
    pub fn tool_count(&self) -> usize {
        self.tool_map.len()
    }
}

impl Default for Router {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_router_creation() {
        let router = Router::new();
        assert!(router.tool_count() > 0);
    }

    #[test]
    fn test_find_server() {
        let router = Router::new();

        // read_file is provided by at least one server (could be filesystem or google-drive due to collision)
        let server = router.find_server("read_file");
        assert!(server.is_some());
        assert!(server == Some("filesystem") || server == Some("google-drive"));

        // create_issue should be provided by github (unique tool)
        let server = router.find_server("create_issue");
        assert_eq!(server, Some("github"));

        // Unknown tool should return None
        let server = router.find_server("nonexistent_tool");
        assert_eq!(server, None);
    }

    #[test]
    fn test_has_tool() {
        let router = Router::new();

        assert!(router.has_tool("read_file"));
        assert!(router.has_tool("query")); // postgres
        assert!(!router.has_tool("nonexistent_tool"));
    }

    #[test]
    fn test_get_routes() {
        let router = Router::new();
        let routes = router.get_routes();

        assert!(!routes.is_empty());

        // Find the create_issue route (unique to github)
        let create_issue_route = routes.iter().find(|r| r.tool_name == "create_issue");
        assert!(create_issue_route.is_some());
        assert_eq!(create_issue_route.unwrap().server_name, "github");
    }
}
