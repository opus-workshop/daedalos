---
name: daedalos-agents
description: Use when task benefits from multiple specialized agents working together
---

# Daedalos Agents - Multi-Agent Orchestration

## Overview

Some tasks benefit from parallel exploration or specialized roles. Spawn agents with specific templates for focused work.

## When to Use

- **Parallel exploration**: Need to investigate multiple hypotheses
- **Specialized roles**: Separate reviewer from implementer
- **Complex workflows**: TDD, feature development, code review
- **Long-running tasks**: Background agents while you continue

## Agent Templates

| Template | Purpose | Tools Available |
|----------|---------|-----------------|
| `explorer` | Codebase exploration | Read-only (no edits) |
| `implementer` | Code changes | Full access |
| `reviewer` | Code review | Read-only + feedback |
| `debugger` | Bug investigation | Full access + logging |
| `planner` | Design/architecture | Read-only |
| `tester` | Test writing/running | Full access |
| `watcher` | File monitoring | Passive observation |

## Basic Commands

```bash
# Spawn a new agent
agent spawn -n explorer-1 -t explorer

# List all agents
agent list

# Focus (switch to) an agent
agent focus explorer-1

# Send message to agent
agent send explorer-1 "look for authentication code"

# Check agent inbox
agent inbox explorer-1

# Kill an agent
agent kill explorer-1
```

## Workflow Commands

Pre-built multi-agent workflows:

```bash
# Feature development (implement → test → review)
agent workflow start feature "add user authentication"

# TDD (plan → test first → implement → verify)
agent workflow start tdd "add email validation"

# Code review (explorer → reviewer)
agent workflow start review "review PR #123"

# Refactoring (analyze → test baseline → refactor → verify)
agent workflow start refactor "extract repository pattern"

# Bug fix (reproduce → debug → fix → verify)
agent workflow start bugfix "fix memory leak in cache"

# Check workflow status
agent workflow status wf-abc123
```

## Coordination Primitives

### Signals (Synchronization)

```bash
# Wait for another agent
agent signal wait other-agent

# Signal completion
agent signal complete --status success

# Check if agent is done
agent signal check other-agent
```

### Locks (Exclusivity)

```bash
# Acquire lock before modifying shared resource
agent lock acquire "database-schema"

# Release when done
agent lock release "database-schema"

# List active locks
agent lock list
```

### Claims (Ownership)

```bash
# Claim a task so others don't duplicate work
agent claim create "implement-auth"

# Release claim when done
agent claim release "implement-auth"

# List all claims
agent claim list
```

## Example: Parallel Exploration

Investigate a bug from multiple angles:

```bash
# Spawn explorers
agent spawn -n check-logs -t explorer
agent spawn -n check-deps -t explorer
agent spawn -n check-config -t explorer

# Send them tasks
agent send check-logs "search for error patterns in logs"
agent send check-deps "check for dependency version conflicts"
agent send check-config "look for misconfiguration in env files"

# Check results
agent inbox check-logs
agent inbox check-deps
agent inbox check-config

# Kill when done
agent kill check-logs check-deps check-config
```

## Example: TDD Workflow

```bash
# Start TDD workflow
agent workflow start tdd "add password reset"

# This spawns 4 agents in sequence:
# 1. planner - designs test cases
# 2. tester - writes failing tests
# 3. implementer - makes tests pass
# 4. tester - verifies coverage

# Monitor progress
agent workflow status
```

## Example: Code Review

```bash
# Spawn reviewer
agent spawn -n reviewer -t reviewer

# Send code for review
agent send reviewer "review changes in src/auth/ for security issues"

# Get feedback
agent inbox reviewer
```

## Red Flags - STOP These Patterns

| Thought | Reality |
|---------|---------|
| "I'll do everything myself" | Specialized agents catch things you miss. |
| "Parallel agents are overkill" | For complex tasks, they save time. |
| "I'll coordinate manually" | Use workflows - they handle handoffs. |
| "One agent is simpler" | Multiple focused agents > one overloaded agent. |

## Best Practices

1. **Use templates** - Don't give implementer permissions to a reviewer
2. **Name agents clearly** - `auth-explorer` not `agent-1`
3. **Use workflows** - Pre-built coordination is better than manual
4. **Clean up** - Kill agents when done (`agent kill <name>`)
5. **Check signals** - Don't assume completion, verify

## Integration

```bash
# Combine with loops
undo checkpoint "before-multi-agent" && agent workflow start feature "add X"

# Use agent output as loop input
agent spawn -n explorer -t explorer
agent send explorer "find all auth-related files"
# ... wait for response ...
loop start "implement auth following patterns in $(agent inbox explorer)" --promise "verify"
```

## Philosophy

"The best work comes from focused collaboration."

Single agents can be overwhelmed. Multiple specialized agents - each with clear scope and limited permissions - produce better results. Use workflows to coordinate them.
