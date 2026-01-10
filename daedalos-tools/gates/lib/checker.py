"""Gate checking and approval flow."""

import os
import sys
import json
import time
from dataclasses import dataclass
from datetime import datetime
from pathlib import Path
from typing import Optional, Dict, Any, List

try:
    from .config import (
        SupervisionConfig,
        load_project_config,
        GATE_ACTIONS,
    )
except ImportError:
    from config import (
        SupervisionConfig,
        load_project_config,
        GATE_ACTIONS,
    )


# Data directory for gate history
DATA_DIR = Path(os.environ.get("XDG_DATA_HOME", Path.home() / ".local" / "share")) / "daedalos" / "gates"


@dataclass
class GateRequest:
    """A request to pass through a gate."""

    gate: str
    action: str
    context: Dict[str, Any]
    timestamp: float
    source: str  # Which tool is requesting


@dataclass
class GateResult:
    """Result of a gate check."""

    allowed: bool
    action: str  # What action was taken (allow, notify, approve, deny)
    reason: str
    approved_by: Optional[str] = None  # "auto", "user", "timeout"


def ensure_data_dir():
    """Ensure data directory exists."""
    DATA_DIR.mkdir(parents=True, exist_ok=True)


def log_gate_event(request: GateRequest, result: GateResult) -> None:
    """Log a gate event for history/audit."""
    ensure_data_dir()

    event = {
        "timestamp": request.timestamp,
        "gate": request.gate,
        "action": request.action,
        "context": request.context,
        "source": request.source,
        "result": {
            "allowed": result.allowed,
            "action": result.action,
            "reason": result.reason,
            "approved_by": result.approved_by,
        }
    }

    # Append to daily log file
    date_str = datetime.fromtimestamp(request.timestamp).strftime("%Y-%m-%d")
    log_file = DATA_DIR / f"gates-{date_str}.jsonl"

    with open(log_file, "a") as f:
        f.write(json.dumps(event) + "\n")


def check_gate(
    gate: str,
    context: Optional[Dict[str, Any]] = None,
    source: str = "unknown",
    config: Optional[SupervisionConfig] = None,
    interactive: bool = True,
) -> GateResult:
    """
    Check if an action is allowed through a gate.

    Args:
        gate: The gate name (e.g., "file_delete", "git_push")
        context: Additional context about the action
        source: Which tool is making the request
        config: Supervision config (loaded if not provided)
        interactive: Whether to prompt for approval if needed

    Returns:
        GateResult indicating if the action is allowed
    """
    if config is None:
        config = load_project_config()

    if context is None:
        context = {}

    # Check for sensitive path override
    if "path" in context and config.is_sensitive_path(context["path"]):
        gate_action = config.get_gate("sensitive_file")
    else:
        gate_action = config.get_gate(gate)

    request = GateRequest(
        gate=gate,
        action=gate_action,
        context=context,
        timestamp=time.time(),
        source=source,
    )

    if gate_action == "allow":
        result = GateResult(
            allowed=True,
            action="allow",
            reason="Gate configured to allow",
            approved_by="auto",
        )

    elif gate_action == "notify":
        # Notify but don't block
        notify_user(request)
        result = GateResult(
            allowed=True,
            action="notify",
            reason="User notified, proceeding",
            approved_by="auto",
        )

    elif gate_action == "deny":
        result = GateResult(
            allowed=False,
            action="deny",
            reason="Gate configured to deny",
            approved_by="auto",
        )

    elif gate_action == "approve":
        if interactive:
            approved = prompt_for_approval(request)
            result = GateResult(
                allowed=approved,
                action="approve",
                reason="User approved" if approved else "User denied",
                approved_by="user" if approved else None,
            )
        else:
            # Non-interactive mode - check for pre-approval or deny
            result = GateResult(
                allowed=False,
                action="approve",
                reason="Approval required but running non-interactively",
                approved_by=None,
            )

    else:
        # Unknown action, default to approve
        result = GateResult(
            allowed=False,
            action="approve",
            reason=f"Unknown gate action: {gate_action}",
            approved_by=None,
        )

    # Log the event
    log_gate_event(request, result)

    return result


def notify_user(request: GateRequest) -> None:
    """Send a notification to the user (non-blocking)."""
    # Format the notification
    msg = format_gate_message(request, is_notification=True)

    # Print to stderr so it's visible but doesn't interfere with stdout
    print(f"\033[33m[gates]\033[0m {msg}", file=sys.stderr)

    # Could also send to system notification daemon, tmux, etc.
    # For now, just print


def prompt_for_approval(request: GateRequest) -> bool:
    """Prompt user for approval (blocking)."""
    msg = format_gate_message(request, is_notification=False)

    # Check if we're in a terminal
    if not sys.stdin.isatty():
        print(f"\033[31m[gates]\033[0m Approval needed but no TTY: {msg}", file=sys.stderr)
        return False

    print(f"\n\033[33m[gates]\033[0m {msg}", file=sys.stderr)
    print(f"\033[33m[gates]\033[0m Allow this action? [y/N] ", file=sys.stderr, end="")

    try:
        response = input().strip().lower()
        return response in ("y", "yes")
    except (EOFError, KeyboardInterrupt):
        print("", file=sys.stderr)
        return False


def format_gate_message(request: GateRequest, is_notification: bool = False) -> str:
    """Format a gate request for display."""
    parts = [f"Gate: {request.gate}"]

    if request.source != "unknown":
        parts.append(f"Source: {request.source}")

    # Add relevant context
    ctx = request.context
    if "path" in ctx:
        parts.append(f"Path: {ctx['path']}")
    if "command" in ctx:
        cmd = ctx["command"]
        if len(cmd) > 60:
            cmd = cmd[:57] + "..."
        parts.append(f"Command: {cmd}")
    if "description" in ctx:
        parts.append(f"Action: {ctx['description']}")

    prefix = "Notification" if is_notification else "Approval required"
    return f"{prefix} - " + " | ".join(parts)


def get_gate_history(
    gate: Optional[str] = None,
    days: int = 7,
    limit: int = 100,
) -> List[Dict[str, Any]]:
    """Get gate check history."""
    ensure_data_dir()

    events = []
    now = datetime.now()

    for i in range(days):
        date = now.replace(hour=0, minute=0, second=0, microsecond=0)
        date_str = date.strftime("%Y-%m-%d")
        log_file = DATA_DIR / f"gates-{date_str}.jsonl"

        if log_file.exists():
            with open(log_file) as f:
                for line in f:
                    try:
                        event = json.loads(line)
                        if gate is None or event.get("gate") == gate:
                            events.append(event)
                    except json.JSONDecodeError:
                        pass

        now = now.replace(day=now.day - 1)

    # Sort by timestamp descending and limit
    events.sort(key=lambda x: x.get("timestamp", 0), reverse=True)
    return events[:limit]


def check_autonomy_limits(
    config: SupervisionConfig,
    iterations: int = 0,
    file_changes: int = 0,
    lines_changed: int = 0,
) -> Optional[str]:
    """
    Check if we've exceeded autonomy limits.

    Returns a reason string if limits exceeded, None otherwise.
    """
    limits = config.autonomy

    if iterations > limits.get("max_iterations", float("inf")):
        return f"Exceeded max iterations ({iterations}/{limits['max_iterations']})"

    if file_changes > limits.get("max_file_changes", float("inf")):
        return f"Exceeded max file changes ({file_changes}/{limits['max_file_changes']})"

    if lines_changed > limits.get("max_lines_changed", float("inf")):
        return f"Exceeded max lines changed ({lines_changed}/{limits['max_lines_changed']})"

    return None
