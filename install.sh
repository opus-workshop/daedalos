#!/usr/bin/env bash
#===============================================================================
#                         DAEDALOS INSTALLER
#                    Full Stack Agent Tool Integration
#===============================================================================
set -euo pipefail

DAEDALOS_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
DAEDALOS_VERSION="1.0.0"

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
CYAN='\033[0;36m'
NC='\033[0m' # No Color

# Directories
BIN_DIR="$HOME/.local/bin"
CONFIG_DIR="$HOME/.config/daedalos"
DATA_DIR="$HOME/.local/share/daedalos"
CLAUDE_DIR="$HOME/.claude"
LAUNCHD_DIR="$HOME/Library/LaunchAgents"

#===============================================================================
# Utilities
#===============================================================================

log() { echo -e "${BLUE}[daedalos]${NC} $1"; }
success() { echo -e "${GREEN}[daedalos]${NC} $1"; }
warn() { echo -e "${YELLOW}[daedalos]${NC} $1"; }
error() { echo -e "${RED}[daedalos]${NC} $1"; exit 1; }

check_command() {
    command -v "$1" &>/dev/null
}

backup_file() {
    local file="$1"
    if [[ -f "$file" ]]; then
        cp "$file" "${file}.backup.$(date +%Y%m%d_%H%M%S)"
        log "Backed up $file"
    fi
}

#===============================================================================
# Prerequisites Check
#===============================================================================

