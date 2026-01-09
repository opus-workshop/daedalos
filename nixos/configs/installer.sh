#!/usr/bin/env bash
#===============================================================================
#                    DAEDALOS INSTALLATION WIZARD
#===============================================================================
#
# Interactive installer for Daedalos NixOS
# "Iterate Until Done"
#

set -euo pipefail

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
CYAN='\033[0;36m'
BOLD='\033[1m'
NC='\033[0m'

# Banner
show_banner() {
    clear
    cat << 'EOF'
    ██████╗  █████╗ ███████╗██████╗  █████╗ ██╗      ██████╗ ███████╗
    ██╔══██╗██╔══██╗██╔════╝██╔══██╗██╔══██╗██║     ██╔═══██╗██╔════╝
    ██║  ██║███████║█████╗  ██║  ██║███████║██║     ██║   ██║███████╗
    ██║  ██║██╔══██║██╔══╝  ██║  ██║██╔══██║██║     ██║   ██║╚════██║
    ██████╔╝██║  ██║███████╗██████╔╝██║  ██║███████╗╚██████╔╝███████║
    ╚═════╝ ╚═╝  ╚═╝╚══════╝╚═════╝ ╚═╝  ╚═╝╚══════╝ ╚═════╝ ╚══════╝

                    INSTALLATION WIZARD

                    "Iterate Until Done"
═══════════════════════════════════════════════════════════════════════════════
EOF
}

# Log functions
info() { echo -e "${BLUE}[INFO]${NC} $1"; }
success() { echo -e "${GREEN}[OK]${NC} $1"; }
warn() { echo -e "${YELLOW}[WARN]${NC} $1"; }
error() { echo -e "${RED}[ERROR]${NC} $1"; }

# Check if running as root
check_root() {
    if [[ $EUID -ne 0 ]]; then
        error "This installer must be run as root"
        echo "Please run: sudo $0"
        exit 1
    fi
}

# Detect available disks
detect_disks() {
    echo ""
    info "Available disks:"
    echo ""
    lsblk -d -p -o NAME,SIZE,MODEL | grep -E "^/dev/(sd|nvme|vd)"
    echo ""
}

# Select installation disk
select_disk() {
    detect_disks
    read -p "Enter disk to install on (e.g., /dev/sda): " DISK

    if [[ ! -b "$DISK" ]]; then
        error "Invalid disk: $DISK"
        exit 1
    fi

    echo ""
    warn "WARNING: This will ERASE ALL DATA on $DISK"
    read -p "Are you sure? Type 'yes' to continue: " confirm

    if [[ "$confirm" != "yes" ]]; then
        error "Installation cancelled"
        exit 1
    fi
}

# Partition disk with Btrfs
partition_disk() {
    info "Partitioning disk..."

    # Wipe existing partition table
    wipefs -a "$DISK"

    # Create GPT partition table
    parted -s "$DISK" mklabel gpt

    # Create EFI partition (512MB)
    parted -s "$DISK" mkpart ESP fat32 1MiB 513MiB
    parted -s "$DISK" set 1 esp on

    # Create root partition (rest of disk)
    parted -s "$DISK" mkpart root btrfs 513MiB 100%

    # Determine partition names
    if [[ "$DISK" == *"nvme"* ]]; then
        EFI_PART="${DISK}p1"
        ROOT_PART="${DISK}p2"
    else
        EFI_PART="${DISK}1"
        ROOT_PART="${DISK}2"
    fi

    # Wait for partitions
    sleep 2
    partprobe "$DISK"

    success "Disk partitioned"
}

# Format partitions
format_disk() {
    info "Formatting partitions..."

    # Format EFI
    mkfs.fat -F32 -n ESP "$EFI_PART"

    # Format root with Btrfs
    mkfs.btrfs -f -L nixos "$ROOT_PART"

    success "Partitions formatted"
}

# Create Btrfs subvolumes
create_subvolumes() {
    info "Creating Btrfs subvolumes..."

    # Mount root
    mount "$ROOT_PART" /mnt

    # Create subvolumes
    btrfs subvolume create /mnt/@
    btrfs subvolume create /mnt/@home
    btrfs subvolume create /mnt/@nix
    btrfs subvolume create /mnt/@snapshots

    # Unmount
    umount /mnt

    success "Subvolumes created"
}

