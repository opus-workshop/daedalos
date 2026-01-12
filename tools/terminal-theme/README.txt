Daedalos Terminal.app Theme
============================

A warm, craftsman's workshop aesthetic for macOS Terminal.app with
built-in keybindings for daedalos-tools.

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
1. Run the installer:
   swift install.swift

2. Quit and reopen Terminal.app

3. Set as default (optional):
   Terminal > Settings > Profiles > Daedalos > "Default" button

4. For Iosevka font (recommended):
   brew install --cask font-iosevka

   Or the theme uses your system monospace font.

Keybindings
-----------
All shortcuts use Cmd+Shift as the modifier:

  Cmd+Shift+L    loop status         Check iteration loop status
  Cmd+Shift+V    verify --quick      Run quick verification
  Cmd+Shift+Z    undo last           Undo the last file change
  Cmd+Shift+U    undo timeline       View undo history
  Cmd+Shift+C    undo checkpoint "   Create named checkpoint (type name, close quote)
  Cmd+Shift+P    project info        Show project overview
  Cmd+Shift+T    project tree        Show file structure
  Cmd+Shift+S    codex search "      Semantic code search (type query, close quote)
  Cmd+Shift+E    error-db match "    Look up error solution (type error, close quote)
  Cmd+Shift+A    agent list          List running agents
  Cmd+Shift+X    context estimate    Check context window usage
  Cmd+Shift+J    journal what        View recent activity

Interactive shortcuts (marked with ") leave the command ready for you to
type your input and close the quote before pressing Enter.

Customization
-------------
To modify keybindings:
  Terminal > Settings > Profiles > Daedalos > Keyboard

To adjust colors:
  Terminal > Settings > Profiles > Daedalos > Text/Background tabs

Uninstall
---------
Terminal > Settings > Profiles > Daedalos > "..." menu > Delete Profile
