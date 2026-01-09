# Daedalos ISO Builder
# Creates a bootable NixOS ISO with Daedalos pre-installed

{ config, pkgs, lib, modulesPath, ... }:

{
  imports = [
    "${modulesPath}/installer/cd-dvd/installation-cd-graphical-calamares.nix"
    "${modulesPath}/installer/cd-dvd/channel.nix"
    ../modules/daedalos.nix
    ../modules/hyprland.nix
    ../modules/development.nix
  ];

  # ISO metadata
  isoImage = {
    isoName = "daedalos-${config.system.nixos.version}-${pkgs.stdenv.hostPlatform.system}.iso";
    volumeID = "DAEDALOS";
    edition = "daedalos";

    # Include memtest
    includeSystemBuildDependencies = false;

    # Append to GRUB
    appendToMenuLabel = " - Daedalos";
  };

  # Boot splash
  boot.plymouth = {
    enable = true;
    theme = "spinner";
  };

  # Live user
  users.users.nixos = {
    isNormalUser = true;
    extraGroups = [ "wheel" "networkmanager" "docker" ];
    initialPassword = "daedalos";
    shell = pkgs.zsh;
  };

  # Auto-login for live environment
  services.greetd = {
    enable = true;
    settings = {
      default_session = {
        command = "${pkgs.greetd.tuigreet}/bin/tuigreet --time --cmd Hyprland";
        user = "nixos";
      };
      initial_session = {
        command = "Hyprland";
        user = "nixos";
      };
    };
  };

  # Include Daedalos welcome and installer
  environment.systemPackages = with pkgs; [
    # Installer
    calamares-nixos
    calamares-nixos-extensions

    # Daedalos branding
    (writeShellScriptBin "daedalos-welcome" ''
      clear
      cat << 'EOF'
      ██████╗  █████╗ ███████╗██████╗  █████╗ ██╗      ██████╗ ███████╗
      ██╔══██╗██╔══██╗██╔════╝██╔══██╗██╔══██╗██║     ██╔═══██╗██╔════╝
      ██║  ██║███████║█████╗  ██║  ██║███████║██║     ██║   ██║███████╗
      ██║  ██║██╔══██║██╔══╝  ██║  ██║██╔══██║██║     ██║   ██║╚════██║
      ██████╔╝██║  ██║███████╗██████╔╝██║  ██║███████╗╚██████╔╝███████║
      ╚═════╝ ╚═╝  ╚═╝╚══════╝╚═════╝ ╚═╝  ╚═╝╚══════╝ ╚═════╝ ╚══════╝

                    The First OS Built BY AI, FOR AI Development

                             "Iterate Until Done"

      ═══════════════════════════════════════════════════════════════════

      Welcome to Daedalos!

      Quick Start:
        • Press Super + Enter      → Open terminal
        • Press Super + L          → Loop dashboard
        • Press Super + A          → Agent switcher
        • Press Super + S          → Semantic search

      Core Commands:
        • loop start "task" --promise "test"  → Start iteration loop
        • verify                              → Run all checks
        • codex search "query"                → Semantic code search
        • agent spawn -n myagent              → Spawn new agent

      To install Daedalos to your system:
        • Run 'daedalos-install' or use the Calamares installer

      Documentation: /etc/daedalos/VISION.txt
      ═══════════════════════════════════════════════════════════════════
      EOF
    '')

    (writeShellScriptBin "daedalos-install" ''
      echo "Starting Daedalos installation..."
      echo ""
      echo "This will guide you through installing Daedalos on your system."
      echo ""
      echo "IMPORTANT: This will modify your disk. Make sure you have backups!"
      echo ""
      read -p "Continue? [y/N] " confirm
      if [[ "$confirm" == [yY] ]]; then
        calamares
      else
        echo "Installation cancelled."
      fi
    '')
  ];

  # Welcome message on login
  environment.etc."motd".text = ''

    ██████╗  █████╗ ███████╗██████╗  █████╗ ██╗      ██████╗ ███████╗
    ██╔══██╗██╔══██╗██╔════╝██╔══██╗██╔══██╗██║     ██╔═══██╗██╔════╝
    ██║  ██║███████║█████╗  ██║  ██║███████║██║     ██║   ██║███████╗
    ██║  ██║██╔══██║██╔══╝  ██║  ██║██╔══██║██║     ██║   ██║╚════██║
    ██████╔╝██║  ██║███████╗██████╔╝██║  ██║███████╗╚██████╔╝███████║
    ╚═════╝ ╚═╝  ╚═╝╚══════╝╚═════╝ ╚═╝  ╚═╝╚══════╝ ╚═════╝ ╚══════╝

    Run 'daedalos-welcome' for quick start guide.
    Run 'daedalos-install' to install to your system.

  '';

  # Copy Daedalos documentation to live environment
  environment.etc."daedalos/VISION.txt".source = ../../docs/VISION.txt;

  # Networking for live environment
  networking = {
    hostName = "daedalos-live";
    networkmanager.enable = true;
    wireless.enable = false;
  };

  # Enable SSH for remote installation
  services.openssh = {
    enable = true;
    settings.PermitRootLogin = "yes";
  };

  # Pre-pull common development tools
  system.extraDependencies = with pkgs; [
    # Ensure these are cached in the ISO
    git
    vim
    tmux
    python3
    nodejs
    rustc
    go
  ];
}
