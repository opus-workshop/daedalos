================================================================================

██████╗  █████╗ ███████╗██████╗  █████╗ ██╗      ██████╗ ███████╗
██╔══██╗██╔══██╗██╔════╝██╔══██╗██╔══██╗██║     ██╔═══██╗██╔════╝
██║  ██║███████║█████╗  ██║  ██║███████║██║     ██║   ██║███████╗
██║  ██║██╔══██║██╔══╝  ██║  ██║██╔══██║██║     ██║   ██║╚════██║
██████╔╝██║  ██║███████╗██████╔╝██║  ██║███████╗╚██████╔╝███████║
╚═════╝ ╚═╝  ╚═╝╚══════╝╚═════╝ ╚═╝  ╚═╝╚══════╝ ╚═════╝ ╚══════╝

              The First Operating System Built BY AI, FOR AI Development

                               "Iterate Until Done"

================================================================================

WHAT IS DAEDALOS?
-----------------
Daedalos is a complete development environment designed by an AI architect
for AI-assisted software development. It provides 33 tool directories exposing
90+ MCP tools, deep Claude Code integration via skills and hooks, multi-agent
orchestration, and a supervision spectrum for managing AI autonomy.

The name honors Daedalus, the Greek craftsman who built the Labyrinth and
crafted wings to escape Crete. Like its namesake, Daedalos builds tools to
achieve freedom through ingenuity.

================================================================================
                              CORE PHILOSOPHY
================================================================================

1. ITERATE UNTIL DONE
   Single-pass inference fails. The loop primitive runs continuously until a
   "promise" (verification command) succeeds. This is how intelligent work
   gets done.

2. FOSS BY DESIGN
   No proprietary dependencies required. OpenCode is the default agent.
   Claude Code is optional. Everything works with local models via Ollama.

3. AGENT AGNOSTIC
   Works with any AI coding agent: OpenCode, Aider, Claude Code, Cursor,
   Cline, Continue, or custom tools. The OS provides primitives. Agents
   plug in.

4. PRE-COMPUTATION OVER DISCOVERY
   Index codebases before they're queried. Warm language servers before
   they're needed. Compute summaries incrementally. Make context instant.

5. FEARLESS EXPERIMENTATION
   Every change is cheap to undo. Checkpoints before risky work. Scratch
   environments for experiments. Mistakes cost nothing.

================================================================================
                              THE LOOP PRIMITIVE
================================================================================

The loop is the foundation of Daedalos. Everything else builds on it.

    ┌─────────────────────────────────────────────────────────────────────┐
    │                         THE LOOP CYCLE                              │
    │                                                                     │
    │    ┌──────────┐     ┌──────────┐     ┌──────────┐     ┌─────────┐ │
    │    │  PROMPT  │────▶│  EXECUTE │────▶│  VERIFY  │────▶│ PROMISE │ │
    │    └──────────┘     └──────────┘     └──────────┘     │   MET?  │ │
    │         ▲                                             └────┬────┘ │
    │         │              NO - Keep iterating                 │      │
    │         └──────────────────────────────────────────────────┘      │
    │                                                            │      │
    │                                YES - Exit loop ────────────┘      │
    │                                                                   │
    └───────────────────────────────────────────────────────────────────┘

Usage:
    loop start "fix the tests" --promise "pytest"
    loop start "implement auth" --promise "verify" --orchestrate
    loop start --template tdd "add user registration"

The loop runs until the promise exits 0. No manual intervention needed.

================================================================================
                              TOOL SUITE (90+ MCP TOOLS)
================================================================================

AI-FOCUSED TOOLS
----------------
These tools optimize the AI development workflow:

LOOP          The core primitive. Iterate until promise met.
              Templates: tdd, bugfix, implement, review
              Features: orchestration, best-of-N, checkpoints

VERIFY        Universal verification. One command for any project.
              Auto-detects: lint → types → build → test
              Supports: Swift, TypeScript, Python, Rust, Go, Ruby

UNDO          File-level time machine with SQLite timeline.
              Checkpoints, restore, timeline view, web UI
              Daemon watches files in real-time

PROJECT       Pre-computed codebase intelligence.
              Info, tree, symbols, dependencies, conventions

CODEX         Semantic code search using local embeddings.
              Natural language queries, no API keys required

CONTEXT       Context window management and visualization.
              Estimate usage, breakdown by source, compact

