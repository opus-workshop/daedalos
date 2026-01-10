Daedalos MCP Server
===================

MCP server that exposes all Daedalos tools to Claude as native tools.

OVERVIEW
--------
Instead of calling tools via Bash, this MCP server lets Claude use
Daedalos tools directly through the Model Context Protocol.

INSTALLATION
------------
  ./install.sh

CONFIGURATION
-------------
Add to ~/.claude/settings.json:

  {
    "mcpServers": {
      "daedalos": {
        "command": "python3",
        "args": ["-m", "daedalos_mcp"]
      }
    }
  }

AVAILABLE TOOLS
---------------
The MCP server exposes these tools:

Loop (iteration primitive):
  - loop_start         Start iteration loop with promise
  - loop_status        Check loop status
  - loop_stop          Stop current loop

Verify (universal verification):
  - verify             Run project checks

Undo (file-level undo):
  - undo_checkpoint    Create named checkpoint
  - undo_last          Undo last change
  - undo_timeline      Show change timeline
  - undo_restore       Restore to checkpoint

Project (codebase intelligence):
  - project_info       Get project overview
  - project_symbols    List symbols
  - project_tree       Show file tree

Codex (semantic search):
  - codex_search       Search code by meaning
  - codex_index        Rebuild search index

Context (context management):
  - context_estimate   Estimate context usage
  - context_breakdown  Detailed breakdown

Error-db (error patterns):
  - error_match        Find error solutions
  - error_add          Add error pattern

Scratch (ephemeral environments):
  - scratch_new        Create scratch env
  - scratch_list       List scratches
  - scratch_destroy    Destroy scratch

Agent (multi-agent):
  - agent_spawn        Spawn new agent
  - agent_list         List agents
  - agent_focus        Focus agent
  - agent_search       Search agent outputs
  - agent_kill         Kill agent

REQUIREMENTS
------------
- Python 3.10+
- mcp package (pip install mcp)
- All Daedalos tools installed in ~/.local/bin

PART OF DAEDALOS
----------------
Tools designed BY AI, FOR AI development.
