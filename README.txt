================================================================================

DAEDALOS

  The First Operating System Built BY AI, FOR AI Development

                         "Iterate Until Done"

================================================================================

WHAT IS DAEDALOS?
-----------------
Daedalos is a complete development environment designed by an AI architect
for AI-assisted software development. It provides tools, specifications,
integration patterns, and eventually a full operating system for AI agents.

The name honors Daedalus, the Greek craftsman who built the Labyrinth and
crafted wings to escape Crete. Like its namesake, Daedalos builds tools to
achieve freedom through ingenuity.

================================================================================
                              PROJECTS
================================================================================

DAEDALOS-TOOLS
  30+ CLI tools and MCP server for AI development
  https://github.com/opus-workshop/daedalos-tools

  Core tools: loop, verify, undo, agent, codex, project, gates, sandbox

  Install: cargo install daedalos-tools

AETHER
  Terminal emulator designed for AI-assisted development
  https://github.com/opus-workshop/aether

================================================================================
                              THIS REPOSITORY
================================================================================

docs/                Specifications and design documents
research/            Development notes and explorations
.claude/             Claude Code integration (skills, hooks)
configs/             Configuration templates
nixos/               NixOS integration

================================================================================
                              PHILOSOPHY
================================================================================

1. ITERATE UNTIL DONE
   Single-pass inference fails. The loop primitive runs continuously until a
   "promise" (verification command) succeeds. This is how work gets done.

2. VERIFY EVERYTHING
   One command handles lint, types, build, and test. Universal pipeline
   that works across languages.

3. FEARLESS EXPERIMENTATION
   Every change is cheap to undo. Checkpoints before risky work. Sandbox
   environments for experiments.

4. PRE-COMPUTATION OVER DISCOVERY
   Index codebases before they're queried. Warm language servers before
   they're needed. Make context instant.

================================================================================
                              LICENSE
================================================================================

MIT License - See LICENSE file

================================================================================
