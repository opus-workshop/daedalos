project - Pre-computed Codebase Intelligence
============================================

Fast, cached project analysis for AI coding assistants.

Commands
--------

  project summary [path]    Show project summary
  project map [path]        Show dependency map
  project deps <file>       Show file dependencies
  project dependents <file> Show reverse dependencies
  project search <query>    Search symbols
  project stats [path]      Show statistics
  project tree [path]       Show file tree
  project index [path]      Re-index project

Features
--------

- Automatic project type detection (Swift, TypeScript, Python, Rust, Go, etc.)
- Architecture pattern detection (MVC, MVVM, Clean Architecture, etc.)
- Convention detection from code patterns
- Symbol indexing with dependency tracking
- Fast cached queries via SQLite

Installation
------------

  ./install.sh

Or manually:

  pip install -e .

Cache Location
--------------

  ~/.cache/daedalos/project/<project-hash>/

The cache auto-updates when files change.
