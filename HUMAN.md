# HUMAN.md - Things I Need From You

## For Testing on Real Hardware

- [ ] Access to a NixOS system with Btrfs to test sandbox/scratch Btrfs backends
- [ ] Ollama installed for codex semantic search (or confirm I should skip this dependency)

## Decisions Needed

- [x] **MCP Hub**: standalone
- [x] **LSP Pool**: typescript, python, rust, go
- [x] **Agent Tool**: OpenCode default, agent-agnostic

## When You Have Time

- [ ] Review the completed tools and let me know if anything needs adjustment
- [ ] Test the tools in your actual workflow

## Current Status

**Phase 2 Complete** - All tools implemented and installed.

### AI-Focused Tools (12)
loop, verify, undo, project, codex, context, error-db, scratch, agent, sandbox, mcp-hub, lsp-pool

### Human-Focused Tools (13)
env, notify, session, secrets, pair, handoff, review, focus, metrics, template, container, remote, backup

### Supervision Tools (3)
observe, gates, journal

**Total: 28 tools** installed in ~/.local/bin

### Next Steps
- Phase 3: NixOS packaging
- Real-world testing
- Documentation polish

---
*Last updated by Claude Jan 10, 2026
