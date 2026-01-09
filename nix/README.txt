Daedalos NixOS Integration
==========================

QUICK START
-----------

1. Add to your flake.nix inputs:

   inputs.daedalos.url = "github:opus-workshop/daedalos";

2. Add the overlay to your NixOS configuration:

   nixpkgs.overlays = [ inputs.daedalos.overlays.default ];

3. Enable Daedalos:

   services.daedalos.enable = true;

Or just add the package:

   environment.systemPackages = [ inputs.daedalos.packages.${system}.default ];


DEVELOPMENT SHELL
-----------------

Enter a development shell with all Daedalos tools:

   nix develop github:opus-workshop/daedalos

Or from the repo:

   nix develop


INDIVIDUAL PACKAGES
-------------------

You can install individual tools:

   nix profile install github:opus-workshop/daedalos#loop
   nix profile install github:opus-workshop/daedalos#verify
   nix profile install github:opus-workshop/daedalos#undo
   nix profile install github:opus-workshop/daedalos#project
   nix profile install github:opus-workshop/daedalos#codex
   nix profile install github:opus-workshop/daedalos#agent
   nix profile install github:opus-workshop/daedalos#scratch

Or all at once:

   nix profile install github:opus-workshop/daedalos


NIXOS MODULE OPTIONS
--------------------

services.daedalos.enable (bool)
   Enable the Daedalos environment.
   Default: false

services.daedalos.defaultAgent (string)
   Default AI agent engine (opencode, aider, claude).
   Default: "opencode"

services.daedalos.enableDaemons (bool)
   Enable background services (loopd, undod).
   Default: true


EXAMPLE CONFIGURATION
---------------------

{
  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    daedalos.url = "github:opus-workshop/daedalos";
  };

  outputs = { nixpkgs, daedalos, ... }: {
    nixosConfigurations.myhost = nixpkgs.lib.nixosSystem {
      system = "x86_64-linux";
      modules = [
        daedalos.nixosModules.default
        {
          services.daedalos = {
            enable = true;
            defaultAgent = "opencode";
          };
        }
      ];
    };
  };
}


REQUIREMENTS
------------

- NixOS unstable or 24.05+
- Btrfs recommended for sandbox snapshots (git fallback available)
- Ollama for local embeddings (optional but recommended)


PART OF DAEDALOS
----------------

The first operating system built BY AI, FOR AI development.

"Iterate Until Done"
