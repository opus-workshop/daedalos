# Daedalos Core Module
# System-level integration for AI development tools

{ config, pkgs, lib, ... }:

let
  # Waybar configuration for Daedalos status bar
  waybarConfig = {
    layer = "top";
    position = "top";
    height = 30;

    modules-left = [
      "hyprland/workspaces"
      "custom/loop"
    ];

    modules-center = [
      "custom/agents"
    ];

    modules-right = [
      "custom/context"
      "custom/verify"
      "pulseaudio"
      "network"
      "clock"
    ];

    "hyprland/workspaces" = {
      format = "{name}";
      on-click = "activate";
    };

    "custom/loop" = {
      exec = ''
        loop status --json 2>/dev/null | jq -r '
          [.[] | select(.status == "running")] |
          if length == 0 then "No loops"
          else map("🔄" + .id[0:6] + "[" + (.current_iteration|tostring) + "/" + (.max_iterations|tostring) + "]") | join(" ")
          end
        ' 2>/dev/null || echo "No loops"
      '';
      interval = 2;
      format = "{}";
      tooltip = true;
    };

    "custom/agents" = {
      exec = ''
        agent list --json 2>/dev/null | jq -r '
          [.[] |
            (if .status == "active" then "🟢"
             elif .status == "thinking" then "🔵"
             elif .status == "waiting" then "🟡"
             elif .status == "attention" then "🔴"
             else "⚪" end) + .name
          ] | join(" ")
        ' 2>/dev/null || echo "No agents"
      '';
      interval = 2;
      format = "{}";
    };

    "custom/context" = {
      exec = ''
        context estimate --json 2>/dev/null | jq -r '"ctx:" + (.percentage|tostring) + "%"' 2>/dev/null || echo "ctx:--"
      '';
      interval = 5;
      format = "{}";
    };

    "custom/verify" = {
      exec = ''
        if [ -f /tmp/daedalos-verify-status ]; then
          cat /tmp/daedalos-verify-status
        else
          echo "verify:?"
        fi
      '';
      interval = 10;
      format = "{}";
      on-click = "kitty verify";
    };

    clock = {
      format = "{:%H:%M}";
      tooltip-format = "{:%Y-%m-%d %H:%M:%S}";
    };

    network = {
      format-wifi = "󰤨 {signalStrength}%";
      format-ethernet = "󰈀";
      format-disconnected = "󰤭";
      tooltip-format = "{ifname}: {ipaddr}";
    };

    pulseaudio = {
      format = "{icon} {volume}%";
      format-muted = "󰝟";
      format-icons = {
        default = [ "󰕿" "󰖀" "󰕾" ];
      };
      on-click = "pavucontrol";
    };
  };

  waybarStyle = ''
    * {
      font-family: "JetBrainsMono Nerd Font", monospace;
      font-size: 13px;
      min-height: 0;
    }

    window#waybar {
      background: rgba(30, 30, 46, 0.9);
      color: #cdd6f4;
      border-bottom: 2px solid rgba(122, 162, 247, 0.5);
    }

    #workspaces button {
      padding: 0 8px;
      color: #6c7086;
      border-radius: 4px;
      margin: 4px 2px;
    }

    #workspaces button.active {
      background: rgba(122, 162, 247, 0.3);
      color: #7aa2f7;
    }

    #custom-loop {
      padding: 0 12px;
      color: #f9e2af;
    }

    #custom-agents {
      padding: 0 12px;
      color: #a6e3a1;
    }

    #custom-context {
      padding: 0 8px;
      color: #89b4fa;
    }

    #custom-verify {
      padding: 0 8px;
      color: #a6e3a1;
    }

    #clock {
      padding: 0 12px;
      color: #cdd6f4;
    }

    #network, #pulseaudio {
      padding: 0 8px;
      color: #94e2d5;
    }

    tooltip {
      background: rgba(30, 30, 46, 0.95);
      border: 1px solid #7aa2f7;
      border-radius: 8px;
    }
  '';