check_prerequisites() {
    log "Checking prerequisites..."

    local missing=()

    # Required
    check_command python3 || missing+=("python3")
    check_command git || missing+=("git")
    check_command jq || missing+=("jq")

    # Optional but recommended
    check_command ollama || warn "Ollama not found - install for local LLM support"
    check_command tmux || warn "tmux not found - some features may be limited"

    if [[ ${#missing[@]} -gt 0 ]]; then
        error "Missing required dependencies: ${missing[*]}"
    fi

    success "Prerequisites OK"
}

#===============================================================================
# Directory Setup
#===============================================================================

setup_directories() {
    log "Setting up directories..."

    mkdir -p "$BIN_DIR"
    mkdir -p "$CONFIG_DIR"/{loop,mcp-hub,sandbox,undo,verify,codex,context,lsp-pool,error-db,scratch,agent,project}
    mkdir -p "$DATA_DIR"/{loop,mcp-hub,undo,codex}
    mkdir -p "$LAUNCHD_DIR"

    success "Directories created"
}

#===============================================================================
# Tool Installation (Symlinks)
#===============================================================================

install_tools() {
    log "Installing tools to $BIN_DIR..."

    local tools=(
        "loop:loop/loop"
        "loopd:loop/loopd"
        "sandbox:sandbox/bin/sandbox"
        "mcp-hub:mcp-hub/bin/mcp-hub"
        "verify:verify/bin/verify"
        "undo:undo/bin/undo"
        "undod:undo/bin/undod"
        "codex:codex/bin/codex"
        "scratch:scratch/bin/scratch"
        "error-db:error-db/bin/error-db"
        "lsp-pool:lsp-pool/bin/lsp-pool"
        "context:context/src/context.py"
        "agent:agent/bin/agent"
    )

    for tool_spec in "${tools[@]}"; do
        local name="${tool_spec%%:*}"
        local path="${tool_spec##*:}"
        local full_path="$DAEDALOS_ROOT/tools/$path"
        local link_path="$BIN_DIR/$name"

        if [[ -f "$full_path" ]]; then
            # Make executable
            chmod +x "$full_path"
            # Create symlink (remove old one if exists)
            rm -f "$link_path"
            ln -s "$full_path" "$link_path"
            log "  Installed: $name"
        else
            warn "  Skipped (not built): $name"
        fi
    done

    # Add to PATH if not already there
    if [[ ":$PATH:" != *":$BIN_DIR:"* ]]; then
        log "Adding $BIN_DIR to PATH..."

        # Detect shell
        local shell_rc
        if [[ -n "${ZSH_VERSION:-}" ]] || [[ "$SHELL" == *"zsh"* ]]; then
            shell_rc="$HOME/.zshrc"
        else
            shell_rc="$HOME/.bashrc"
        fi

        if ! grep -q "# Daedalos tools" "$shell_rc" 2>/dev/null; then
            cat >> "$shell_rc" << 'EOF'

# Daedalos tools
export PATH="$HOME/.local/bin:$PATH"
export DAEDALOS_ROOT="$HOME/Daedalos"
EOF
            log "Added PATH to $shell_rc (restart shell or source it)"
        fi
    fi

    success "Tools installed"
}

#===============================================================================
# Claude Code Integration
#===============================================================================

setup_claude_code() {
    log "Setting up Claude Code integration..."

    # Ensure .claude directory exists
    mkdir -p "$CLAUDE_DIR"

    # Update CLAUDE.md
    setup_claude_md

    # Update settings.json
    setup_claude_settings

    # Register MCP server (if claude CLI available)
    setup_claude_mcp

    success "Claude Code integration complete"
}

setup_claude_md() {
    local claude_md="$CLAUDE_DIR/CLAUDE.md"
    local marker="# Daedalos Tools"

    # Check if already configured
    if grep -q "$marker" "$claude_md" 2>/dev/null; then
        log "  CLAUDE.md already has Daedalos section"
        return
    fi

    backup_file "$claude_md"

    # Append Daedalos documentation
    cat >> "$claude_md" << 'CLAUDEMD'

# Daedalos Tools

You have access to powerful development tools. Use them proactively.

## Core Tools

### loop - Iteration Primitive (USE THIS)
```bash
loop start "<task>" --promise "<verification command>"
loop status                    # Check running loops
loop watch <id>                # Live view
loop inject <id> "<context>"   # Add context mid-loop
```
**Always use loop for tasks needing verification.** Example:
```bash
loop start "fix failing tests" --promise "npm test"
loop start "add auth" --promise "./verify.sh" --max-iterations 20
```

### sandbox - Filesystem Isolation
```bash
sandbox create <name>          # Create isolated environment
sandbox enter <name>           # Enter sandbox
sandbox destroy <name>         # Clean up
```

### verify - Verification Pipelines
```bash
verify run <pipeline>          # Run verification
verify list                    # Available pipelines
```

### undo - File-level Undo
```bash
undo list                      # Show undo history
undo show <id>                 # Preview restore
undo restore <id>              # Restore file state
```

### codex - Semantic Code Search
```bash
codex search "<query>"         # Natural language search
codex index                    # Rebuild index
```

### error-db - Error Pattern Database
```bash
error-db match "<error>"       # Find known solutions
error-db add                   # Record new pattern
```

### mcp-hub - MCP Server Management
```bash
mcp-hub status                 # Running servers
mcp-hub tools                  # Available tools
mcp-hub install <server>       # Add server
```

### context - Context Window Management
```bash
context status                 # Check usage
context compress               # Reduce context
```

### scratch - Ephemeral Workspaces
```bash
scratch create                 # New temp workspace
scratch list                   # Active scratches
```

### lsp-pool - Language Server Pool
```bash
lsp-pool status                # Pool status
lsp-pool warm <language>       # Pre-warm server
```

### agent - Multi-Agent Orchestration
```bash
agent spawn "<task>"           # Spawn sub-agent
agent status                   # Running agents
```

## When to Use What

| Situation | Tool |
|-----------|------|
| Need verification/iteration | `loop` |
| Risky file changes | `sandbox` or `undo` |
| Find code by meaning | `codex` |
| Unknown error | `error-db` |
| Running tests/lints | `verify` |
| Complex multi-step | `agent` |
CLAUDEMD

    log "  Updated CLAUDE.md with tool documentation"
}

setup_claude_settings() {
    local settings="$CLAUDE_DIR/settings.json"

    if [[ ! -f "$settings" ]]; then
        echo '{}' > "$settings"
    fi

    backup_file "$settings"

    # Add Daedalos tool permissions using jq
    local new_permissions=(
        "Bash(loop:*)"
        "Bash(loopd:*)"
        "Bash(sandbox:*)"
        "Bash(mcp-hub:*)"
        "Bash(verify:*)"
        "Bash(undo:*)"
        "Bash(undod:*)"
        "Bash(codex:*)"
        "Bash(scratch:*)"
        "Bash(error-db:*)"
        "Bash(lsp-pool:*)"
        "Bash(context:*)"
        "Bash(agent:*)"
        "Bash(ssh-add:*)"
    )

    # Build jq filter to add permissions
    local jq_filter='.permissions.allow = (.permissions.allow // []) + ['
    for i in "${!new_permissions[@]}"; do
        if [[ $i -gt 0 ]]; then
            jq_filter+=','
        fi
        jq_filter+='"'"${new_permissions[$i]}"'"'
    done
    jq_filter+='] | .permissions.allow = (.permissions.allow | unique)'

    # Apply with jq
    local tmp=$(mktemp)
    jq "$jq_filter" "$settings" > "$tmp" && mv "$tmp" "$settings"

    log "  Updated settings.json with tool permissions"
}

setup_claude_mcp() {
    if ! check_command claude; then
        warn "  Claude CLI not found - skipping MCP registration"
        warn "  Run manually: claude mcp add --transport stdio daedalos --scope user -- mcp-hub serve"
        return
    fi

    # Check if already registered
    if claude mcp list 2>/dev/null | grep -q "daedalos"; then
        log "  MCP server 'daedalos' already registered"
        return
    fi

    log "  Registering mcp-hub as MCP server..."
    claude mcp add --transport stdio daedalos --scope user -- "$BIN_DIR/mcp-hub" serve 2>/dev/null || \
        warn "  Could not register MCP server (mcp-hub may not be built yet)"
}

#===============================================================================
# Daemon Setup (launchd on macOS)
#===============================================================================

setup_daemons() {
    log "Setting up daemon autostart..."

    # Only for macOS
    if [[ "$(uname)" != "Darwin" ]]; then
        warn "Daemon autostart only supported on macOS currently"
        return
    fi

    # loopd
    create_launchd_plist "loopd" "$BIN_DIR/loopd" "start"

    # mcp-hub
    create_launchd_plist "mcp-hub" "$BIN_DIR/mcp-hub" "start"

    # undod
    create_launchd_plist "undod" "$BIN_DIR/undod" "start"

    success "Daemons configured (will start on login)"
}

create_launchd_plist() {
    local name="$1"
    local binary="$2"
    local args="$3"
    local plist="$LAUNCHD_DIR/com.daedalos.$name.plist"

    # Skip if binary doesn't exist
    if [[ ! -f "$binary" ]]; then
        warn "  Skipped $name (not built)"
        return
    fi

    cat > "$plist" << EOF
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>Label</key>
    <string>com.daedalos.$name</string>
    <key>ProgramArguments</key>
    <array>
        <string>$binary</string>
        <string>$args</string>
    </array>
    <key>RunAtLoad</key>
    <true/>
    <key>KeepAlive</key>
    <true/>
    <key>StandardOutPath</key>
    <string>$DATA_DIR/$name/stdout.log</string>
    <key>StandardErrorPath</key>
    <string>$DATA_DIR/$name/stderr.log</string>
    <key>EnvironmentVariables</key>
    <dict>
        <key>PATH</key>
        <string>/usr/local/bin:/usr/bin:/bin:$BIN_DIR</string>
        <key>HOME</key>
        <string>$HOME</string>
    </dict>
</dict>
</plist>
EOF

    log "  Created $plist"
}

#===============================================================================
# Ollama / OpenCode Setup
#===============================================================================

setup_ollama() {
    log "Setting up Ollama integration..."

    if ! check_command ollama; then
        warn "Ollama not installed - skipping"
        return
    fi

    # Check if ollama is running
    if ! curl -s http://localhost:11434/api/tags &>/dev/null; then
        warn "Ollama not running. Start with: ollama serve"
    fi

    # Create default model config
    local ollama_config="$CONFIG_DIR/ollama.yaml"
    cat > "$ollama_config" << 'EOF'
# Daedalos Ollama Configuration
# Recommended models for development tasks

# ============================================================================
# CHAT/CODING MODELS (for agents)
# ============================================================================

default_model: qwen2.5-coder:14b

models:
  # Primary coding model - best balance of speed/quality
  coding:
    name: qwen2.5-coder:14b
    context_length: 32768

  # Fast model for simple tasks
  fast:
    name: qwen2.5-coder:7b
    context_length: 32768

  # Large model for complex reasoning
  reasoning:
    name: qwen2.5-coder:32b
    context_length: 32768

# ============================================================================
# EMBEDDING MODELS (for codex semantic search)
# ============================================================================

embedding:
  # Primary embedding model - excellent for code
  default: nomic-embed-text

  # Alternatives:
  # - mxbai-embed-large: Higher quality, slower
  # - all-minilm: Lightweight, faster
  # - snowflake-arctic-embed: Good for code

  models:
    nomic-embed-text:
      dimensions: 768
      max_tokens: 8192
      description: "Best balance for code search"

    mxbai-embed-large:
      dimensions: 1024
      max_tokens: 512
      description: "Higher quality embeddings"

    snowflake-arctic-embed:
      dimensions: 1024
      max_tokens: 512
      description: "Optimized for retrieval"

# ============================================================================
# QUICK START
# ============================================================================
#
# Pull recommended models:
#   ollama pull qwen2.5-coder:14b      # For coding/chat
#   ollama pull nomic-embed-text       # For semantic search (codex)
#
# Optional extras:
#   ollama pull qwen2.5-coder:7b       # Fast model
#   ollama pull mxbai-embed-large      # High quality embeddings
#
EOF

    log "  Created Ollama config at $ollama_config"

    # Create codex-specific config
    local codex_config="$CONFIG_DIR/codex/config.yaml"
    cat > "$codex_config" << 'EOF'
# Codex Configuration
# Semantic code search settings

embedding:
  provider: ollama
  model: nomic-embed-text
  # Alternative: mxbai-embed-large for higher quality

  # Fallback to sentence-transformers if Ollama unavailable
  fallback:
    provider: sentence-transformers
    model: all-MiniLM-L6-v2

indexing:
  # File patterns to index
  include:
    - "*.py"
    - "*.swift"
    - "*.ts"
    - "*.tsx"
    - "*.js"
    - "*.jsx"
    - "*.go"
    - "*.rs"
    - "*.rb"
    - "*.java"
    - "*.kt"
    - "*.c"
    - "*.cpp"
    - "*.h"
    - "*.hpp"

  # Patterns to skip
  exclude:
    - "node_modules/**"
    - ".git/**"
    - "vendor/**"
    - "build/**"
    - "dist/**"
    - "__pycache__/**"
    - "*.min.js"

  # Max file size to index (in KB)
  max_file_size: 500

  # Chunk size for embedding
  chunk_size: 2000

search:
  # Default number of results
  default_limit: 10

  # Minimum similarity threshold (0-1)
  min_similarity: 0.5

cache:
  # Where to store embeddings
  path: ~/.local/share/daedalos/codex

  # Auto-reindex on file changes
  watch: true
EOF

    log "  Created codex config at $codex_config"

    # Check for models
    local models_found=0
    local models_missing=()

    if ollama list 2>/dev/null | grep -q "qwen2.5-coder"; then
        log "  Found: qwen2.5-coder (coding)"
        ((models_found++)) || true
    else
        models_missing+=("qwen2.5-coder:14b")
    fi

    if ollama list 2>/dev/null | grep -q "nomic-embed-text"; then
        log "  Found: nomic-embed-text (embeddings)"
        ((models_found++)) || true
    else
        models_missing+=("nomic-embed-text")
    fi

    if [[ ${#models_missing[@]} -gt 0 ]]; then
        echo ""
        warn "  Missing recommended models. Pull with:"
        for model in "${models_missing[@]}"; do
            echo "    ollama pull $model"
        done
    fi

    success "Ollama integration configured"
}

setup_opencode() {
    log "Setting up OpenCode integration..."

    # OpenCode config directory
    local opencode_config="$HOME/.config/opencode"
    mkdir -p "$opencode_config"

    # Create OpenCode config
    cat > "$opencode_config/config.yaml" << EOF
# OpenCode Configuration for Daedalos
# https://github.com/opencode-ai/opencode

provider: ollama
model: qwen2.5-coder:14b

ollama:
  base_url: http://localhost:11434
  model: qwen2.5-coder:14b

# MCP servers (via mcp-hub)
mcp:
  servers:
    daedalos:
      command: mcp-hub
      args: [serve]

# Editor integration
editor: \${EDITOR:-nvim}

# Loop integration
loop:
  enabled: true
  default_promise: "make test"
EOF

    log "  Created OpenCode config"
    success "OpenCode integration configured"
}

#===============================================================================
# Environment File
#===============================================================================

create_env_file() {
    log "Creating environment file..."

    local env_file="$CONFIG_DIR/env.sh"
    cat > "$env_file" << EOF
# Daedalos Environment
# Source this file: source ~/.config/daedalos/env.sh

export DAEDALOS_ROOT="$DAEDALOS_ROOT"
export DAEDALOS_CONFIG="$CONFIG_DIR"
export DAEDALOS_DATA="$DATA_DIR"

# Default agent for loop
export LOOP_AGENT="opencode"

# MCP Hub
export MCP_HUB_SOCKET="/run/daedalos/mcp-hub.sock"

# Ollama (if using local models)
export OLLAMA_HOST="http://localhost:11434"

# Aliases
alias ll='loop list'
alias ls='loop status'
alias lw='loop watch'
EOF

    success "Environment file created: $env_file"
}

#===============================================================================
# Verification
#===============================================================================

verify_installation() {
    log "Verifying installation..."

    local errors=0

    # Check symlinks
    for tool in loop sandbox mcp-hub verify undo codex; do
        if [[ -L "$BIN_DIR/$tool" ]]; then
            log "  $tool: OK"
        else
            warn "  $tool: Not installed"
            ((errors++)) || true
        fi
    done

    # Check Claude integration
    if grep -q "Daedalos Tools" "$CLAUDE_DIR/CLAUDE.md" 2>/dev/null; then
        log "  Claude CLAUDE.md: OK"
    else
        warn "  Claude CLAUDE.md: Not configured"
        ((errors++)) || true
    fi

    if [[ $errors -eq 0 ]]; then
        success "Installation verified!"
    else
        warn "Installation completed with $errors warnings"
    fi
}

#===============================================================================
# Main
#===============================================================================

print_banner() {
    echo -e "${CYAN}"
    cat << 'EOF'
    ____                  __      __
   / __ \____ ____  ____/ /___ _/ /___  _____
  / / / / __ `/ _ \/ __  / __ `/ / __ \/ ___/
 / /_/ / /_/ /  __/ /_/ / /_/ / / /_/ (__  )
/_____/\__,_/\___/\__,_/\__,_/_/\____/____/

         AI-Native Development Tools
EOF
    echo -e "${NC}"
    echo "Version: $DAEDALOS_VERSION"
    echo "Root: $DAEDALOS_ROOT"
    echo ""
}

print_post_install() {
    echo ""
    echo -e "${GREEN}============================================${NC}"
    echo -e "${GREEN}        Installation Complete!${NC}"
    echo -e "${GREEN}============================================${NC}"
    echo ""
    echo "Next steps:"
    echo ""
    echo "1. Restart your shell or run:"
    echo "   source ~/.zshrc  # or ~/.bashrc"
    echo ""
    echo "2. Pull recommended Ollama models:"
    echo "   ollama pull qwen2.5-coder:14b   # Coding agent"
    echo "   ollama pull nomic-embed-text    # Semantic search (codex)"
    echo ""
    echo "3. Start a loop:"
    echo "   loop start \"fix the tests\" --promise \"npm test\""
    echo ""
    echo "4. Try semantic code search:"
    echo "   codex \"where is authentication handled?\""
    echo ""
    echo "5. In Claude Code, all tools are auto-allowed."
    echo ""
    echo "Configs created:"
    echo "  ~/.config/daedalos/ollama.yaml   # Model settings"
    echo "  ~/.config/daedalos/codex/        # Semantic search config"
    echo ""
    echo "Documentation: $DAEDALOS_ROOT/docs/"
    echo ""
}

main() {
    print_banner

    check_prerequisites
    setup_directories
    install_tools
    setup_claude_code
    setup_daemons
    setup_ollama
    setup_opencode
    create_env_file
    verify_installation

    print_post_install
}

# Run if executed directly
if [[ "${BASH_SOURCE[0]}" == "${0}" ]]; then
    main "$@"
fi
