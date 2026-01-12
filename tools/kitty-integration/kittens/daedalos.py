#!/usr/bin/env python3
"""
Daedalos Launcher Kitten for Kitty terminal.

A quick-access menu for all Daedalos tools. Invoke with ctrl+shift+space
or run: kitten daedalos.py

Single-key shortcuts match the mnemonic keybindings:
  l = loop, a = agents, v = verify, etc.
"""

import subprocess
import sys
from typing import List, Tuple

from kitty.boss import Boss


def main(args: List[str]) -> str:
    """Entry point when run as: kitten daedalos.py [tool]"""
    pass


TOOLS = [
    ("l", "loop status", "Loop status - check iteration progress"),
    ("a", "agent list", "Agent list - see running agents"),
    ("n", "agent spawn --name agent-new", "New agent - spawn implementer"),
    ("v", "verify --quick", "Verify quick - fast checks"),
    ("V", "verify", "Verify full - comprehensive checks"),
    ("u", "undo timeline", "Undo timeline - recent changes"),
    ("z", "undo last", "Undo last - revert last change"),
    ("p", "project info", "Project info - codebase summary"),
    ("t", "project tree", "Project tree - file structure"),
    ("s", "codex search", "Codex search - semantic search"),
    ("e", "error-db match", "Error lookup - search solutions"),
    ("j", "journal what", "Journal - activity history"),
    ("g", "gates level", "Gates level - supervision status"),
    ("c", "context estimate", "Context - token usage"),
    ("x", "scratch list", "Scratch - ephemeral environments"),
    ("o", "observe", "Observe TUI - full dashboard"),
    ("d", "daedalos status", "Daedalos status - system health"),
    ("?", "daedalos help", "Help - command reference"),
]

# Colors matching Daedalos theme
BRONZE = "\033[38;2;212;165;116m"
TEAL = "\033[38;2;107;196;180m"
CREAM = "\033[38;2;232;224;212m"
MUTED = "\033[38;2;107;101;96m"
GREEN = "\033[38;2;90;154;122m"
AMBER = "\033[38;2;229;168;75m"
RESET = "\033[0m"
BOLD = "\033[1m"
DIM = "\033[2m"


def handle_result(
    args: List[str], answer: str, target_window_id: int, boss: Boss
) -> None:
    """Handle the selected menu item."""
    if not answer:
        return

    answer = answer.strip()

    # Find the command for this key
    cmd = None
    for key, command, _ in TOOLS:
        if answer.lower() == key.lower():
            cmd = command
            break

    if not cmd:
        return

    # Special handling for interactive commands
    if cmd == "codex search":
        # Open overlay and prompt for query
        boss.call_remote_control(
            boss.active_window,
            ("launch", "--type=overlay", "sh", "-c",
             'echo -n "Search: "; read q; codex search "$q"')
        )
    elif cmd == "error-db match":
        boss.call_remote_control(
            boss.active_window,
            ("launch", "--type=overlay", "sh", "-c",
             'echo -n "Error: "; read e; error-db match "$e"')
        )
    elif cmd == "observe":
        # Observe gets its own tab
        boss.call_remote_control(
            boss.active_window,
            ("launch", "--type=tab", "--title=Observe", "observe")
        )
    elif cmd.startswith("agent spawn"):
        # New agents get their own tab
        boss.call_remote_control(
            boss.active_window,
            ("launch", "--type=tab", "--title=Agent", "sh", "-c", cmd)
        )
    else:
        # Default: run in overlay
        boss.call_remote_control(
            boss.active_window,
            ("launch", "--type=overlay", "sh", "-c",
             f'{cmd} || echo "Command failed: {cmd}"')
        )


def draw_menu() -> str:
    """Draw the launcher menu."""
    lines = []

    # Header
    lines.append("")
    lines.append(f"  {BRONZE}{BOLD}DAEDALOS{RESET}")
    lines.append(f"  {DIM}Press a key to launch, Esc to cancel{RESET}")
    lines.append("")
    lines.append(f"  {MUTED}{'─' * 44}{RESET}")
    lines.append("")

    # Tool list
    for key, _, description in TOOLS:
        key_display = f"{TEAL}{BOLD}{key}{RESET}"
        desc_parts = description.split(" - ")
        name = desc_parts[0]
        detail = desc_parts[1] if len(desc_parts) > 1 else ""

        lines.append(f"  {key_display}  {CREAM}{name}{RESET}  {MUTED}{detail}{RESET}")

    lines.append("")
    lines.append(f"  {MUTED}{'─' * 44}{RESET}")
    lines.append(f"  {DIM}ctrl+shift+? for keybindings{RESET}")
    lines.append("")

    return "\n".join(lines)


if __name__ == "__main__":
    # When run directly, show the menu
    print(draw_menu())
    print("Enter key: ", end="", flush=True)

    try:
        import tty
        import termios

        fd = sys.stdin.fileno()
        old = termios.tcgetattr(fd)
        try:
            tty.setraw(fd)
            key = sys.stdin.read(1)
        finally:
            termios.tcsetattr(fd, termios.TCSADRAIN, old)

        print(key)

        # Find and run command
        for k, cmd, _ in TOOLS:
            if key.lower() == k.lower():
                if cmd == "codex search":
                    query = input("Search: ")
                    subprocess.run(["codex", "search", query])
                elif cmd == "error-db match":
                    error = input("Error: ")
                    subprocess.run(["error-db", "match", error])
                else:
                    subprocess.run(cmd.split())
                break
        else:
            if key == "\x1b":  # Escape
                print("Cancelled")
            else:
                print(f"Unknown key: {repr(key)}")

    except Exception as e:
        print(f"Error: {e}")
