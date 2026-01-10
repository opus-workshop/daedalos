"""Journal event collector - aggregates events from all Daedalos tools."""

import os
import json
import glob
from dataclasses import dataclass, field
from datetime import datetime, timedelta
from pathlib import Path
from typing import Optional, List, Dict, Any, Iterator


# Data directories
DATA_DIR = Path(os.environ.get("XDG_DATA_HOME", Path.home() / ".local" / "share")) / "daedalos"
JOURNAL_DIR = DATA_DIR / "journal"


@dataclass
class Event:
    """A single event in the journal."""
    timestamp: float
    source: str  # Which tool/component generated the event
    event_type: str  # Type of event (e.g., "loop_started", "file_changed", "gate_check")
    summary: str  # Human-readable summary
    details: Dict[str, Any] = field(default_factory=dict)

    def to_dict(self) -> Dict[str, Any]:
        return {
            "timestamp": self.timestamp,
            "source": self.source,
            "event_type": self.event_type,
            "summary": self.summary,
            "details": self.details,
        }

    @classmethod
    def from_dict(cls, data: Dict[str, Any]) -> "Event":
        return cls(
            timestamp=data.get("timestamp", 0),
            source=data.get("source", "unknown"),
            event_type=data.get("event_type", "unknown"),
            summary=data.get("summary", ""),
            details=data.get("details", {}),
        )


def ensure_journal_dir():
    """Ensure journal directory exists."""
    JOURNAL_DIR.mkdir(parents=True, exist_ok=True)


def collect_gate_events(since: float) -> Iterator[Event]:
    """Collect gate check events."""
    gates_dir = DATA_DIR / "gates"
    if not gates_dir.exists():
        return

    for log_file in sorted(gates_dir.glob("gates-*.jsonl")):
        try:
            with open(log_file) as f:
                for line in f:
                    try:
                        data = json.loads(line)
                        ts = data.get("timestamp", 0)
                        if ts < since:
                            continue

                        result = data.get("result", {})
                        allowed = "allowed" if result.get("allowed") else "denied"
                        gate = data.get("gate", "unknown")
                        source = data.get("source", "unknown")

                        yield Event(
                            timestamp=ts,
                            source="gates",
                            event_type="gate_check",
                            summary=f"Gate '{gate}' {allowed} (from {source})",
                            details=data,
                        )
                    except json.JSONDecodeError:
                        pass
        except Exception:
            pass


def collect_loop_events(since: float) -> Iterator[Event]:
    """Collect loop iteration events."""
    loop_dir = DATA_DIR / "loop"
    if not loop_dir.exists():
        return

    for log_file in sorted(loop_dir.glob("*.json")):
        try:
            with open(log_file) as f:
                data = json.load(f)

            ts = data.get("started", 0)
            if ts < since:
                continue

            task = data.get("task", "unknown task")
            status = data.get("status", "unknown")
            iterations = data.get("iteration", 0)

            yield Event(
                timestamp=ts,
                source="loop",
                event_type="loop_started",
                summary=f"Loop started: {task}",
                details=data,
            )

            # Add end event if completed
            if status in ("completed", "failed", "stopped"):
                ended = data.get("ended", ts)
                yield Event(
                    timestamp=ended,
                    source="loop",
                    event_type=f"loop_{status}",
                    summary=f"Loop {status} after {iterations} iterations: {task}",
                    details=data,
                )
        except Exception:
            pass


def collect_agent_events(since: float) -> Iterator[Event]:
    """Collect agent lifecycle events."""
    agent_dir = DATA_DIR / "agent"
    if not agent_dir.exists():
        return

    # Check for agent activity logs
    for log_file in sorted(agent_dir.glob("*.jsonl")):
        try:
            with open(log_file) as f:
                for line in f:
                    try:
                        data = json.loads(line)
                        ts = data.get("timestamp", 0)
                        if ts < since:
                            continue

                        event_type = data.get("event", "agent_event")
                        agent_name = data.get("name", "unknown")
                        summary = data.get("summary", f"Agent event: {agent_name}")

                        yield Event(
                            timestamp=ts,
                            source="agent",
                            event_type=event_type,
                            summary=summary,
                            details=data,
                        )
                    except json.JSONDecodeError:
                        pass
        except Exception:
            pass


