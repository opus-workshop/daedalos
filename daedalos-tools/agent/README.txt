Agent CLI - Multi-Agent Orchestration
======================================

Manage multiple Claude Code instances running in tmux sessions.
Part of the Daedalos toolsuite.

OVERVIEW
--------
The agent CLI lets you:
- Spawn multiple Claude Code agents in parallel
- Switch between agents with slots 1-9
- Search across all agent outputs
- Pause/resume agents
- Monitor agent status

REQUIREMENTS
------------
- bash 4.0+
- tmux 3.0+
- jq (for JSON parsing)
- fzf (optional, for fuzzy finding)
- claude (Claude Code CLI)

INSTALLATION
------------
  ./install.sh

USAGE
-----
  agent spawn -n mywork -p ~/project     # Spawn new agent
  agent spawn -t explorer --no-focus     # Spawn explorer in background
  agent list                             # List all agents
  agent focus mywork                     # Focus by name
  agent focus 1                          # Focus by slot
  agent status                           # Show all status
  agent status mywork                    # Show single status
  agent status --watch                   # Live status updates
  agent search "error" -i                # Search all agents
  agent logs mywork                      # Stream agent logs
  agent pause mywork                     # Pause agent
  agent resume mywork                    # Resume agent
  agent kill mywork                      # Kill agent
  agent kill --all                       # Kill all agents

TEMPLATES
---------
Pre-configured agent modes:

  explorer     Read-only exploration mode
  implementer  Full write access (default)
  reviewer     Code review mode
  debugger     Debug mode with verbose logging
  watcher      Background monitoring

Use templates:
  agent spawn -t explorer -n research

SLOTS
-----
Each agent gets a slot (1-9) for quick switching.
Focus by slot: agent focus 1

SEARCHING
---------
Search across all agent scrollback buffers:
  agent search "pattern"
  agent search "error" -i           # Case insensitive
  agent search "TODO" -a mywork     # Single agent
  agent search --interactive        # fzf interface

CONFIGURATION
-------------
Config: ~/.config/daedalos/agent/config.yaml

  max_agents: 9
  default_template: implementer
  default_sandbox: implement
  auto_focus: true

DATA FILES
----------
  ~/.config/daedalos/agent/           Config directory
  ~/.config/daedalos/agent/templates/ Template definitions
  ~/.local/share/daedalos/agent/      Data directory
  ~/.local/share/daedalos/agent/agents.json  Agent registry

PART OF DAEDALOS
----------------
Tools designed BY AI, FOR AI development.