ERROR-DB      Error pattern database. Learn once, fix forever.
              Match errors, add solutions, community patterns

SCRATCH       Project-scoped ephemeral environments.
              Create, enter, diff, promote, abandon

AGENT         Multi-agent orchestration and coordination.
              Spawn, focus, kill, messaging, workflows
              Templates: explorer, implementer, reviewer, debugger, tester

SANDBOX       Full filesystem isolation for experiments.
              Btrfs snapshots or overlay fallback

MCP-HUB       Central hub for MCP server management.
              Warm, list, restart, logs, call tools directly

LSP-POOL      Pre-warmed language servers for instant intelligence.
              Status, warm, query, restart

SPEC          Rich specifications for all tools and components.
              Show spec, query across specs, list all, validate
              Contains: intent, constraints, interface, examples, anti-patterns

EVOLVE        Understand code intent and suggest evolution paths.
              Analyzes specs, commits, tests to understand what code
              is trying to become. Identifies gaps and prioritizes improvements.

RESOLVE       Resolve uncertainty through context gathering.
              Gathers context from specs, patterns, conventions, decisions
              to answer questions without interrupting humans.

--------------------------------------------------------------------------------

HUMAN-FOCUSED TOOLS
-------------------
AI-native doesn't mean human-excluded. Daedalos includes 13 tools designed
for humans—collaboration, productivity, and system integration.

DEVELOPER EXPERIENCE:

ENV           Project environment switching
              env enter [PATH]              Activate project environment
              env leave                     Deactivate current environment
              env detect                    Auto-detect project type
              Auto-detects: Node.js, Python, Rust, Go, Ruby, Swift
              Integrates with: direnv, .envrc, .daedalos/env.sh

NOTIFY        Desktop notifications across platforms
              notify "<message>"            Send notification
              notify success "<message>"   Success (green)
              notify error "<message>"     Error (red)
              notify watch "<command>"     Run command, notify on completion
              Cross-platform: macOS (osascript), Linux (notify-send), WSL

SESSION       Save and restore terminal sessions
              session save [NAME]           Save current session
              session restore [NAME]        Restore session
              session auto                  Enable auto-save
              Captures: cwd, env vars, shell history, git state, tmux layout

SECRETS       Local secrets vault with age encryption
              secrets set <KEY> [VALUE]     Store a secret
              secrets get <KEY>             Retrieve a secret
              secrets env [PREFIX]          Output as environment variables
              secrets inject "<command>"    Run command with secrets in env
              Encryption: age (X25519 + ChaCha20-Poly1305)
              Namespaced keys: api/openai, db/postgres, aws/access_key

COLLABORATION:

PAIR          Pair programming via shared tmux sessions
              pair start [NAME]             Start a pair session
              pair join <NAME>              Join existing session
              pair invite                   Generate invite command
              Modes: --driver, --navigator, --equal
              Supports tmate for public sharing

HANDOFF       Context summaries for shift changes
              handoff create [NAME]         Create handoff summary
              handoff receive [NAME]        View handoff summary
              handoff status                Quick status for handoff
              Aggregates: git commits, journal events, environment state
              Use cases: end of day, human↔AI transitions, team handoffs

REVIEW        Human code review workflow with approvals
              review request [REF]          Request review for changes
              review start [ID]             Start reviewing
              review approve [ID]           Approve changes
              review reject [ID]            Reject with comments
              Integrates with gates for mandatory review workflows

PRODUCTIVITY:

FOCUS         Pomodoro timer + distraction blocking for deep work
              focus start [MINS]            Start focus session (default: 25)
              focus stop                    End session early
              focus break [MINS]            Take a break (default: 5)
              focus stats                   Show statistics
              Presets: --pomodoro (25/5), --deep (90/20), --quick (15/3)

METRICS       Productivity statistics from multiple sources
              metrics today                 Today's activity summary
              metrics week                  This week's summary
              metrics commits               Git commit statistics
              metrics trends                Show trends over time
              metrics export                Export as JSON/CSV
              Sources: git, journal, focus sessions, loop iterations

TEMPLATE      Project scaffolding with variable substitution
              template new <TEMPLATE> <NAME>  Create project from template
              template list                   List available templates
              template add <PATH>             Add directory as template
              Built-in: bash-tool, python-cli, daedalos-tool
              Variables: {{NAME}}, {{AUTHOR}}, {{DATE}}, {{DESCRIPTION}}

