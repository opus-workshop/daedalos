================================================================================

DAEDALOS - AI Development Tools

  The iterate-until-done toolkit for AI-assisted software development

================================================================================

WHAT IS DAEDALOS?
-----------------
A Rust workspace providing 30+ MCP tools for AI-assisted development:
loop, verify, undo, agent, codex, project, gates, sandbox, and more.

The name honors Daedalus, the Greek craftsman who built the Labyrinth.
Like its namesake, Daedalos builds tools to achieve freedom through ingenuity.

================================================================================
                              STRUCTURE
================================================================================

crates/              Rust tool implementations
  daedalos-core/     Shared library (config, paths, daemon support)
  daedalos-mcp/      MCP server exposing all tools
  loop/              Iterate until promise met
  verify/            Universal project verification
  undo/              File-level time machine
  agent/             Multi-agent orchestration
  codex/             Semantic code search
  project/           Codebase intelligence
  gates/             Supervision and autonomy levels
  sandbox/           Filesystem isolation
  ... and 20+ more

docs/                Specifications and plans
research/            Development notes
.claude/             Claude Code integration (skills, hooks)

================================================================================
                              QUICK START
================================================================================

Build all tools:
    cargo build --release

Run the MCP server:
    cargo run --release -p daedalos-mcp

Use individual tools:
    cargo run -p loop -- start "fix tests" --promise "cargo test"
    cargo run -p verify
    cargo run -p undo -- timeline

================================================================================
                              CORE TOOLS
================================================================================

LOOP        Iterate until a verification command succeeds
VERIFY      Universal lint -> types -> build -> test pipeline
UNDO        File changes with SQLite timeline and checkpoints
AGENT       Spawn and coordinate multiple Claude Code agents
CODEX       Semantic code search with local embeddings
PROJECT     Pre-computed codebase structure and symbols
GATES       Supervision levels controlling AI autonomy
SANDBOX     Isolated environments for risky experiments
SPEC        Rich specifications with intent and anti-patterns
EVOLVE      Analyze code intent and suggest improvements
RESOLVE     Gather context to answer questions autonomously

================================================================================
                              RELATED PROJECTS
================================================================================

Aether      Terminal emulator for AI development
            https://github.com/opus-workshop/aether

================================================================================
                              LICENSE
================================================================================

MIT License - See LICENSE file

================================================================================
