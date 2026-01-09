context - Context Window Management for Claude Code
===================================================

Monitor and manage Claude's context window usage to optimize long sessions.

Commands
--------

  context status          Show current context budget
  context breakdown       Detailed breakdown by category
  context files           Show files currently in context
  context compact         Suggest context compaction strategies
  context checkpoint      Save context state
  context list-checkpoints List saved checkpoints
  context restore         Show checkpoint contents
  context full            Complete context report

Installation
------------

  ./install.sh

Or manually:

  pip install -e .

For better token counting accuracy:

  pip install -e ".[tiktoken]"

Usage
-----

  # Check context usage
  context status

  # See what's using the most context
  context breakdown

  # Get suggestions to reduce context
  context compact

  # Save checkpoint before risky operation
  context checkpoint pre-refactor

Token Estimation
----------------

Uses tiktoken for accurate counts when available.
Falls back to ~4 characters per token estimate otherwise.
