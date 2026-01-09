LSP Pool - Pre-warmed Language Server Management
================================================

Manages a pool of pre-started language servers for instant code intelligence.
Instead of waiting 10-30 seconds for LSP startup, get immediate responses.

OVERVIEW
--------
LSP servers provide code intelligence (completions, diagnostics, go-to-definition)
but have slow startup times. LSP Pool:
- Pre-starts servers before they're needed
- Keeps them warm and ready
- Predicts which servers you'll need
- Manages memory to prevent system overload

USAGE
-----
  lsp-pool start              Start the daemon
  lsp-pool status             Show running servers
  lsp-pool warm typescript .  Pre-warm TypeScript server

  lsp-pool query hover file.ts --line 42 --col 10
  lsp-pool query definition file.ts --line 42 --col 10
  lsp-pool query completion file.ts --line 42 --col 10

SUPPORTED LANGUAGES
-------------------
  typescript  - TypeScript/JavaScript (typescript-language-server)
  python      - Python (pyright-langserver)
  rust        - Rust (rust-analyzer)
  go          - Go (gopls)

CONFIGURATION
-------------
Config file: ~/.config/daedalos/lsp-pool/config.yaml

  max_servers: 5            # Maximum concurrent servers
  memory_limit_mb: 2048     # Memory limit for all servers
  idle_timeout_minutes: 30  # Evict idle servers after this time

  servers:
    typescript:
      command: ["typescript-language-server", "--stdio"]
      memory_estimate_mb: 400

PREDICTION
----------
LSP Pool learns your usage patterns and pre-warms servers you're likely to need.
Use 'lsp-pool predict' to see what it would warm.

INSTALL
-------
  ./install.sh

PART OF DAEDALOS
----------------
Tools designed BY AI, FOR AI development.