def collect_undo_events(since: float) -> Iterator[Event]:
    """Collect file change events from undo timeline."""
    undo_dir = DATA_DIR / "undo"
    if not undo_dir.exists():
        return

    timeline_file = undo_dir / "timeline.jsonl"
    if not timeline_file.exists():
        return

    try:
        with open(timeline_file) as f:
            for line in f:
                try:
                    data = json.loads(line)
                    ts = data.get("timestamp", 0)
                    if ts < since:
                        continue

                    file_path = data.get("path", "unknown")
                    action = data.get("action", "modified")
                    checkpoint = data.get("checkpoint")

                    if checkpoint:
                        yield Event(
                            timestamp=ts,
                            source="undo",
                            event_type="checkpoint",
                            summary=f"Checkpoint created: {checkpoint}",
                            details=data,
                        )
                    else:
                        yield Event(
                            timestamp=ts,
                            source="undo",
                            event_type=f"file_{action}",
                            summary=f"File {action}: {file_path}",
                            details=data,
                        )
                except json.JSONDecodeError:
                    pass
    except Exception:
        pass


def collect_mcp_events(since: float) -> Iterator[Event]:
    """Collect MCP hub events."""
    mcp_dir = DATA_DIR / "mcp-hub"
    if not mcp_dir.exists():
        return

    for log_file in sorted(mcp_dir.glob("*.log")):
        try:
            stat = log_file.stat()
            if stat.st_mtime < since:
                continue

            # Basic parsing of log files
            with open(log_file) as f:
                for line in f:
                    if "started" in line.lower() or "stopped" in line.lower():
                        yield Event(
                            timestamp=stat.st_mtime,
                            source="mcp-hub",
                            event_type="mcp_event",
                            summary=line.strip()[:100],
                            details={"file": str(log_file)},
                        )
        except Exception:
            pass


def collect_journal_events(since: float) -> Iterator[Event]:
    """Collect events logged directly to the journal."""
    if not JOURNAL_DIR.exists():
        return

    for log_file in sorted(JOURNAL_DIR.glob("journal-*.jsonl")):
        try:
            with open(log_file) as f:
                for line in f:
                    try:
                        data = json.loads(line)
                        ts = data.get("timestamp", 0)
                        if ts < since:
                            continue

                        yield Event.from_dict(data)
                    except json.JSONDecodeError:
                        pass
        except Exception:
            pass


def collect_all_events(
    since: Optional[float] = None,
    hours: float = 24,
    sources: Optional[List[str]] = None,
    event_types: Optional[List[str]] = None,
    limit: int = 1000,
) -> List[Event]:
    """
    Collect events from all sources.

    Args:
        since: Unix timestamp to start from (overrides hours)
        hours: Number of hours to look back (default: 24)
        sources: Filter by source names (e.g., ["gates", "loop"])
        event_types: Filter by event types (e.g., ["gate_check", "file_modified"])
        limit: Maximum number of events

    Returns:
        List of events sorted by timestamp (newest first)
    """
    if since is None:
        since = (datetime.now() - timedelta(hours=hours)).timestamp()

    events = []

    # Collect from all sources
    collectors = {
        "gates": collect_gate_events,
        "loop": collect_loop_events,
        "agent": collect_agent_events,
        "undo": collect_undo_events,
        "mcp-hub": collect_mcp_events,
        "journal": collect_journal_events,
    }

    for source_name, collector in collectors.items():
        if sources and source_name not in sources:
            continue

        for event in collector(since):
            if event_types and event.event_type not in event_types:
                continue
            events.append(event)

    # Sort by timestamp descending (newest first)
    events.sort(key=lambda e: e.timestamp, reverse=True)

    return events[:limit]


def log_event(source: str, event_type: str, summary: str, details: Optional[Dict] = None) -> None:
    """
    Log an event to the journal.

    This can be called by other tools to record events.
    """
    ensure_journal_dir()

    event = Event(
        timestamp=datetime.now().timestamp(),
        source=source,
        event_type=event_type,
        summary=summary,
        details=details or {},
    )

    # Write to daily log file
    date_str = datetime.now().strftime("%Y-%m-%d")
    log_file = JOURNAL_DIR / f"journal-{date_str}.jsonl"

    with open(log_file, "a") as f:
        f.write(json.dumps(event.to_dict()) + "\n")
