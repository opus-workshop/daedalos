Daedalos Kitty Integration
==========================

A warm, craftsman's workshop aesthetic for Kitty terminal with built-in
keybindings for daedalos-tools. Matches the Terminal.app theme.


Colors
------
Background:     #151820 (deep blue-black)
Foreground:     #e8e0d4 (warm cream)
Cursor:         #d4a574 (bronze)
Selection:      #2d3a4a (muted teal)

ANSI palette draws from the Daedalos theme:
- Terracotta for errors
- Copper-green for success
- Amber for warnings
- Teal for info/prompts
- Plum for special highlights


Installation
------------
1. Copy configuration files:

   cp daedalos.conf ~/.config/kitty/
   cp tab_bar.py ~/.config/kitty/
   mkdir -p ~/.config/kitty/kittens
   cp kittens/daedalos.py ~/.config/kitty/kittens/

2. Add to your kitty.conf:

   include daedalos.conf

3. For Iosevka font (recommended):

   # macOS
   brew install --cask font-iosevka

   # NixOS
   fonts.packages = [ pkgs.iosevka ];

   # Arch
   pacman -S ttf-iosevka

   # Or the theme falls back to your system monospace font.

4. Restart Kitty or press ctrl+shift+F5 to reload.

5. Verify: Press ctrl+shift+space to open the launcher.


Keybindings
-----------
All shortcuts use ctrl+shift as the modifier:

  ctrl+shift+l        loop status         Check iteration loop status
  ctrl+shift+a        agent list          See running agents
  ctrl+shift+n        new agent           Spawn agent in new tab
  ctrl+shift+v        verify --quick      Run quick verification
  ctrl+shift+V        verify              Run full verification
  ctrl+shift+u        undo timeline       View undo history
  ctrl+shift+z        undo last           Undo the last file change
  ctrl+shift+k        undo checkpoint     Create named checkpoint
  ctrl+shift+p        project info        Show project overview
  ctrl+shift+t        project tree        Show file structure
  ctrl+shift+s        codex search        Semantic code search (interactive)
  ctrl+shift+e        error-db match      Look up error solution (interactive)
  ctrl+shift+j        journal what        View recent activity
  ctrl+shift+g        gates level         Check supervision status
  ctrl+shift+c        context estimate    Check context window usage
  ctrl+shift+x        scratch list        List ephemeral environments
  ctrl+shift+alt+l    observe             Full dashboard TUI
  ctrl+shift+space    launcher            Daedalos tool menu
  ctrl+shift+?        help                Show all keybindings


Launcher Menu
-------------
Press ctrl+shift+space to open the launcher, then press a single key
to run the corresponding tool:

  l - loop status        a - agent list       v - verify quick
  u - undo timeline      p - project info     s - codex search
  e - error lookup       j - journal          g - gates level
  c - context            x - scratch          o - observe TUI

The launcher is discoverable - use it to learn the keybindings, then
graduate to direct keys once you have muscle memory.


Tab Bar Agent Status
--------------------
The tab bar shows agent counts at the right edge: [R/T/I]

  R = Running agents (green)
  T = Thinking agents (teal)
  I = Idle agents (amber)

To enable the custom tab bar:

  # In kitty.conf (after include daedalos.conf)
  tab_bar_style custom

The status updates every 2 seconds with minimal performance impact.


Remote Control
--------------
Remote control is enabled via Unix socket for programmatic access:

  socket: unix:/tmp/kitty-daedalos

This allows agents to spawn new windows, switch tabs, and inject
commands. Essential for multi-agent orchestration.

Example usage:
  kitty @ --to unix:/tmp/kitty-daedalos new-window


NixOS Integration
-----------------
For declarative configuration on NixOS with home-manager:

  programs.kitty = {
    enable = true;
    extraConfig = builtins.readFile ./daedalos.conf;
  };

  xdg.configFile = {
    "kitty/tab_bar.py".source = ./tab_bar.py;
    "kitty/kittens/daedalos.py".source = ./kittens/daedalos.py;
  };


Customization
-------------
To modify keybindings, edit daedalos.conf in ~/.config/kitty/

To adjust colors, modify the color values in the COLOR SCHEME section.
All colors use standard hex notation (#RRGGBB).

To disable agent status in tab bar:
  - Remove or comment out tab_bar_style custom
  - Or delete tab_bar.py from ~/.config/kitty/


Uninstall
---------
1. Remove from kitty.conf:
   # include daedalos.conf

2. Delete files:
   rm ~/.config/kitty/daedalos.conf
   rm ~/.config/kitty/tab_bar.py
   rm ~/.config/kitty/kittens/daedalos.py

3. Restart Kitty


Comparison with Terminal.app Theme
----------------------------------
Both themes use the same Daedalos color palette. Key differences:

  Terminal.app          Kitty
  -----------           -----
  Cmd+Shift modifier    ctrl+shift modifier
  Action: send text     Action: overlay/tab launch
  Plist keybindings     Kitty native mappings
  Basic tab bar         Agent status in tab bar
  No kittens            Launcher kitten menu

The ctrl+shift modifier is used on Kitty because:
1. Works consistently across macOS and Linux
2. Cmd is window-manager territory on many Linux setups
3. ctrl+shift is the Kitty convention for extensions
