{
  description = "Daedalos - The First OS Built BY AI, FOR AI Development";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs = { self, nixpkgs, flake-utils }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        pkgs = nixpkgs.legacyPackages.${system};

        # Python environment for tools
        pythonEnv = pkgs.python312.withPackages (ps: with ps; [
          click
          rich
          pyyaml
          watchdog
          httpx
          mcp
        ]);

        # Individual tool derivations
        daedalosTools = {
          loop = pkgs.stdenv.mkDerivation {
            pname = "daedalos-loop";
            version = "1.0.0";
            src = ./tools/loop;

            nativeBuildInputs = [ pkgs.makeWrapper ];
            buildInputs = [ pkgs.bash pythonEnv ];

            installPhase = ''
              mkdir -p $out/bin $out/lib $out/share/daedalos/templates
              cp loop $out/bin/
              cp loopd $out/bin/
              cp -r lib/* $out/lib/
              cp -r templates/* $out/share/daedalos/templates/

              wrapProgram $out/bin/loop \
                --prefix PATH : ${pkgs.lib.makeBinPath [ pythonEnv pkgs.git ]} \
                --set PYTHONPATH "$out/lib"

              wrapProgram $out/bin/loopd \
                --prefix PATH : ${pkgs.lib.makeBinPath [ pythonEnv ]} \
                --set PYTHONPATH "$out/lib"
            '';

            meta = with pkgs.lib; {
              description = "The core iteration primitive of Daedalos";
              license = licenses.mit;
              platforms = platforms.unix;
            };
          };

          verify = pkgs.stdenv.mkDerivation {
            pname = "daedalos-verify";
            version = "1.0.0";
            src = ./tools/verify;

            nativeBuildInputs = [ pkgs.makeWrapper ];
            buildInputs = [ pkgs.bash ];

            installPhase = ''
              mkdir -p $out/bin $out/lib $out/share/daedalos/pipelines
              cp bin/verify $out/bin/
              cp -r lib/* $out/lib/
              cp -r pipelines/* $out/share/daedalos/pipelines/

              wrapProgram $out/bin/verify \
                --prefix PATH : ${pkgs.lib.makeBinPath [
                  pkgs.shellcheck pkgs.shfmt  # Shell
                  pkgs.nodePackages.eslint    # JS/TS
                  pkgs.ruff pkgs.mypy         # Python
                  pkgs.clippy                 # Rust
                ]}
            '';

            meta.description = "Universal verification pipelines";
          };

          undo = pkgs.stdenv.mkDerivation {
            pname = "daedalos-undo";
            version = "1.0.0";
            src = ./tools/undo;

            nativeBuildInputs = [ pkgs.makeWrapper ];
            buildInputs = [ pkgs.bash pkgs.sqlite ];

            installPhase = ''
              mkdir -p $out/bin $out/lib
              cp bin/undo $out/bin/
              cp bin/undod $out/bin/
              cp -r lib/* $out/lib/

              wrapProgram $out/bin/undo \
                --prefix PATH : ${pkgs.lib.makeBinPath [ pkgs.git pkgs.sqlite ]}

              wrapProgram $out/bin/undod \
                --prefix PATH : ${pkgs.lib.makeBinPath [ pythonEnv pkgs.sqlite ]}
            '';

            meta.description = "File-level undo with timeline navigation";
          };

          project = pkgs.python312Packages.buildPythonApplication {
            pname = "daedalos-project";
            version = "1.0.0";
            src = ./tools/project;

            propagatedBuildInputs = with pkgs.python312Packages; [
              click
              rich
              pyyaml
            ];

            meta.description = "Pre-computed codebase intelligence";
          };

          codex = pkgs.python312Packages.buildPythonApplication {
            pname = "daedalos-codex";
            version = "1.0.0";
            src = ./tools/codex;

            propagatedBuildInputs = with pkgs.python312Packages; [
              click
              rich
              numpy
            ];

            meta.description = "Semantic code search with local embeddings";
          };

          context = pkgs.python312Packages.buildPythonApplication {
            pname = "daedalos-context";
            version = "1.0.0";
            src = ./tools/context;

            propagatedBuildInputs = with pkgs.python312Packages; [
              click
              rich
            ];

            meta.description = "Context window management";
          };

          error-db = pkgs.stdenv.mkDerivation {
            pname = "daedalos-error-db";
            version = "1.0.0";
            src = ./tools/error-db;

            nativeBuildInputs = [ pkgs.makeWrapper ];
            buildInputs = [ pythonEnv ];

            installPhase = ''
              mkdir -p $out/bin $out/lib
              cp bin/error-db $out/bin/
              cp -r errordb/* $out/lib/

              wrapProgram $out/bin/error-db \
                --prefix PATH : ${pkgs.lib.makeBinPath [ pythonEnv ]} \
                --set PYTHONPATH "$out/lib"
            '';

            meta.description = "Error pattern database with solutions";
          };

          scratch = pkgs.stdenv.mkDerivation {
            pname = "daedalos-scratch";
            version = "1.0.0";
            src = ./tools/scratch;

            nativeBuildInputs = [ pkgs.makeWrapper ];
            buildInputs = [ pkgs.bash pkgs.git ];

            installPhase = ''
              mkdir -p $out/bin $out/lib
              cp bin/scratch $out/bin/
              cp -r lib/* $out/lib/

              wrapProgram $out/bin/scratch \
                --prefix PATH : ${pkgs.lib.makeBinPath [ pkgs.git ]}
            '';

            meta.description = "Project-scoped ephemeral environments";
          };

          agent = pkgs.stdenv.mkDerivation {
            pname = "daedalos-agent";
            version = "1.0.0";
            src = ./tools/agent;

            nativeBuildInputs = [ pkgs.makeWrapper ];
            buildInputs = [ pkgs.bash pkgs.tmux pythonEnv ];

            installPhase = ''
              mkdir -p $out/bin $out/lib $out/share/daedalos/agent-templates
              cp bin/agent $out/bin/
              cp -r lib/* $out/lib/
              cp -r templates/* $out/share/daedalos/agent-templates/

              wrapProgram $out/bin/agent \
                --prefix PATH : ${pkgs.lib.makeBinPath [ pkgs.tmux pkgs.fzf ]}
            '';

            meta.description = "Multi-agent orchestration";
          };

          sandbox = pkgs.stdenv.mkDerivation {
            pname = "daedalos-sandbox";
            version = "1.0.0";
            src = ./tools/sandbox;

            nativeBuildInputs = [ pkgs.makeWrapper ];
            buildInputs = [ pkgs.bash ];

            installPhase = ''
              mkdir -p $out/bin $out/lib
              cp bin/sandbox $out/bin/
              cp -r lib/* $out/lib/

              wrapProgram $out/bin/sandbox \
                --prefix PATH : ${pkgs.lib.makeBinPath [ pkgs.bubblewrap pkgs.git ]}
            '';

            meta.description = "Filesystem isolation with Btrfs/overlay";
          };

          mcp-hub = pkgs.stdenv.mkDerivation {
            pname = "daedalos-mcp-hub";
            version = "1.0.0";
            src = ./tools/mcp-hub;

            nativeBuildInputs = [ pkgs.makeWrapper ];
            buildInputs = [ pythonEnv ];

            installPhase = ''
              mkdir -p $out/bin $out/lib
              cp bin/mcp-hub $out/bin/
              cp -r mcphub/* $out/lib/

              wrapProgram $out/bin/mcp-hub \
                --prefix PATH : ${pkgs.lib.makeBinPath [ pythonEnv ]} \
                --set PYTHONPATH "$out/lib"
            '';

            meta.description = "MCP server management hub";
          };

          lsp-pool = pkgs.stdenv.mkDerivation {
            pname = "daedalos-lsp-pool";
            version = "1.0.0";
            src = ./tools/lsp-pool;

            nativeBuildInputs = [ pkgs.makeWrapper ];
            buildInputs = [ pythonEnv ];

            installPhase = ''
              mkdir -p $out/bin $out/lib
              cp bin/lsp-pool $out/bin/
              cp -r lsppool/* $out/lib/

              wrapProgram $out/bin/lsp-pool \
                --prefix PATH : ${pkgs.lib.makeBinPath [ pythonEnv ]} \
                --set PYTHONPATH "$out/lib"
            '';

            meta.description = "Pre-warmed language server pool";
          };

          daedalos-mcp = pkgs.python312Packages.buildPythonApplication {
            pname = "daedalos-mcp";
            version = "1.0.0";
            src = ./tools/daedalos-mcp;

            propagatedBuildInputs = with pkgs.python312Packages; [
              mcp
            ];

            meta.description = "MCP server exposing all Daedalos tools";
          };
        };

        # Combined package with all tools
        daedalos = pkgs.symlinkJoin {
          name = "daedalos";
          paths = builtins.attrValues daedalosTools;

          meta = with pkgs.lib; {
            description = "Daedalos - AI-native development environment";
            homepage = "https://github.com/opus-workshop/daedalos";
            license = licenses.mit;
            platforms = platforms.unix;
          };
        };

      in {
        packages = daedalosTools // {
          default = daedalos;
          inherit daedalos;
        };

        devShells.default = pkgs.mkShell {
          buildInputs = [
            daedalos
            pythonEnv
            pkgs.git
            pkgs.tmux
            pkgs.fzf
            pkgs.ripgrep
            pkgs.fd
          ];

          shellHook = ''
            echo "🏛️  Daedalos Development Shell"
            echo "   Tools: loop, verify, undo, project, codex, agent, scratch"
            echo "   Run 'loop --help' to get started"
          '';
        };
      }
    ) // {
      # NixOS module
      nixosModules.default = { config, lib, pkgs, ... }:
        with lib;
        let
          cfg = config.services.daedalos;
        in {
          options.services.daedalos = {
            enable = mkEnableOption "Daedalos AI development environment";

            defaultAgent = mkOption {
              type = types.str;
              default = "opencode";
              description = "Default AI agent engine";
            };

            enableDaemons = mkOption {
              type = types.bool;
              default = true;
              description = "Enable background daemons (loopd, undod)";
            };
          };

          config = mkIf cfg.enable {
            environment.systemPackages = [ self.packages.${pkgs.system}.daedalos ];

            # Systemd services for daemons
            systemd.user.services = mkIf cfg.enableDaemons {
              loopd = {
                description = "Daedalos Loop Daemon";
                wantedBy = [ "default.target" ];
                serviceConfig = {
                  ExecStart = "${self.packages.${pkgs.system}.loop}/bin/loopd";
                  Restart = "on-failure";
                };
              };

              undod = {
                description = "Daedalos Undo Daemon";
                wantedBy = [ "default.target" ];
                serviceConfig = {
                  ExecStart = "${self.packages.${pkgs.system}.undo}/bin/undod";
                  Restart = "on-failure";
                };
              };
            };

            # Create state directories
            systemd.tmpfiles.rules = [
              "d /run/daedalos 0755 root root -"
            ];
          };
        };

      # Overlay for use in other flakes
      overlays.default = final: prev: {
        daedalos = self.packages.${prev.system}.daedalos;
      };
    };
}