in {
  # Waybar for status bar
  programs.waybar.enable = true;

  # Create Daedalos directories
  systemd.tmpfiles.rules = [
    "d /run/daedalos 0755 root root -"
    "d /var/lib/daedalos 0755 root root -"
  ];

  # Daedalos systemd services
  systemd.user.services = {
    loopd = {
      description = "Daedalos Loop Daemon";
      wantedBy = [ "default.target" ];
      after = [ "network.target" ];
      serviceConfig = {
        Type = "simple";
        ExecStart = "${pkgs.bash}/bin/bash -c 'loopd'";
        Restart = "on-failure";
        RestartSec = 5;
      };
    };

    undod = {
      description = "Daedalos Undo Daemon";
      wantedBy = [ "default.target" ];
      serviceConfig = {
        Type = "simple";
        ExecStart = "${pkgs.bash}/bin/bash -c 'undod'";
        Restart = "on-failure";
        RestartSec = 5;
      };
    };

    projectd = {
      description = "Daedalos Project Intelligence Daemon";
      wantedBy = [ "default.target" ];
      serviceConfig = {
        Type = "simple";
        ExecStart = "${pkgs.bash}/bin/bash -c 'project watch'";
        Restart = "on-failure";
        RestartSec = 5;
      };
    };
  };

  # Waybar configuration
  environment.etc."xdg/waybar/config".text = builtins.toJSON waybarConfig;
  environment.etc."xdg/waybar/style.css".text = waybarStyle;

  # Mako notification daemon configuration
  environment.etc."xdg/mako/config".text = ''
    font=JetBrainsMono Nerd Font 11
    background-color=#1e1e2e
    text-color=#cdd6f4
    border-color=#7aa2f7
    border-radius=8
    border-size=2
    padding=12
    default-timeout=5000

    [urgency=high]
    border-color=#f38ba8
    default-timeout=0

    [app-name=loop]
    border-color=#f9e2af

    [app-name=verify]
    border-color=#a6e3a1
  '';

  # Kitty terminal configuration
  environment.etc."xdg/kitty/kitty.conf".text = ''
    # Daedalos Kitty Configuration

    font_family JetBrainsMono Nerd Font
    font_size 12

    # Colors (Tokyo Night)
    background #1a1b26
    foreground #c0caf5
    selection_background #33467c
    selection_foreground #c0caf5
    cursor #c0caf5

    # Black
    color0 #15161e
    color8 #414868

    # Red
    color1 #f7768e
    color9 #f7768e

    # Green
    color2 #9ece6a
    color10 #9ece6a

    # Yellow
    color3 #e0af68
    color11 #e0af68

    # Blue
    color4 #7aa2f7
    color12 #7aa2f7

    # Magenta
    color5 #bb9af7
    color13 #bb9af7

    # Cyan
    color6 #7dcfff
    color14 #7dcfff

    # White
    color7 #a9b1d6
    color15 #c0caf5

    # Window
    window_padding_width 8
    hide_window_decorations yes
    confirm_os_window_close 0

    # Tab bar
    tab_bar_style powerline
    tab_powerline_style slanted

    # Shell integration
    shell_integration enabled

    # Keybindings for splits (like tmux)
    map ctrl+shift+enter new_window_with_cwd
    map ctrl+shift+h neighboring_window left
    map ctrl+shift+l neighboring_window right
    map ctrl+shift+k neighboring_window up
    map ctrl+shift+j neighboring_window down
  '';

  # Starship prompt configuration
  environment.etc."xdg/starship.toml".text = ''
    # Daedalos Starship Prompt

    format = """
    $directory\
    $git_branch\
    $git_status\
    $python\
    $nodejs\
    $rust\
    $golang\
    $cmd_duration\
    $line_break\
    $character"""

    [directory]
    style = "blue bold"
    truncation_length = 3

    [git_branch]
    symbol = " "
    style = "purple"

    [git_status]
    style = "red"

    [character]
    success_symbol = "[❯](green)"
    error_symbol = "[❯](red)"

    [cmd_duration]
    min_time = 2000
    format = "[$duration]($style) "
    style = "yellow"

    [python]
    symbol = " "

    [nodejs]
    symbol = " "

    [rust]
    symbol = " "

    [golang]
    symbol = " "
  '';

  # Zsh configuration
  programs.zsh.interactiveShellInit = ''
    # Daedalos shell initialization
    eval "$(starship init zsh)"

    # Daedalos aliases
    alias l='loop'
    alias ls='loop status'
    alias lw='loop watch'
    alias v='verify'
    alias vq='verify --quick'
    alias u='undo'
    alias ut='undo timeline'
    alias p='project'
    alias ps='project summary'
    alias c='codex'
    alias cs='codex search'
    alias a='agent'
    alias al='agent list'
    alias af='agent focus'

    # Quick loop start
    lstart() {
      loop start "$1" --promise "$2"
    }

    # Path additions
    export PATH="$HOME/.local/bin:$PATH"

    # Daedalos environment
    export DAEDALOS_CONFIG="$HOME/.config/daedalos"
    export DAEDALOS_DATA="$HOME/.local/share/daedalos"
  '';
}
