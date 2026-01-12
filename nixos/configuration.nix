# Daedalos NixOS Configuration
# The First Operating System Built BY AI, FOR AI Development
#
# "Iterate Until Done"

{ config, pkgs, lib, ... }:

{
  imports = [
    ./modules/daedalos.nix
    ./modules/hyprland.nix
    ./modules/development.nix
  ];

  # System basics
  system.stateVersion = "24.05";

  nix = {
    settings = {
      experimental-features = [ "nix-command" "flakes" ];
      auto-optimise-store = true;
    };
    gc = {
      automatic = true;
      dates = "weekly";
      options = "--delete-older-than 30d";
    };
  };

  # Boot configuration
  boot = {
    loader = {
      systemd-boot.enable = true;
      efi.canTouchEfiVariables = true;
    };

    # Btrfs support for snapshots
    supportedFilesystems = [ "btrfs" ];

    # Kernel parameters for development
    kernelParams = [
      "mitigations=off"  # Performance (disable for production)
    ];
  };

  # Filesystem - Btrfs recommended
  fileSystems."/" = {
    device = "/dev/disk/by-label/nixos";
    fsType = "btrfs";
    options = [ "subvol=@" "compress=zstd" "noatime" ];
  };

  fileSystems."/home" = {
    device = "/dev/disk/by-label/nixos";
    fsType = "btrfs";
    options = [ "subvol=@home" "compress=zstd" "noatime" ];
  };

  fileSystems."/nix" = {
    device = "/dev/disk/by-label/nixos";
    fsType = "btrfs";
    options = [ "subvol=@nix" "compress=zstd" "noatime" ];
  };

  # Networking
  networking = {
    hostName = "daedalos";
    networkmanager.enable = true;
    firewall.enable = true;
  };

  # Time and locale
  time.timeZone = "UTC";
  i18n.defaultLocale = "en_US.UTF-8";

  # User configuration
  users.users.dev = {
    isNormalUser = true;
    description = "Daedalos Developer";
    extraGroups = [ "wheel" "networkmanager" "docker" "audio" "video" ];
    shell = pkgs.zsh;
  };

  # Security
  security = {
    sudo.wheelNeedsPassword = false;  # For AI agents
    rtkit.enable = true;  # For audio
    polkit.enable = true;
  };

  # Services
  services = {
    # Display
    xserver.enable = false;  # Wayland only

    # Audio
    pipewire = {
      enable = true;
      alsa.enable = true;
      pulse.enable = true;
    };

    # SSH for remote agents
    openssh = {
      enable = true;
      settings.PasswordAuthentication = false;
    };

    # Docker for containerized development
    docker = {
      enable = true;
      autoPrune.enable = true;
    };

    # Ollama for local LLMs
    ollama = {
      enable = true;
      acceleration = "cuda";  # or "rocm" for AMD
    };
  };

  # Programs
  programs = {
    zsh = {
      enable = true;
      autosuggestions.enable = true;
      syntaxHighlighting.enable = true;
    };

    git.enable = true;

    # Hyprland (configured in module)
    hyprland.enable = true;
  };

  # Environment
  environment = {
    systemPackages = with pkgs; [
      # Core utilities
      vim
      wget
      curl
      git
      tmux
      htop
      btop
      ripgrep
      fd
      fzf
      jq
      tree

      # Development
      gnumake
      gcc
      python3
      nodejs
      rustc
      cargo
      go

      # Terminal
      kitty
      starship

      # Wayland
      wl-clipboard
      grim
      slurp

      # Terminal browsers
      w3m        # Fast, lightweight, images in kitty
      carbonyl   # Chromium-based, full modern web in terminal

      # Media
      imv        # Wayland image viewer

      # Git tools
      lazygit    # TUI for git (staging, rebasing, etc.)
      difftastic # Structural diffs that understand code

      # Archives
      unzip
      p7zip

      # Disk awareness
      duf        # Modern df, pretty disk usage overview
      dust       # Modern du, visual directory sizes
      ncdu       # Interactive disk usage explorer

      # Daedalos tools (from flake)
      # daedalos  # Uncomment when using as module
    ];

    variables = {
      EDITOR = "vim";
      VISUAL = "vim";
      TERMINAL = "kitty";
    };

    # Daedalos directories
    etc."daedalos/config.yaml".text = ''
      # Daedalos Configuration

      agent:
        default_engine: opencode
        fallback_engine: aider

      loop:
        default_max_iterations: 50
        checkpoint_backend: btrfs  # or 'git'

      verify:
        auto_fix: false
        fail_fast: true

      codex:
        embedding_model: nomic-embed-text
        ollama_url: http://localhost:11434
    '';
  };

  # Fonts
  fonts = {
    packages = with pkgs; [
      (nerdfonts.override { fonts = [ "JetBrainsMono" "FiraCode" ]; })
      inter
      noto-fonts
      noto-fonts-emoji
    ];

    fontconfig.defaultFonts = {
      monospace = [ "JetBrainsMono Nerd Font" ];
      sansSerif = [ "Inter" ];
    };
  };
}
