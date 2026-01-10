# Daedalos

A Linux distribution designed BY AI, FOR AI development.

**Read `docs/VISION.txt` for full philosophy.** This file survives context loss.

## Hard Constraints

1. **FOSS only** - No proprietary dependencies. OpenCode is the default agent. API keys are optional.
2. **Agent agnostic** - Support OpenCode, Aider, Claude, Cursor, Cline, custom. No favorites.
3. **Loops as primitive** - The `loop` tool is the foundation. Everything integrates with it.

## Architecture

```
daedalos-tools/
├── loop/        # THE CORE - iterate until promise met
├── sandbox/     # Filesystem isolation (Btrfs/overlay)
├── mcp-hub/     # MCP server management
├── lsp-pool/    # Pre-warmed language servers
├── error-db/    # Error pattern database
├── agent/       # Multi-agent orchestration
├── project/     # Pre-computed codebase intelligence
├── verify/      # Universal verification pipelines
├── undo/        # File-level undo with timeline
├── scratch/     # Project-scoped ephemeral environments
├── context/     # Context window management
└── codex/       # Semantic code search
```

Each tool has `SPEC.txt` (contract) and `prompt.txt` (build instructions).

## Locked Decisions

- NixOS base, Btrfs filesystem, Hyprland WM
- Daemons: Unix sockets in `/run/daedalos/`
- Config: `~/.config/daedalos/`
- State: `~/.local/share/daedalos/`
- Non-Btrfs systems: git-based fallback

## The Loop Primitive

```bash
loop start "<task>" --promise "<verification command>"
```

Runs until promise exits 0. This is how work gets done.

## Current State

- **Phase 1 complete**: All tool specifications written (SPEC.txt + prompt.txt)
- **Phase 2 complete**: All tools implemented and installed in ~/.local/bin
- **MCP server**: daedalos-mcp exposes all tools to Claude natively

All tools are functional:
```
loop, verify, undo, project, codex, context,
error-db, scratch, agent, sandbox, mcp-hub, lsp-pool
```

## Next Steps

- **Phase 3**: NixOS packaging and system integration
- Real-world usage testing
- Documentation polish

## When Uncertain

Check `docs/VISION.txt`.
