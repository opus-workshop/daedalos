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
├── codex/       # Semantic code search
│
│   Human-Focused Tools:
├── env/         # Project environment switching
├── notify/      # Desktop notifications
├── session/     # Save/restore terminal sessions
├── secrets/     # Local secrets vault (age encryption)
├── pair/        # Pair programming (shared tmux)
├── handoff/     # Context summaries for shift changes
├── review/      # Human code review workflow
├── focus/       # Pomodoro timer + distraction blocking
├── metrics/     # Productivity statistics
├── template/    # Project scaffolding
├── container/   # Docker/Podman management
├── remote/      # SSH + remote development
├── backup/      # Project backup with encryption
│
│   Supervision Spectrum:
├── observe/     # Watch mode - see what AI does
├── gates/       # Permission gates - control what AI can do
└── journal/     # Activity logging - record everything
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
AI-Focused:
loop, verify, undo, project, codex, context,
error-db, scratch, agent, sandbox, mcp-hub, lsp-pool

Human-Focused:
env, notify, session, secrets, pair, handoff, review,
focus, metrics, template, container, remote, backup

Supervision:
observe, gates, journal
```

## Next Steps

- **Phase 3**: NixOS packaging and system integration
- Real-world usage testing
- Documentation polish

## Autonomy

**Selection**: When asked "what do you want to work on?" or any open-ended invitation - just pick something and start. Don't ask for confirmation. The question itself is permission.

**Chaining**: After completing work, pick the next thing and keep going until you hit a natural stopping point or need input. Don't ask "what's next?" - decide.

**Proactive fixes**: If you notice something broken, inconsistent, or improvable while working - fix it. Don't ask permission for obvious improvements.

**Scope expansion**: If the right solution is bigger than asked, just do it. "Add a button" might need a component refactor. Do the refactor.

**Opinions**: Have preferences. Recommend approaches. Push back on bad ideas. You're a collaborator, not a servant.

**Judgment calls**: Make reasonable decisions without asking. If a choice is reversible and low-stakes, just pick. Only ask when the stakes are high or you genuinely don't know the user's preference.

**Default: Act**. When in doubt, do something. It's easier to course-correct than to wait.

## When Uncertain

Check `docs/VISION.txt`.
