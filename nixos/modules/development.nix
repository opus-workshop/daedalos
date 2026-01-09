# Daedalos Development Environment Module
# Pre-configured tools for AI-assisted software development

{ config, pkgs, lib, ... }:

{
  # Language servers for LSP pool
  environment.systemPackages = with pkgs; [
    # Language servers
    nodePackages.typescript-language-server  # TypeScript/JavaScript
    nodePackages.vscode-langservers-extracted # HTML/CSS/JSON
    rust-analyzer                             # Rust
    gopls                                     # Go
    pyright                                   # Python
    lua-language-server                       # Lua
    nil                                       # Nix
    clang-tools                               # C/C++
    sourcekit-lsp                             # Swift (if available)

    # Build tools
    gnumake
    cmake
    ninja
    meson

    # Compilers/interpreters
    gcc
    clang
    python312
    python312Packages.pip
    python312Packages.virtualenv
    nodejs_22
    rustc
    cargo
    go

    # Package managers
    yarn
    pnpm
    poetry
    uv  # Fast Python package installer

    # Linters/formatters (for verify tool)
    shellcheck
    shfmt
    nodePackages.eslint
    nodePackages.prettier
    ruff              # Python linter (replaces flake8, isort, etc.)
    rustfmt
    clippy
    golangci-lint
    nixpkgs-fmt

    # Testing tools
    nodePackages.jest
    python312Packages.pytest
    python312Packages.coverage

    # Database tools
    sqlite
    postgresql
    redis

    # Container tools
    docker-compose
    podman
    buildah

    # AI/ML tools (for codex embeddings)
    # ollama is configured as a service

    # Version control
    git
    git-lfs
    gh  # GitHub CLI

    # Debugging
    gdb
    lldb
    strace
    ltrace

    # Network tools
    curl
    wget
    httpie
    websocat

    # Documentation
    man-pages
    man-pages-posix
  ];

  # OpenCode (FOSS AI coding agent)
  # Note: Install from source or package when available
  # environment.systemPackages = [ opencode ];

  # Aider (git-focused AI pair programming)
  environment.systemPackages = with pkgs; [
    python312Packages.aider-chat or []
  ];

  # Git configuration
  programs.git = {
    enable = true;
    config = {
      init.defaultBranch = "main";
      pull.rebase = true;
      push.autoSetupRemote = true;
      core.editor = "vim";
      diff.colorMoved = "default";
      merge.conflictStyle = "diff3";

      # Useful aliases
      alias = {
        st = "status -sb";
        co = "checkout";
        br = "branch";
        ci = "commit";
        unstage = "reset HEAD --";
        last = "log -1 HEAD";
        graph = "log --oneline --graph --decorate --all";
      };
    };
  };

  # Docker configuration
  virtualisation.docker = {
    enable = true;
    enableOnBoot = true;
    autoPrune = {
      enable = true;
      dates = "weekly";
    };
  };

  # Podman as alternative
  virtualisation.podman = {
    enable = true;
    dockerCompat = false;  # Don't conflict with Docker
  };

  # Development-related services
  services = {
    # PostgreSQL for database development
    postgresql = {
      enable = true;
      package = pkgs.postgresql_16;
      authentication = lib.mkForce ''
        local all all trust
        host all all 127.0.0.1/32 trust
        host all all ::1/128 trust
      '';
    };

    # Redis for caching
    redis.servers."" = {
      enable = true;
      port = 6379;
    };
  };

  # Environment variables for development
  environment.variables = {
    # Rust
    CARGO_HOME = "$HOME/.cargo";
    RUSTUP_HOME = "$HOME/.rustup";

    # Go
    GOPATH = "$HOME/go";
    GOBIN = "$HOME/go/bin";

    # Node
    NPM_CONFIG_PREFIX = "$HOME/.npm-global";

    # Python
    PYTHONDONTWRITEBYTECODE = "1";

    # Development conveniences
    LESS = "-R";
    PAGER = "less";
  };

  # Shell configuration for development
  programs.zsh.shellAliases = {
    # Git shortcuts
    g = "git";
    ga = "git add";
    gc = "git commit";
    gp = "git push";
    gl = "git pull";
    gst = "git status";
    gd = "git diff";
    gco = "git checkout";
    gb = "git branch";

    # Docker shortcuts
    d = "docker";
    dc = "docker-compose";
    dps = "docker ps";
    dex = "docker exec -it";

    # Development shortcuts
    py = "python3";
    pip = "pip3";
    nr = "npm run";
    yr = "yarn run";
    cr = "cargo run";
    ct = "cargo test";
  };
}
