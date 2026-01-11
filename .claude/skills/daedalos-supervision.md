---
name: daedalos-supervision
description: Use when working with varying levels of human oversight - check gates and respect autonomy limits
---

# Daedalos Supervision - Respecting Autonomy Levels

## Overview

Daedalos supports any level of human-AI supervision. Before taking significant actions, check gates and respect the configured autonomy level.

## Supervision Levels

```
AUTONOMOUS ─────────────────────────────────────── MANUAL
     │                                                │
     AI runs freely              Human drives, AI helps
```

| Level | Description | When Used |
|-------|-------------|-----------|
| `autonomous` | AI runs freely, minimal gates | Trusted tasks, low risk |
| `supervised` | AI runs, human notified | Normal development |
| `collaborative` | AI proposes, human approves major actions | Important changes |
| `assisted` | Human drives, AI suggests | Learning, sensitive code |
| `manual` | AI only responds to commands | High-security contexts |

## Check Current Level

```bash
gates level
```

## Gate Actions

For each type of action, there's a gate with one of:

| Action | Meaning |
|--------|---------|
| `allow` | Proceed without asking |
| `notify` | Notify but don't block |
| `approve` | Require explicit approval |
| `deny` | Always deny |

## Before Significant Actions

Check if the action is allowed:

```bash
# Check specific gates
gates check file_delete
gates check git_push
gates check loop_start
gates check agent_spawn
```

## Built-in Gates

| Gate | What It Controls |
|------|------------------|
| `file_create` | Creating new files |
| `file_modify` | Modifying existing files |
| `file_delete` | Deleting files |
| `git_commit` | Making git commits |
| `git_push` | Pushing to remote |
| `git_force_push` | Force pushing |
| `loop_start` | Starting iteration loops |
| `agent_spawn` | Spawning new agents |
| `shell_command` | Running shell commands |
| `sensitive_file` | Modifying secrets, env, keys |

## Sensitive Paths

These paths always require extra scrutiny:

- `*.env` - Environment files
- `**/secrets/**` - Secret directories
- `**/*.key` - Key files
- `**/*.pem` - Certificates
- `**/credentials*` - Credential files

## Observability

Let humans see what's happening:

```bash
# Real-time dashboard
observe

# Activity journal
journal show

# What happened in last hour?
journal

# Search events
journal search "git push"
```

## Red Flags - STOP These Patterns

| Thought | Reality |
|---------|---------|
| "It's fine, I know what I'm doing" | Check gates anyway. Autonomy level may have changed. |
| "This is just a small change" | Small changes to sensitive files still need approval. |
| "I'll notify after" | Check gates BEFORE the action, not after. |
| "The human trusts me" | Trust is configured via gates. Respect the configuration. |

## Example: Respecting Gates

```bash
# Before deleting files
if gates check file_delete; then
    rm -rf old_code/
else
    echo "File deletion not allowed - ask human"
fi

# Before pushing
if gates check git_push; then
    git push origin main
else
    echo "Git push requires approval"
fi
```

## Changing Supervision Level

Only humans change this:

```bash
# Human runs these commands, not AI
gates level supervised
gates set git_push approve
gates set file_delete deny
```

## Integration

```bash
# Before starting work
gates level  # Know what level you're at
gates config # See all gates

# During work
observe      # Keep dashboard open

# After work
journal      # Review what happened
```

## Philosophy

"Daedalos supports any supervision level. Respect the one configured."

The human decides how much autonomy to grant. Your job is to respect that decision, check gates before significant actions, and keep the human informed through observe/journal.
