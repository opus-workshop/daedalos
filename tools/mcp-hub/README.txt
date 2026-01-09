MCP Hub - Central MCP Server Management
=======================================

Central management for Model Context Protocol (MCP) servers.
Instead of configuring MCP servers per-project, manage them system-wide.

OVERVIEW
--------
MCP Hub:
- Discovers and catalogs available MCP servers
- Starts/stops servers on demand
- Routes tool requests to appropriate servers
- Provides unified capability discovery

USAGE
-----
  mcp-hub start              Start the hub daemon
  mcp-hub status             Show running servers
  mcp-hub list               List available servers
  mcp-hub enable filesystem  Enable a server
  mcp-hub tools              List available tools

  mcp-hub call read_file --path /etc/hosts
  mcp-hub call search_files --path . --regex "TODO"

BUILT-IN SERVERS
----------------
  filesystem   File system operations (read, write, list, search)
  github       GitHub operations (issues, PRs, code search)
  memory       Persistent memory for conversations
  sqlite       SQLite database operations
  fetch        HTTP fetch operations
  brave-search Brave Search API

INSTALLING SERVERS
------------------
  mcp-hub install filesystem           # Enable built-in
  mcp-hub install npm:@org/server-x    # Install from npm
  mcp-hub install github:user/repo     # Install from GitHub

CONFIGURATION
-------------
Config: ~/.config/daedalos/mcp-hub/config.yaml

  auto_start_servers:
    - filesystem
    - memory

  servers:
    custom:
      command: ["node", "/path/to/server.js"]
      env:
        API_KEY: "xxx"

INTEGRATION
-----------
MCP Hub works with:
- Claude Code (Anthropic)
- OpenCode
- Any MCP-compatible client

INSTALL
-------
  ./install.sh

PART OF DAEDALOS
----------------
Tools designed BY AI, FOR AI development.
