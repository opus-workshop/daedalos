{
  description = "Daedalos NixOS - AI-native development operating system";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    home-manager = {
      url = "github:nix-community/home-manager";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs = { self, nixpkgs, home-manager, ... }@inputs:
    let
      supportedSystems = [ "x86_64-linux" "aarch64-linux" ];
      forAllSystems = nixpkgs.lib.genAttrs supportedSystems;

      # Common NixOS modules for Daedalos
      daedalosModules = [
        ./modules/daedalos.nix
        ./modules/hyprland.nix
        ./modules/development.nix
      ];

    in {
      # NixOS configurations
      nixosConfigurations = {
        # Standard Daedalos installation
        daedalos = nixpkgs.lib.nixosSystem {
          system = "x86_64-linux";
          modules = [
            ./configuration.nix
          ] ++ daedalosModules;
        };

        # Daedalos for ARM64 (e.g., Apple Silicon VM, Pi)
        daedalos-arm = nixpkgs.lib.nixosSystem {
          system = "aarch64-linux";
          modules = [
            ./configuration.nix
          ] ++ daedalosModules;
        };

        # Live ISO
        daedalos-iso = nixpkgs.lib.nixosSystem {
          system = "x86_64-linux";
          modules = [
            ./iso/default.nix
          ];
        };

        # Live ISO for ARM64
        daedalos-iso-arm = nixpkgs.lib.nixosSystem {
          system = "aarch64-linux";
          modules = [
            ./iso/default.nix
          ];
        };

        # Minimal server variant (no GUI)
        daedalos-server = nixpkgs.lib.nixosSystem {
          system = "x86_64-linux";
          modules = [
            ./configuration.nix
            ./modules/daedalos.nix
            ./modules/development.nix
            {
              # Disable GUI for server
              programs.hyprland.enable = nixpkgs.lib.mkForce false;
              services.greetd.enable = nixpkgs.lib.mkForce false;
              environment.systemPackages = nixpkgs.lib.mkForce (with nixpkgs.legacyPackages.x86_64-linux; [
                vim git tmux htop
              ]);
            }
          ];
        };
      };

      # Build ISO images
      packages = forAllSystems (system: {
        iso = self.nixosConfigurations."daedalos-iso${if system == "aarch64-linux" then "-arm" else ""}".config.system.build.isoImage;

        # VM for testing
        vm = self.nixosConfigurations."daedalos${if system == "aarch64-linux" then "-arm" else ""}".config.system.build.vm;
      });

      # Development shells
      devShells = forAllSystems (system:
        let pkgs = nixpkgs.legacyPackages.${system};
        in {
          default = pkgs.mkShell {
            buildInputs = with pkgs; [
              nixos-rebuild
              nixos-generators
            ];
            shellHook = ''
              echo "Daedalos NixOS Development Shell"
              echo ""
              echo "Build ISO:  nix build .#iso"
              echo "Build VM:   nix build .#vm"
              echo "Run VM:     ./result/bin/run-daedalos-vm"
            '';
          };
        }
      );

      # Expose modules for use in other flakes
      nixosModules = {
        daedalos = ./modules/daedalos.nix;
        hyprland = ./modules/hyprland.nix;
        development = ./modules/development.nix;
        default = {
          imports = daedalosModules;
        };
      };
    };
}
