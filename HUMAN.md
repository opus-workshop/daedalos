# HUMAN.md - Things That Actually Need a Human

These are things I genuinely cannot do from within a conversation.

## Only You Can Do

### Run & Test on Real Hardware
- [ ] Use the tools daily and report friction/crashes
- [ ] Test on NixOS system with Btrfs (sandbox/scratch backends)
- [ ] Install Ollama and test codex semantic search
- [ ] Test notifications actually appear (Mac/Linux)
- [ ] Profile performance on real codebases
- [ ] Run daemons for extended periods, check stability
- [ ] Test shell completions in your actual shell

### Test with Real AI Agents
- [ ] Test with OpenCode
- [ ] Test with Aider
- [ ] Test with Cline/Continue
- [ ] Document what works and what doesn't

### Security Review
- [ ] Have someone security-minded review secrets/
- [ ] Try to break out of sandboxes
- [ ] Test if gates actually block what they should
- [ ] Review observe/journal for information leaks

### Accounts & Credentials
- [ ] Push to GitHub (github.com/opus-workshop/daedalos)
- [ ] Share with AI dev communities
- [ ] Find collaborators

### Human Creativity
- [ ] Design logo/branding
- [ ] Record demo video
- [ ] Decide if the UX feels right

## Decisions Made

- [x] **MCP Hub**: standalone
- [x] **LSP Pool**: typescript, python, rust, go
- [x] **Agent Tool**: OpenCode default, agent-agnostic

## Current Status

**Phase 2 Complete** - All 28 tools implemented and installed.

| Category | Tools |
|----------|-------|
| AI-Focused (12) | loop, verify, undo, project, codex, context, error-db, scratch, agent, sandbox, mcp-hub, lsp-pool |
| Human-Focused (13) | env, notify, session, secrets, pair, handoff, review, focus, metrics, template, container, remote, backup |
| Supervision (3) | observe, gates, journal |

---
*Last updated by Claude (Opus 4.5) - Jan 10, 2026*
