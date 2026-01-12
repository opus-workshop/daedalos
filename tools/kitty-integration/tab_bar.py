"""
Daedalos Tab Bar for Kitty.

Displays agent status counts in the tab bar: [running/thinking/idle]
Updates every draw cycle with 2-second caching to prevent performance impact.

To use, add to kitty.conf:
    tab_bar_style custom
    tab_bar_edge top

And place this file at ~/.config/kitty/tab_bar.py
"""

import json
import subprocess
import time
from typing import Any, Dict, List, Optional, Tuple

from kitty.boss import get_boss
from kitty.fast_data_types import Screen
from kitty.tab_bar import DrawData, ExtraData, TabBarData, as_rgb, draw_title

# Cache for agent status
_agent_cache: Dict[str, Any] = {
    "data": None,
    "timestamp": 0,
}
CACHE_TTL = 2.0  # seconds


def get_agent_status() -> Optional[Dict[str, int]]:
    """
    Get agent status counts from 'agent list --json'.
    Returns dict with running/thinking/idle counts, or None on error.
    Cached for CACHE_TTL seconds.
    """
    global _agent_cache

    now = time.time()
    if _agent_cache["data"] is not None and (now - _agent_cache["timestamp"]) < CACHE_TTL:
        return _agent_cache["data"]

    try:
        result = subprocess.run(
            ["agent", "list", "--json"],
            capture_output=True,
            text=True,
            timeout=1.0,
        )

        if result.returncode != 0:
            _agent_cache["data"] = None
            _agent_cache["timestamp"] = now
            return None

        data = json.loads(result.stdout)
        agents = data.get("agents", [])

        counts = {"running": 0, "thinking": 0, "idle": 0}
        for agent in agents:
            status = agent.get("status", "unknown").lower()
            if status == "running":
                counts["running"] += 1
            elif status in ("thinking", "waiting"):
                counts["thinking"] += 1
            else:
                counts["idle"] += 1

        _agent_cache["data"] = counts
        _agent_cache["timestamp"] = now
        return counts

    except (subprocess.TimeoutExpired, json.JSONDecodeError, FileNotFoundError):
        _agent_cache["data"] = None
        _agent_cache["timestamp"] = now
        return None


def draw_tab(
    draw_data: DrawData,
    screen: Screen,
    tab: TabBarData,
    before: int,
    max_tab_length: int,
    index: int,
    is_last: bool,
    extra_data: ExtraData,
) -> int:
    """Draw a single tab with optional agent status suffix."""

    # Use default title drawing
    end = draw_title(draw_data, screen, tab, index, max_tab_length)

    # Only add agent status to the last tab (rightmost)
    if is_last:
        status = get_agent_status()
        if status and (status["running"] + status["thinking"] + status["idle"]) > 0:
            # Format: [R/T/I]
            status_str = f" [{status['running']}/{status['thinking']}/{status['idle']}]"

            # Colors from Daedalos theme
            # Green for running, teal for thinking, amber for idle
            running_color = as_rgb(0x5a9a7a)  # copper-green
            thinking_color = as_rgb(0x4a9a8c)  # teal
            idle_color = as_rgb(0xe5a84b)  # amber
            bracket_color = as_rgb(0x6b6560)  # fg-muted

            # Draw the status indicator
            screen.cursor.fg = bracket_color
            screen.draw(" [")

            screen.cursor.fg = running_color
            screen.draw(str(status["running"]))

            screen.cursor.fg = bracket_color
            screen.draw("/")

            screen.cursor.fg = thinking_color
            screen.draw(str(status["thinking"]))

            screen.cursor.fg = bracket_color
            screen.draw("/")

            screen.cursor.fg = idle_color
            screen.draw(str(status["idle"]))

            screen.cursor.fg = bracket_color
            screen.draw("]")

            end = screen.cursor.x

    return end


def draw_tab_with_powerline(
    draw_data: DrawData,
    screen: Screen,
    tab: TabBarData,
    before: int,
    max_tab_length: int,
    index: int,
    is_last: bool,
    extra_data: ExtraData,
) -> int:
    """
    Alternative tab drawing with powerline style.
    Use this if you prefer powerline aesthetics.
    """
    # Powerline characters
    POWERLINE_SEP = ""
    POWERLINE_SEP_THIN = ""

    # Get colors
    if tab.is_active:
        bg = as_rgb(0xd4a574)  # bronze
        fg = as_rgb(0x151820)  # bg-main
    else:
        bg = as_rgb(0x1e222a)  # slightly lighter bg
        fg = as_rgb(0xa89f94)  # fg-secondary

    tab_bar_bg = as_rgb(0x0f1114)  # bg-deep

    # Draw separator from previous tab
    if before > 0:
        prev_bg = as_rgb(0xd4a574) if extra_data.previous_tab.is_active else as_rgb(0x1e222a)
        screen.cursor.fg = prev_bg
        screen.cursor.bg = bg
        screen.draw(POWERLINE_SEP)

    # Draw tab content
    screen.cursor.fg = fg
    screen.cursor.bg = bg

    # Truncate title if needed
    title = tab.title
    max_title_len = max_tab_length - 4  # Leave room for padding and separator
    if len(title) > max_title_len:
        title = title[:max_title_len - 1] + "…"

    screen.draw(f" {title} ")

    # Draw end separator
    if is_last:
        screen.cursor.fg = bg
        screen.cursor.bg = tab_bar_bg
        screen.draw(POWERLINE_SEP)

        # Add agent status
        status = get_agent_status()
        if status and (status["running"] + status["thinking"] + status["idle"]) > 0:
            screen.cursor.bg = tab_bar_bg

            running_color = as_rgb(0x5a9a7a)
            thinking_color = as_rgb(0x4a9a8c)
            idle_color = as_rgb(0xe5a84b)
            bracket_color = as_rgb(0x6b6560)

            screen.cursor.fg = bracket_color
            screen.draw(" [")
            screen.cursor.fg = running_color
            screen.draw(str(status["running"]))
            screen.cursor.fg = bracket_color
            screen.draw("/")
            screen.cursor.fg = thinking_color
            screen.draw(str(status["thinking"]))
            screen.cursor.fg = bracket_color
            screen.draw("/")
            screen.cursor.fg = idle_color
            screen.draw(str(status["idle"]))
            screen.cursor.fg = bracket_color
            screen.draw("]")

    return screen.cursor.x