# Mount filesystems
mount_filesystems() {
    info "Mounting filesystems..."

    # Mount root subvolume
    mount -o subvol=@,compress=zstd,noatime "$ROOT_PART" /mnt

    # Create mount points
    mkdir -p /mnt/{home,nix,boot,.snapshots}

    # Mount other subvolumes
    mount -o subvol=@home,compress=zstd,noatime "$ROOT_PART" /mnt/home
    mount -o subvol=@nix,compress=zstd,noatime "$ROOT_PART" /mnt/nix
    mount -o subvol=@snapshots,compress=zstd,noatime "$ROOT_PART" /mnt/.snapshots

    # Mount EFI
    mount "$EFI_PART" /mnt/boot

    success "Filesystems mounted"
}

# Generate NixOS configuration
generate_config() {
    info "Generating NixOS configuration..."

    # Generate hardware configuration
    nixos-generate-config --root /mnt

    # Get user input
    echo ""
    read -p "Enter hostname [daedalos]: " HOSTNAME
    HOSTNAME="${HOSTNAME:-daedalos}"

    read -p "Enter username [dev]: " USERNAME
    USERNAME="${USERNAME:-dev}"

    read -p "Enter timezone [UTC]: " TIMEZONE
    TIMEZONE="${TIMEZONE:-UTC}"

    # Download Daedalos configuration
    info "Downloading Daedalos configuration..."

    mkdir -p /mnt/etc/nixos/daedalos

    # Copy or download Daedalos modules
    if [[ -d "/etc/daedalos/nixos" ]]; then
        # Running from live ISO
        cp -r /etc/daedalos/nixos/* /mnt/etc/nixos/daedalos/
    else
        # Download from repository
        curl -sL https://raw.githubusercontent.com/opus-workshop/daedalos/main/nixos/modules/daedalos.nix \
            -o /mnt/etc/nixos/daedalos/daedalos.nix
        curl -sL https://raw.githubusercontent.com/opus-workshop/daedalos/main/nixos/modules/hyprland.nix \
            -o /mnt/etc/nixos/daedalos/hyprland.nix
        curl -sL https://raw.githubusercontent.com/opus-workshop/daedalos/main/nixos/modules/development.nix \
            -o /mnt/etc/nixos/daedalos/development.nix
    fi

    # Generate main configuration
    cat > /mnt/etc/nixos/configuration.nix << NIXEOF
# Daedalos NixOS Configuration
# Generated by installer

{ config, pkgs, ... }:

{
  imports = [
    ./hardware-configuration.nix
    ./daedalos/daedalos.nix
    ./daedalos/hyprland.nix
    ./daedalos/development.nix
  ];

  # Boot
  boot.loader.systemd-boot.enable = true;
  boot.loader.efi.canTouchEfiVariables = true;

  # Networking
  networking.hostName = "${HOSTNAME}";
  networking.networkmanager.enable = true;

  # Time
  time.timeZone = "${TIMEZONE}";

  # Locale
  i18n.defaultLocale = "en_US.UTF-8";

  # User
  users.users.${USERNAME} = {
    isNormalUser = true;
    extraGroups = [ "wheel" "networkmanager" "docker" ];
    shell = pkgs.zsh;
  };

  # Allow unfree packages
  nixpkgs.config.allowUnfree = true;

  # Nix settings
  nix.settings.experimental-features = [ "nix-command" "flakes" ];

  # System version
  system.stateVersion = "24.05";
}
NIXEOF

    success "Configuration generated"
}

# Install NixOS
install_system() {
    info "Installing NixOS (this may take a while)..."

    nixos-install --no-root-passwd

    success "NixOS installed!"
}

# Set user password
set_password() {
    echo ""
    info "Set password for user: $USERNAME"
    nixos-enter --root /mnt -- passwd "$USERNAME"
}

# Finish installation
finish() {
    echo ""
    echo "═══════════════════════════════════════════════════════════════════════════════"
    success "Daedalos installation complete!"
    echo ""
    echo "  Next steps:"
    echo "    1. Reboot into your new system"
    echo "    2. Log in as '$USERNAME'"
    echo "    3. Run 'daedalos-welcome' for quick start guide"
    echo ""
    echo "  Core commands:"
    echo "    loop start \"task\" --promise \"test\"  - Start iteration loop"
    echo "    verify                               - Run all checks"
    echo "    codex search \"query\"                 - Semantic code search"
    echo ""
    echo "═══════════════════════════════════════════════════════════════════════════════"
    echo ""
    read -p "Reboot now? [Y/n] " reboot_now

    if [[ "${reboot_now,,}" != "n" ]]; then
        reboot
    fi
}

# Main
main() {
    show_banner
    check_root

    echo ""
    info "This wizard will install Daedalos on your system."
    echo ""
    read -p "Press Enter to continue or Ctrl+C to cancel..."

    select_disk
    partition_disk
    format_disk
    create_subvolumes
    mount_filesystems
    generate_config
    install_system
    set_password
    finish
}

main "$@"