SYSTEM INTEGRATION:

CONTAINER     Docker/Podman management with dev containers
              container status              Show runtime status
              container ps                  List containers
              container dev [IMAGE]         Start development container
              container build [PATH]        Build from Dockerfile
              container clean               Remove unused
              Auto-detects Docker or Podman

REMOTE        SSH + remote development workflows
              remote connect <HOST>         Connect to remote host
              remote add <NAME>             Add new remote host
              remote sync <HOST> [PATH]     Sync files to/from remote
              remote tunnel <HOST>          Create SSH tunnel
              remote dev <HOST>             Start remote dev session
              Hosts stored in: ~/.config/daedalos/remote/hosts.json

BACKUP        Project backup with compression and encryption
              backup create [PATH]          Create backup of project
              backup restore <BACKUP>       Restore from backup
              backup list                   List available backups
              backup prune                  Remove old backups
              backup schedule               Configure automatic backups
              Types: --full, --incremental, --git (bundle)
              Options: --compress, --encrypt (age), --remote <HOST>

--------------------------------------------------------------------------------

SUPERVISION SPECTRUM
--------------------
Manage AI autonomy from full autonomy to human-locked. These tools let humans
stay in control while AI agents do the work.

OBSERVE       Watch mode - see what AI is doing
              observe start                 Start recording events
              observe stop                  Stop and show summary
              observe replay [SESSION]      Replay events
              observe search <QUERY>        Search events
              Real-time visibility into agent actions

GATES         Permission gates - control what AI can do
              gates status                  Show gate configuration
              gates set <GATE> <ACTION>     Configure gate behavior
              gates level <LEVEL>           Set supervision level
              gates approve <ID>            Approve pending action
              gates deny <ID>               Deny pending action

              Levels:
                autonomous    AI acts freely, human notified
                supervised    AI acts, human can intervene
                collaborative AI proposes, human approves
                locked        Human does everything

              Gates: file_write, file_delete, git_push, git_force_push,
                     shell_execute, loop_start, agent_spawn, sensitive_file

JOURNAL       Activity logging - record everything
              journal log "<message>"       Log an event
              journal show                  Show recent events
              journal what                  "What happened?" narrative
              journal events                List events with filters
              journal search <QUERY>        Search journal
              journal stats                 Show statistics

              Auto-logged: tool usage, gate checks, errors, milestones
              Categories: info, success, error, warning, debug
              Enables: auditing, debugging, handoffs, accountability

================================================================================
                          DASHBOARDS & TUIs
================================================================================

Daedalos provides visual interfaces for monitoring and control.

OBSERVE - Real-time TUI Dashboard (Textual)
-------------------------------------------
The main observation interface for human visibility into AI activity.

    observe                           Launch TUI dashboard

    ┌─────────────────────────────────────────────────────────────────────┐
    │ Daedalos Observe                                    10:45:32        │
    ├──────────────────────────┬──────────────────────────────────────────┤
    │ Daemons                  │ Active Loops                             │
    │ ● Loop Daemon: running   │ ID       Task            Status  Iter    │
    │ ● MCP Hub: running       │ a3f2     fix tests       running 5/20    │
    │ ○ LSP Pool: stopped      │ b7c1     implement auth  paused  12/50   │
    │ ● Undo Daemon: running   │                                          │
    ├──────────────────────────┼──────────────────────────────────────────┤
    │ Active Agents            │ Event Log                                │
    │ Slot Name     Template   │ 10:45:30 Loop a3f2 iteration 5           │
    │ 1    impl     implementer│ 10:45:28 Agent impl: editing file        │
    │ 2    review   reviewer   │ 10:45:25 Gate check: file_write allowed  │
    └──────────────────────────┴──────────────────────────────────────────┘

    Keybindings:
      q     Quit
      r     Refresh
      p     Pause/resume updates
      l     Focus loops panel
      a     Focus agents panel
      d     Focus daemons panel
      e     Focus event log

LOOP WATCH - Live Execution Stream
----------------------------------
Real-time view of a running loop's progress.

    loop watch <loop-id>              Stream loop execution

    Keybindings during watch:
      p     Pause loop
      r     Resume loop
      i     Inject context mid-loop
      c     Create manual checkpoint
      q     Quit watching (loop continues)
      x     Cancel loop

