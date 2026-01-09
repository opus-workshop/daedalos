# Daedalos Hyprland Configuration
# Wayland compositor optimized for AI-assisted development

{ config, pkgs, lib, ... }:

{
  # Hyprland compositor
  programs.hyprland = {
    enable = true;
    xwayland.enable = true;
  };

  # Required services
  services.greetd = {
    enable = true;
    settings = {
      default_session = {
        command = "${pkgs.greetd.tuigreet}/bin/tuigreet --time --cmd Hyprland";
        user = "greeter";
      };
    };
  };

  # XDG portal for screen sharing, file dialogs
  xdg.portal = {
    enable = true;
    wlr.enable = true;
    extraPortals = [ pkgs.xdg-desktop-portal-gtk ];
  };

  # Environment for Wayland
  environment.sessionVariables = {
    NIXOS_OZONE_WL = "1";
    WLR_NO_HARDWARE_CURSORS = "1";
    XDG_CURRENT_DESKTOP = "Hyprland";
    XDG_SESSION_TYPE = "wayland";
    XDG_SESSION_DESKTOP = "Hyprland";
    QT_QPA_PLATFORM = "wayland";
    QT_WAYLAND_DISABLE_WINDOWDECORATION = "1";
    GDK_BACKEND = "wayland";
    MOZ_ENABLE_WAYLAND = "1";
  };

  # Hyprland configuration file
  environment.etc."daedalos/hyprland.conf".text = ''
    #===============================================================================
    #                    DAEDALOS HYPRLAND CONFIGURATION
    #===============================================================================
    #
    # "Iterate Until Done" - Keyboard-driven, AI-native window management
    #

    # Monitor configuration (adjust for your setup)
    monitor = , preferred, auto, 1

    # Input
    input {
        kb_layout = us
        follow_mouse = 1
        sensitivity = 0
        touchpad {
            natural_scroll = true
        }
    }

    # General appearance
    general {
        gaps_in = 5
        gaps_out = 10
        border_size = 2
        col.active_border = rgba(7aa2f7ee) rgba(bb9af7ee) 45deg
        col.inactive_border = rgba(414868aa)
        layout = dwindle
    }

    # Decoration
    decoration {
        rounding = 8
        blur {
            enabled = true
            size = 8
            passes = 2
        }
        drop_shadow = true
        shadow_range = 15
        shadow_render_power = 3
        col.shadow = rgba(1a1a1aee)
    }

    # Animations
    animations {
        enabled = true
        bezier = smooth, 0.25, 0.1, 0.25, 1
        animation = windows, 1, 4, smooth
        animation = windowsOut, 1, 4, smooth, popin 80%
        animation = fade, 1, 4, smooth
        animation = workspaces, 1, 4, smooth
    }

    # Dwindle layout
    dwindle {
        pseudotile = true
        preserve_split = true
    }

    # Startup applications
    exec-once = waybar
    exec-once = mako  # Notifications
    exec-once = wl-paste --watch cliphist store  # Clipboard history

    #===========================================================================
    #                         DAEDALOS KEYBINDINGS
    #===========================================================================

    $mod = SUPER

    #---------------------------------------------------------------------------
    # LOOP MANAGEMENT
    #---------------------------------------------------------------------------
    bind = $mod, L, exec, kitty --class loop-dashboard loop status
    bind = $mod SHIFT, L, exec, kitty --class loop-start bash -c 'read -p "Task: " task && read -p "Promise: " promise && loop start "$task" --promise "$promise"'
    bind = $mod CTRL, L, exec, loop cancel $(loop list --json | jq -r '.[0].id')

    #---------------------------------------------------------------------------
    # AGENT MANAGEMENT
    #---------------------------------------------------------------------------
    bind = $mod, 1, exec, agent focus 1
    bind = $mod, 2, exec, agent focus 2
    bind = $mod, 3, exec, agent focus 3
    bind = $mod, 4, exec, agent focus 4
    bind = $mod, 5, exec, agent focus 5
    bind = $mod, 6, exec, agent focus 6
    bind = $mod, 7, exec, agent focus 7
    bind = $mod, 8, exec, agent focus 8
    bind = $mod, 9, exec, agent focus 9
    bind = $mod, Tab, exec, agent focus --mru
    bind = $mod, grave, exec, agent focus --last
    bind = $mod, A, exec, kitty --class agent-switcher agent list
    bind = $mod, N, exec, kitty --class agent-new bash -c 'read -p "Agent name: " name && agent spawn -n "$name"'
    bind = $mod, Q, exec, agent kill --current
    bind = $mod, G, exec, agent grid

    #---------------------------------------------------------------------------
    # SEARCH
    #---------------------------------------------------------------------------
    bind = $mod, slash, exec, kitty --class search agent search
    bind = $mod, P, exec, kitty --class project-switcher bash -c 'cd $(find ~/projects -maxdepth 1 -type d | fzf) && exec $SHELL'
    bind = $mod, S, exec, kitty --class codex-search bash -c 'read -p "Search: " q && codex search "$q"'

    #---------------------------------------------------------------------------
    # WINDOW MANAGEMENT
    #---------------------------------------------------------------------------
    bind = $mod, Return, exec, kitty
    bind = $mod, C, killactive
    bind = $mod, M, exit
    bind = $mod, F, fullscreen
    bind = $mod, V, togglesplit
    bind = $mod, Space, togglefloating

    # Vim-style focus
    bind = $mod, H, movefocus, l
    bind = $mod, J, movefocus, d
    bind = $mod, K, movefocus, u
    bind = $mod, L, movefocus, r

    # Move windows
    bind = $mod SHIFT, H, movewindow, l
    bind = $mod SHIFT, J, movewindow, d
    bind = $mod SHIFT, K, movewindow, u
    bind = $mod SHIFT, L, movewindow, r

    # Resize
    bind = $mod CTRL, H, resizeactive, -50 0
    bind = $mod CTRL, J, resizeactive, 0 50
    bind = $mod CTRL, K, resizeactive, 0 -50
    bind = $mod CTRL, L, resizeactive, 50 0

    # Workspaces
    bind = $mod, 1, workspace, 1
    bind = $mod, 2, workspace, 2
    bind = $mod, 3, workspace, 3
    bind = $mod, 4, workspace, 4
    bind = $mod, 5, workspace, 5

    bind = $mod SHIFT, 1, movetoworkspace, 1
    bind = $mod SHIFT, 2, movetoworkspace, 2
    bind = $mod SHIFT, 3, movetoworkspace, 3
    bind = $mod SHIFT, 4, movetoworkspace, 4
    bind = $mod SHIFT, 5, movetoworkspace, 5

    # Mouse bindings
    bindm = $mod, mouse:272, movewindow
    bindm = $mod, mouse:273, resizewindow

    #---------------------------------------------------------------------------
    # UTILITIES
    #---------------------------------------------------------------------------
    bind = $mod, D, exec, wofi --show drun  # App launcher
    bind = $mod SHIFT, S, exec, grim -g "$(slurp)" - | wl-copy  # Screenshot
    bind = $mod SHIFT, V, exec, cliphist list | wofi --dmenu | cliphist decode | wl-copy

    #---------------------------------------------------------------------------
    # DAEDALOS TOOLS
    #---------------------------------------------------------------------------
    bind = $mod, U, exec, kitty --class undo undo timeline
    bind = $mod SHIFT, U, exec, undo last
    bind = $mod, Y, exec, kitty --class verify verify
    bind = $mod SHIFT, Y, exec, verify --fix

    #---------------------------------------------------------------------------
    # WINDOW RULES
    #---------------------------------------------------------------------------
    windowrulev2 = float, class:^(loop-dashboard)$
    windowrulev2 = size 800 600, class:^(loop-dashboard)$
    windowrulev2 = center, class:^(loop-dashboard)$

    windowrulev2 = float, class:^(agent-switcher)$
    windowrulev2 = size 600 400, class:^(agent-switcher)$
    windowrulev2 = center, class:^(agent-switcher)$

    windowrulev2 = float, class:^(codex-search)$
    windowrulev2 = size 800 600, class:^(codex-search)$
    windowrulev2 = center, class:^(codex-search)$

    windowrulev2 = float, class:^(loop-start)$
    windowrulev2 = size 600 200, class:^(loop-start)$
    windowrulev2 = center, class:^(loop-start)$
  '';
}
