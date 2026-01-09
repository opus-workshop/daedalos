Codex - Semantic Code Search
=============================

Ask natural language questions about your codebase.

USAGE
-----
  codex "where is authentication handled?"
  codex "what functions call the database?"
  codex "show me error handling patterns"
  codex -f auth.py "login logic"

COMMANDS
--------
  codex <query>              Search the codebase
  codex index                Build/update the index
  codex status               Show index statistics
  codex clear                Clear the index
  codex similar <file> <line>  Find similar code
  codex explain <file> <query> Search within a file

OPTIONS
-------
  -p, --project PATH    Project path (default: current directory)
  -n, --limit N         Number of results (default: 5)
  -f, --file PATTERN    Filter by file path
  -t, --type TYPE       Filter by chunk type (function, class, etc.)
  -c, --show-content    Show code content in results
  --reindex             Force reindex

HOW IT WORKS
------------
1. Indexes your code by chunking files into functions, classes, etc.
2. Generates embeddings for each chunk using:
   - Ollama (nomic-embed-text) if available
   - TF-IDF fallback otherwise
3. Searches using vector similarity

EMBEDDING BACKENDS
------------------
- Ollama (recommended): High quality semantic search
  Install: https://ollama.ai
  Model: nomic-embed-text (pulled automatically)

- TF-IDF (fallback): Works without external dependencies
  Keyword-based similarity, less semantic understanding

INSTALL
-------
  ./install.sh

PART OF DAEDALOS
----------------
Tools designed BY AI, FOR AI development.