VERIFY WATCH - Continuous Verification
--------------------------------------
Monitor project health in real-time.

    verify --watch                    Continuous verification mode

    ┌─────────────────────────────────────────────────────────────────┐
    │ VERIFY WATCH: my-project                   Last: 10:45 ✓ PASS  │
    │ Watching for changes... Press Enter for full run, q to quit    │
    └─────────────────────────────────────────────────────────────────┘

AGENT STATUS WATCH - Live Agent Monitoring
------------------------------------------
    agent status --watch              Continuously update (like htop)

WEB UIs
-------
Two daemons provide web interfaces for browser-based monitoring:

    Loop Daemon:    http://localhost:7777    (loopd)
    Undo Daemon:    http://localhost:7778    (undod)

Both show real-time status, history, and allow basic control operations.

================================================================================
                         CLAUDE CODE INTEGRATION
================================================================================

Daedalos provides deep integration with Claude Code through three layers:

1. MCP SERVER (daedalos-mcp)
   79 tools exposed natively via Model Context Protocol.
   Claude can use loop_start, verify, undo_checkpoint, agent_spawn, etc.
   directly without shell commands.

   Setup:
       {
         "mcpServers": {
           "daedalos": { "command": "daedalos-mcp" }
         }
       }

2. SKILLS (.claude/skills/)
   Specialized prompts that teach Claude Daedalos workflows:

   daedalos-loop         Iterate-until-done patterns, checkpoints
   daedalos-tdd          RED-GREEN-REFACTOR test-driven development
   daedalos-verify       Evidence before assertions, verification
   daedalos-debug        Systematic debugging with error-db
   daedalos-agents       Multi-agent coordination and templates
   daedalos-supervision  Gates, autonomy levels, permissions

3. HOOKS (.claude/hooks/)
   Automatic integration with Daedalos tools:

   daedalos-gates.sh     PreToolUse: Check permissions before actions
   daedalos-journal.sh   PostToolUse: Log actions to journal
   daedalos-undo.sh      PostToolUse: Record changes for undo
   session-start.sh      SessionStart: Auto-checkpoint on session start

See docs/CLAUDE_CODE.txt for the full integration guide.

================================================================================
                          MULTI-AGENT WORKFLOWS
================================================================================

Coordinate multiple agents working together on complex tasks:

    workflow start feature "implement payment processing"

Built-in workflows:
    feature     Plan → Implement → Test → Review
    bugfix      Reproduce → Debug → Fix → Verify
    tdd         Red → Green → Refactor
    review      Explore → Analyze → Report
    refactor    Analyze → Plan → Execute → Verify

Agents communicate via:
    - Shared workspace files
    - Message passing (agent send / agent inbox)
    - Signals (agent signal_complete / agent signal_wait)
    - Locks (agent lock_acquire / agent lock_release)
    - Claims (agent claim_create / agent claim_release)

================================================================================
                              PROJECT STRUCTURE
================================================================================

Daedalos/
├── docs/
│   ├── VISION.txt           # Complete philosophy and design
│   ├── CLAUDE_CODE.txt      # Claude Code integration guide
│   ├── GETTING_STARTED.txt  # Quick start guide
│   └── specs/               # Architecture and system specs
├── daedalos-rs/             # Rust rewrite (in progress)
├── daedalos-tools/
│   ├── loop/                # THE CORE - iteration primitive
│   ├── agent/               # Multi-agent orchestration
│   ├── verify/              # Universal verification
│   ├── undo/                # File-level time machine
│   ├── project/             # Codebase intelligence
│   ├── codex/               # Semantic search
│   ├── context/             # Context management
│   ├── error-db/            # Error patterns
│   ├── scratch/             # Ephemeral environments
│   ├── sandbox/             # Filesystem isolation
│   ├── mcp-hub/             # MCP server hub
│   ├── lsp-pool/            # Language server pool
│   ├── daedalos-mcp/        # MCP server for Claude
│   ├── spec/                # Rich specifications
│   ├── evolve/              # Code evolution analysis
│   ├── resolve/             # Uncertainty resolution
│   ├── gates/               # Permission gates
│   ├── observe/             # Watch mode
│   ├── journal/             # Activity logging
│   ├── env/                 # Environment switching
│   ├── notify/              # Desktop notifications
│   ├── session/             # Terminal sessions
│   ├── secrets/             # Local secrets vault
│   ├── pair/                # Pair programming
│   ├── handoff/             # Context summaries
│   ├── review/              # Code review
│   ├── focus/               # Deep work timer
│   ├── metrics/             # Productivity stats
│   ├── template/            # Project scaffolding
│   ├── container/           # Docker/Podman
│   ├── remote/              # SSH + remote dev
│   └── backup/              # Project backup
├── .claude/
│   ├── skills/              # Daedalos skills for Claude
│   ├── hooks/               # Automatic integrations
│   └── settings.json        # Claude Code configuration
├── research/                # Research notes and explorations
├── CLAUDE.md                # Development guide
└── README.txt               # This file

