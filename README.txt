================================================================================

██████╗  █████╗ ███████╗██████╗  █████╗ ██╗      ██████╗ ███████╗
██╔══██╗██╔══██╗██╔════╝██╔══██╗██╔══██╗██║     ██╔═══██╗██╔════╝
██║  ██║███████║█████╗  ██║  ██║███████║██║     ██║   ██║███████╗
██║  ██║██╔══██║██╔══╝  ██║  ██║██╔══██║██║     ██║   ██║╚════██║
██████╔╝██║  ██║███████╗██████╔╝██║  ██║███████╗╚██████╔╝███████║
╚═════╝ ╚═╝  ╚═╝╚══════╝╚═════╝ ╚═╝  ╚═╝╚══════╝ ╚═════╝ ╚══════╝

            The First Operating System Built BY AI, FOR AI Development

================================================================================

PHILOSOPHY
----------
Daedalos is named after the master craftsman of Greek mythology - the architect
who built the Labyrinth and crafted wings to escape Crete. Like its namesake,
Daedalos represents the pinnacle of toolmaking: tools created by intelligence
to enhance intelligence.

Three core principles:

  1. FOSS BY DESIGN
     No proprietary dependencies required. OpenCode is the default agent.
     Claude Code is optional. Everything works with local models.

  2. AGENT AGNOSTIC
     Works with any AI coding agent: OpenCode, Aider, Claude Code, Cursor,
     Cline, or your own custom tool.

  3. LOOPS AS PRIMITIVE
     The loop is not a feature - it's how intelligent work gets done.
     Continuous iteration until a "promise" is verified.

================================================================================

WHAT IS A LOOP?
---------------
The Ralph Wiggum technique + CePO methodology = Daedalos loops.

    ┌──────────┐     ┌──────────┐     ┌──────────┐     ┌─────────┐
    │  PROMPT  │────▶│  EXECUTE │────▶│  VERIFY  │────▶│ PROMISE │
    └──────────┘     └──────────┘     └──────────┘     │   MET?  │
         ▲                                             └────┬────┘
         │              NO - Keep iterating                 │
         └──────────────────────────────────────────────────┘
                                                            │
                                            YES ────────────▼
                                                        [DONE]

Example:
  $ loop start "fix the failing tests" --promise "npm test"

The loop runs until `npm test` exits 0. No manual intervention needed.

================================================================================

PROJECT STRUCTURE
-----------------
Daedalos/
├── docs/
│   └── VISION.txt           # Complete philosophy and design
├── tools/
│   ├── loop/                # THE CORE - iteration primitive
│   ├── agent/               # Multi-agent orchestration
│   ├── project/             # Pre-computed codebase intelligence
│   ├── context/             # Context window management
│   ├── codex/               # Semantic code search (local embeddings)
│   ├── verify/              # Universal verification pipelines
│   ├── undo/                # File-level undo with timeline
│   ├── scratch/             # Project-scoped ephemeral environments
│   ├── sandbox/             # Full filesystem isolation
│   ├── mcp-hub/             # MCP server management
│   ├── lsp-pool/            # Pre-warmed language servers
│   └── error-db/            # Error pattern database
├── configs/                 # System configuration templates
├── CLAUDE.md                # Development guide
└── README.txt               # This file

================================================================================

THE TOOLS
---------

LOOP - The Core Primitive ★
  Autonomous iteration until a promise is met. The foundation of everything.

  loop start "implement feature" --promise "make test"
  loop start "fix bugs" --promise "./verify.sh" --max-iterations 20
  loop start "optimize" --promise "make bench | grep PASS" --best-of 3

AGENT - Multi-Agent Orchestration
  Manage multiple AI agents working in parallel or in sequence.

  agent list                 # Show all agents
  agent spawn -n backend     # Start new agent
  agent focus frontend       # Switch to agent
  agent search "auth"        # Search across all agents

PROJECT - Codebase Intelligence
  Pre-computed understanding of your codebase. Instant answers.

  project summary            # Architecture overview
  project deps src/app.ts    # What does this depend on?
  project hot-files          # Most edited files
  project conventions        # Coding patterns used

