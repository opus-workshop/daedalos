"""Output formatting for context status."""

from typing import Dict, Any, List

# ANSI colors
COLORS = {
    "reset": "\033[0m",
    "bold": "\033[1m",
    "dim": "\033[2m",
    "green": "\033[32m",
    "yellow": "\033[33m",
    "orange": "\033[38;5;208m",
    "red": "\033[31m",
    "blue": "\033[34m",
    "cyan": "\033[36m",
}


def get_usage_color(percentage: float) -> str:
    """Get color based on usage percentage."""
    if percentage < 50:
        return COLORS["green"]
    elif percentage < 70:
        return COLORS["yellow"]
    elif percentage < 85:
        return COLORS["orange"]
    else:
        return COLORS["red"]


def format_bar(percentage: float, width: int = 40) -> str:
    """Create a progress bar."""
    filled = int(width * min(percentage, 100) / 100)
    empty = width - filled
    return "█" * filled + "░" * empty


def format_tokens(count: int) -> str:
    """Format token count for display."""
    if count >= 1000000:
        return f"{count / 1000000:.1f}M"
    elif count >= 1000:
        return f"{count / 1000:.1f}K"
    return str(count)


def format_status(status: Dict[str, Any], use_color: bool = True) -> str:
    """Format context status for display."""
    c = COLORS if use_color else {k: "" for k in COLORS}

    pct = status["percentage"]
    used = status["used"]
    max_ctx = status["max"]
    remaining = status["remaining"]

    color = get_usage_color(pct) if use_color else ""
    bar = format_bar(pct)

    lines = []
    lines.append("┌─────────────────────────────────────────────────────────┐")
    lines.append("│ CONTEXT BUDGET                                          │")
    lines.append("├─────────────────────────────────────────────────────────┤")
    lines.append(f"│ {color}{bar}{c['reset']} {pct:5.1f}%          │")
    lines.append(f"│ Used: {format_tokens(used):>6} / {format_tokens(max_ctx):>6} tokens                      │")
    lines.append(f"│ Remaining: {format_tokens(remaining):>6} tokens                              │")
    lines.append("│                                                         │")

    # Warning based on level
    warning_level = status.get("warning_level", "ok")
    if warning_level == "critical":
        lines.append(f"│ {c['red']}⚠  CRITICAL: Consider starting fresh session{c['reset']}          │")
    elif warning_level == "high":
        lines.append(f"│ {c['orange']}⚠  HIGH: Consider running 'context compact'{c['reset']}           │")
    elif warning_level == "moderate":
        lines.append(f"│ {c['yellow']}ℹ  Moderate usage - monitor as you continue{c['reset']}           │")

    lines.append("└─────────────────────────────────────────────────────────┘")

    return "\n".join(lines)


def format_breakdown(status: Dict[str, Any], use_color: bool = True) -> str:
    """Format detailed breakdown for display."""
    c = COLORS if use_color else {k: "" for k in COLORS}

    breakdown = status.get("breakdown", {})
    total = status.get("used", 1)

    lines = []
    lines.append("┌─────────────────────────────────────────────────────────┐")
    lines.append("│ CONTEXT BREAKDOWN                                       │")
    lines.append("├─────────────────────────────────────────────────────────┤")

    # Sort by token count
    sorted_items = sorted(breakdown.items(), key=lambda x: x[1], reverse=True)

    for category, count in sorted_items:
        if count == 0:
            continue

        pct = (count / total * 100) if total > 0 else 0
        bar_width = 20
        bar_filled = int(bar_width * pct / 100)
        bar = "▓" * bar_filled + "░" * (bar_width - bar_filled)

        lines.append(f"│ {category:<15} {bar} {format_tokens(count):>6} ({pct:4.1f}%) │")

    lines.append("│                                                         │")
    lines.append(f"│ {c['bold']}Total:{c['reset']} {format_tokens(total):>44} tokens │")
    lines.append("└─────────────────────────────────────────────────────────┘")

    return "\n".join(lines)


def format_files(files: List[Dict[str, Any]], use_color: bool = True, limit: int = 10) -> str:
    """Format files in context for display."""
    c = COLORS if use_color else {k: "" for k in COLORS}

    if not files:
        return f"{c['dim']}No files tracked in current context{c['reset']}"

    lines = []
    lines.append("┌─────────────────────────────────────────────────────────┐")
    lines.append("│ FILES IN CONTEXT                                        │")
    lines.append("├─────────────────────────────────────────────────────────┤")

    for f in files[:limit]:
        path = f["path"]
        tokens = f["tokens"]

        # Truncate path if needed
        if len(path) > 40:
            path = "..." + path[-37:]

        lines.append(f"│ {path:<40} {format_tokens(tokens):>8} │")

    if len(files) > limit:
        lines.append(f"│ {c['dim']}... and {len(files) - limit} more files{c['reset']}                               │")

    total_tokens = sum(f["tokens"] for f in files)
    lines.append("│                                                         │")
    lines.append(f"│ {c['bold']}Total file tokens:{c['reset']} {format_tokens(total_tokens):>32} │")
    lines.append("└─────────────────────────────────────────────────────────┘")

    return "\n".join(lines)


def format_suggestions(suggestions: List[Dict[str, Any]], use_color: bool = True) -> str:
    """Format compaction suggestions for display."""
    c = COLORS if use_color else {k: "" for k in COLORS}

    if not suggestions:
        return f"{c['green']}✓ No compaction suggestions - context usage is healthy{c['reset']}"

    lines = []
    lines.append("┌─────────────────────────────────────────────────────────┐")
    lines.append("│ COMPACTION SUGGESTIONS                                  │")
    lines.append("├─────────────────────────────────────────────────────────┤")

    for s in suggestions:
        desc = s["description"]
        savings = s["savings"]

        # Wrap description if needed
        if len(desc) > 50:
            desc = desc[:47] + "..."

        lines.append(f"│ • {desc:<50} │")
        lines.append(f"│   {c['green']}Potential savings: ~{format_tokens(savings)} tokens{c['reset']}              │")
        lines.append("│                                                         │")

    lines.append("└─────────────────────────────────────────────────────────┘")

    return "\n".join(lines)


def format_checkpoints(checkpoints: List[Dict[str, Any]], use_color: bool = True) -> str:
    """Format checkpoint list for display."""
    c = COLORS if use_color else {k: "" for k in COLORS}

    if not checkpoints:
        return f"{c['dim']}No checkpoints saved{c['reset']}"

    lines = []
    lines.append(f"{c['bold']}{'NAME':<20} {'CREATED':<20} {'TOKENS':>10}{c['reset']}")
    lines.append("-" * 52)

    for cp in checkpoints:
        name = cp["name"][:18] if len(cp["name"]) > 18 else cp["name"]
        created = cp["created"][:19] if cp["created"] else "unknown"
        tokens = format_tokens(cp.get("tokens", 0))

        lines.append(f"{name:<20} {created:<20} {tokens:>10}")

    return "\n".join(lines)