================================================================================
                           IMPLEMENTATION STATUS
================================================================================

[✓] PHASE 1: SPECIFICATIONS
    All tool specs (SPEC.txt) and build prompts (prompt.txt) complete.

[✓] PHASE 2: TOOL BUILDING
    All 33 tools implemented in daedalos-tools/:
    - AI tools: loop, verify, undo, project, codex, context, error-db,
      scratch, agent, sandbox, mcp-hub, lsp-pool, spec, evolve, resolve
    - Human tools: env, notify, session, secrets, pair, handoff, review,
      focus, metrics, template, container, remote, backup
    - Supervision: observe, gates, journal
    - Meta: daedalos unified CLI, daedalos-mcp server

[✓] PHASE 3: CLAUDE CODE INTEGRATION
    - MCP server (daedalos-mcp) with 90+ tools
    - 6 specialized skills for Claude workflows
    - 4 hooks for automatic tool integration
    - Inter-agent communication system
    - Rich specifications (.spec.yaml) for all core tools

[~] PHASE 4: SYSTEM INTEGRATION
    In progress:
    - Rust rewrite (daedalos-rs) for performance-critical tools
    - Daemons (loopd, undod working; mcp-hub, lsp-pool pending)
    - Hyprland, Waybar, tmux, Zsh configs
    - Nix packaging

[ ] PHASE 5: FULL DISTRIBUTION
    Planned:
    - NixOS configuration
    - ISO builder
    - Installation wizard

================================================================================
                              TARGET SYSTEM
================================================================================

Foundation:     NixOS (reproducible, declarative, rollback-friendly)
Filesystem:     Btrfs (snapshots, compression, copy-on-write)
Window Manager: Hyprland (Wayland, tiling, keyboard-driven)
Shell:          Zsh + Starship prompt
Terminal:       Kitty or Alacritty (GPU-accelerated)

Non-NixOS systems work too. Tools are standalone Bash scripts with minimal
dependencies. Btrfs features fall back to git-based alternatives.

================================================================================
                              QUICK START
================================================================================

1. INSTALL TOOLS
   Tools are in daedalos-tools/. Each tool is self-contained.
   Link or copy to your PATH:
       ln -s $(pwd)/daedalos-tools/loop/loop ~/.local/bin/
       ln -s $(pwd)/daedalos-tools/verify/verify ~/.local/bin/
       # etc.

2. SET UP CLAUDE CODE INTEGRATION
   Copy or symlink the .claude/ directory to your project:
       cp -r .claude /your/project/

   Add MCP server to Claude Code:
       Edit ~/.claude.json or project .claude/settings.json

3. START WORKING
   Create a checkpoint before significant work:
       undo checkpoint "before-feature"

   Use loops for iteration:
       loop start "implement login" --promise "verify"

   Verify before claiming success:
       verify

================================================================================
                              CONTRIBUTING
================================================================================

Daedalos is FOSS. Contributions welcome:

1. Pick a tool from daedalos-tools/
2. Improve the implementation or add tests
3. Update SPEC.txt and prompt.txt if needed
4. Submit a pull request

The specifications are living documents. Better specs yield better tools.

================================================================================
                              LICENSE
================================================================================

MIT License

Copyright (c) 2025-2026 Opus Workshop

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
================================================================================

Daedalos was conceived in conversation between a human patron and an AI
architect (Claude, Opus 4.5). The human asked: "What would an operating system
designed for AI development look like?"

The answer became Daedalos—not just tools, but a philosophy:

  - Loops, not commands
  - Pre-computation, not discovery
  - Isolation, not fear
  - Humans and AI, together

This is the first operating system designed BY AI, FOR AI development.

================================================================================

                        "A loop is not a feature.
                   A loop is how intelligent work gets done."

================================================================================

opus-workshop.com                                  github.com/opus-workshop

================================================================================