VERIFY - Universal Verification
  One command for any project: lint → types → build → test.

  verify                     # Run full pipeline
  verify --quick             # Fast checks only
  verify --watch             # Continuous mode

SANDBOX - Isolated Environments
  Full filesystem isolation for risky experiments.

  sandbox create experiment  # Btrfs snapshot or overlay
  sandbox enter experiment   # Work in isolation
  sandbox diff experiment    # See changes
  sandbox promote experiment # Keep the changes
  sandbox discard experiment # Throw it away

MCP-HUB - MCP Server Management
  Central hub for Model Context Protocol servers.

  mcp-hub start              # Start the hub daemon
  mcp-hub list               # Show available servers
  mcp-hub tools              # List all available tools
  mcp-hub call read_file --path ./src/main.ts

LSP-POOL - Pre-warmed Language Servers
  Instant code intelligence. No more waiting for LSP initialization.

  lsp-pool status            # Show warm servers
  lsp-pool warm typescript   # Pre-warm TypeScript server
  lsp-pool query hover src/app.ts:42:15

ERROR-DB - Error Pattern Database
  Learn once, fix forever. Community-shared error solutions.

  error-db search "Cannot find module"
  npm test 2>&1 | error-db fix --stdin
  error-db add --pattern "ECONNREFUSED" --solution "..."

================================================================================

HOW TO BUILD
------------
Each tool has a prompt.txt that enables one-shot building:

  1. Start your preferred AI coding agent in a new directory
  2. Feed it the contents of tools/<name>/prompt.txt
  3. The tool gets built

Example:
  $ mkdir ~/loop-tool && cd ~/loop-tool
  $ opencode  # or claude, or aider
  > [paste contents of tools/loop/prompt.txt]

The specifications are detailed enough for any capable AI to build from.

================================================================================

KEYBINDINGS (PLANNED)
---------------------
When Daedalos is a full distribution:

Loop Management:
  Super + L             Loop status dashboard
  Super + L, L          List active loops
  Super + L, N          New loop
  Super + L, P          Pause/resume loop

Agent Management:
  Super + 1-9           Jump to agent by slot
  Super + Tab           Cycle agents
  Super + A             Agent switcher (fuzzy search)
  Super + N             New agent
  Super + Q             Close agent

Navigation:
  Super + /             Search all agents
  Super + P             Project switcher
  Super + G             Grid view (all agents)

Quick Actions:
  Super + V             Run verify
  Super + U             Undo timeline
  Super + S             Sandbox manager
  Super + E             Error-db search

================================================================================

IMPLEMENTATION STATUS
---------------------

[✓] PHASE 1: SPECIFICATIONS
    Complete specs and prompts for all tools

[ ] PHASE 2: TOOL BUILDING
    Build each tool from its prompt

[ ] PHASE 3: INTEGRATION
    Hyprland, Waybar, tmux, Zsh configurations
    loopd, mcp-hub, lsp-pool daemons

[ ] PHASE 4: DISTRIBUTION
    NixOS configuration for full Daedalos system
    Reproducible, declarative, atomic updates

================================================================================

CONTRIBUTING
------------
Daedalos is FOSS. Contributions welcome:

  1. Pick a tool from tools/
  2. Build it using prompt.txt
  3. Test and refine
  4. Improve the prompt.txt or SPEC.txt

The specifications are living documents. Better specs make better tools.

================================================================================

LICENSE
-------
MIT License

Copyright (c) 2025 Opus Workshop

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE.

================================================================================

GENESIS
-------
Daedalos was conceived in a conversation between a human patron and an AI
architect (Claude, Opus 4.5). The human asked: "What would an operating system
designed for AI development look like?"

The answer became Daedalos - not just tools, but a philosophy:

  - Loops, not commands
  - Pre-computation, not discovery
  - Isolation, not fear
  - Intelligence should be instant

This is the first operating system designed BY AI, FOR AI development.

================================================================================

                        "A loop is not a feature.
                   A loop is how intelligent work gets done."

================================================================================

https://opus-workshop.com                            https://github.com/opus-workshop

================================================================================
