"""Journal narrative builder - synthesizes events into readable stories."""

from datetime import datetime, timedelta
from typing import List, Dict, Any, Optional
from collections import defaultdict

try:
    from .collector import Event, collect_all_events
except ImportError:
    from collector import Event, collect_all_events


def format_time(ts: float) -> str:
    """Format timestamp for display."""
    dt = datetime.fromtimestamp(ts)
    now = datetime.now()

    if dt.date() == now.date():
        return dt.strftime("%H:%M:%S")
    elif dt.date() == (now - timedelta(days=1)).date():
        return "Yesterday " + dt.strftime("%H:%M")
    else:
        return dt.strftime("%Y-%m-%d %H:%M")


def format_duration(seconds: float) -> str:
    """Format duration for display."""
    if seconds < 60:
        return f"{int(seconds)}s"
    elif seconds < 3600:
        return f"{int(seconds // 60)}m {int(seconds % 60)}s"
    else:
        hours = int(seconds // 3600)
        minutes = int((seconds % 3600) // 60)
        return f"{hours}h {minutes}m"


def format_relative_time(ts: float) -> str:
    """Format timestamp as relative time."""
    now = datetime.now()
    dt = datetime.fromtimestamp(ts)
    delta = now - dt

    if delta.total_seconds() < 60:
        return "just now"
    elif delta.total_seconds() < 3600:
        mins = int(delta.total_seconds() / 60)
        return f"{mins} minute{'s' if mins != 1 else ''} ago"
    elif delta.total_seconds() < 86400:
        hours = int(delta.total_seconds() / 3600)
        return f"{hours} hour{'s' if hours != 1 else ''} ago"
    else:
        days = delta.days
        return f"{days} day{'s' if days != 1 else ''} ago"


def build_narrative(
    events: List[Event],
    verbose: bool = False,
) -> str:
    """
    Build a human-readable narrative from events.

    Args:
        events: List of events (should already be sorted newest-first)
        verbose: Include more details

    Returns:
        Formatted narrative string
    """
    if not events:
        return "No events found in the specified time range."

    lines = []

    # Group events by time period
    now = datetime.now()
    today_events = []
    yesterday_events = []
    older_events = []

    for event in events:
        dt = datetime.fromtimestamp(event.timestamp)
        if dt.date() == now.date():
            today_events.append(event)
        elif dt.date() == (now - timedelta(days=1)).date():
            yesterday_events.append(event)
        else:
            older_events.append(event)

    # Build narrative sections
    if today_events:
        lines.append("## Today")
        lines.append("")
        lines.extend(_format_event_group(today_events, verbose))

    if yesterday_events:
        lines.append("")
        lines.append("## Yesterday")
        lines.append("")
        lines.extend(_format_event_group(yesterday_events, verbose))

    if older_events:
        lines.append("")
        lines.append("## Earlier")
        lines.append("")
        lines.extend(_format_event_group(older_events, verbose))

    return "\n".join(lines)


def _format_event_group(events: List[Event], verbose: bool) -> List[str]:
    """Format a group of events."""
    lines = []

    for event in events:
        time_str = format_time(event.timestamp)
        icon = _get_icon(event.source, event.event_type)

        if verbose:
            lines.append(f"{time_str}  {icon} [{event.source}] {event.summary}")
            if event.details:
                for key, value in event.details.items():
                    if key not in ("timestamp", "source", "event_type", "summary"):
                        lines.append(f"           {key}: {value}")
        else:
            lines.append(f"{time_str}  {icon} {event.summary}")

    return lines


def _get_icon(source: str, event_type: str) -> str:
    """Get an icon for an event type."""
    icons = {
        # Sources
        "gates": "🚧",
        "loop": "🔄",
        "agent": "🤖",
        "undo": "↩️",
        "mcp-hub": "🔌",

        # Event types
        "gate_check": "🚧",
        "loop_started": "▶️",
        "loop_completed": "✅",
        "loop_failed": "❌",
        "loop_stopped": "⏹️",
        "agent_spawned": "🤖",
        "agent_killed": "💀",
        "file_created": "📄",
        "file_modified": "✏️",
        "file_deleted": "🗑️",
        "checkpoint": "📌",
    }

    return icons.get(event_type, icons.get(source, "•"))


def build_summary(
    events: List[Event],
) -> Dict[str, Any]:
    """
    Build a summary of events.

    Returns:
        Dictionary with summary statistics
    """
    summary = {
        "total_events": len(events),
        "time_range": {
            "start": None,
            "end": None,
        },
        "by_source": defaultdict(int),
        "by_type": defaultdict(int),
        "highlights": [],
    }

    if not events:
        return summary

    # Calculate stats
    timestamps = [e.timestamp for e in events]
    summary["time_range"]["start"] = format_time(min(timestamps))
    summary["time_range"]["end"] = format_time(max(timestamps))

    for event in events:
        summary["by_source"][event.source] += 1
        summary["by_type"][event.event_type] += 1

    # Identify highlights
    highlights = []

    # Check for loops
    loop_events = [e for e in events if e.source == "loop"]
    if loop_events:
        completed = len([e for e in loop_events if e.event_type == "loop_completed"])
        failed = len([e for e in loop_events if e.event_type == "loop_failed"])
        if completed:
            highlights.append(f"{completed} loop(s) completed")
        if failed:
            highlights.append(f"{failed} loop(s) failed")

    # Check for gate denials
    gate_events = [e for e in events if e.source == "gates"]
    denied = len([e for e in gate_events if "denied" in e.summary.lower()])
    if denied:
        highlights.append(f"{denied} gate check(s) denied")

    # Check for agents
    agent_events = [e for e in events if e.source == "agent"]
    if agent_events:
        highlights.append(f"{len(agent_events)} agent event(s)")

    # Check for file changes
    file_events = [e for e in events if e.source == "undo"]
    if file_events:
        checkpoints = len([e for e in file_events if e.event_type == "checkpoint"])
        changes = len(file_events) - checkpoints
        if changes:
            highlights.append(f"{changes} file change(s)")
        if checkpoints:
            highlights.append(f"{checkpoints} checkpoint(s)")

    summary["highlights"] = highlights
    summary["by_source"] = dict(summary["by_source"])
    summary["by_type"] = dict(summary["by_type"])

    return summary


def what_happened(
    hours: float = 1,
    sources: Optional[List[str]] = None,
    verbose: bool = False,
) -> str:
    """
    Answer "what happened?" for a time period.

    Args:
        hours: How many hours to look back
        sources: Filter by sources
        verbose: Include more details

    Returns:
        Human-readable narrative
    """
    events = collect_all_events(hours=hours, sources=sources)

    if not events:
        period = f"the last {hours} hour{'s' if hours != 1 else ''}"
        return f"Nothing recorded in {period}."

    summary = build_summary(events)

    lines = []

    # Header with summary
    lines.append(f"## What happened in the last {hours} hour{'s' if hours != 1 else ''}?")
    lines.append("")

    if summary["highlights"]:
        lines.append("**Highlights:** " + " • ".join(summary["highlights"]))
        lines.append("")

    lines.append(f"**{summary['total_events']} events** from {summary['time_range']['start']} to {summary['time_range']['end']}")
    lines.append("")

    # Activity by source
    if summary["by_source"]:
        sources_str = ", ".join(f"{k}: {v}" for k, v in sorted(summary["by_source"].items(), key=lambda x: -x[1]))
        lines.append(f"By source: {sources_str}")
        lines.append("")

    # Detailed narrative
    lines.append("---")
    lines.append("")
    lines.append(build_narrative(events, verbose=verbose))

    return "\n".join(lines)
