"""Context tracking for Claude Code sessions."""

import json
import os
from pathlib import Path
from typing import Dict, Any, List, Optional
from datetime import datetime

from .estimator import TokenEstimator


class ContextTracker:
    """Track and analyze Claude Code context usage."""

    # Claude's context window sizes
    CONTEXT_SIZES = {
        "claude-3-opus": 200000,
        "claude-3-sonnet": 200000,
        "claude-3-haiku": 200000,
        "default": 200000,
    }

    def __init__(self, project_path: Optional[str] = None):
        """Initialize tracker.

        Args:
            project_path: Path to project, auto-detected if None
        """
        self.estimator = TokenEstimator()
        self.claude_dir = Path.home() / ".claude"
        self.project_path = Path(project_path) if project_path else self._detect_project()
        self.max_context = self.CONTEXT_SIZES["default"]

    def _detect_project(self) -> Optional[Path]:
        """Detect current project from Claude Code state."""
        # Try current directory first
        cwd = Path.cwd()
        if (cwd / ".git").exists():
            return cwd

        # Check Claude's project directories
        projects_dir = self.claude_dir / "projects"
        if projects_dir.exists():
            # Find most recently modified project
            latest = None
            latest_time = 0
            for p in projects_dir.iterdir():
                if p.is_dir():
                    mtime = p.stat().st_mtime
                    if mtime > latest_time:
                        latest_time = mtime
                        latest = p

            if latest:
                return latest

        return cwd

    def _find_history(self) -> Optional[Path]:
        """Find conversation history file."""
        # Check various possible locations
        locations = [
            self.claude_dir / "conversation.jsonl",
            self.claude_dir / "history.jsonl",
        ]

        if self.project_path:
            project_id = self.project_path.name
            locations.extend([
                self.claude_dir / "projects" / project_id / "conversation.jsonl",
                self.claude_dir / "projects" / project_id / "history.jsonl",
            ])

        for loc in locations:
            if loc.exists():
                return loc

        return None

    def get_conversation_tokens(self) -> Dict[str, int]:
        """Analyze conversation and return token breakdown."""
        tokens = {
            "system": 0,
            "user": 0,
            "assistant": 0,
            "tool_calls": 0,
            "tool_results": 0,
            "files_read": 0,
        }

        history_file = self._find_history()
        if not history_file:
            # Estimate based on typical session
            return self._estimate_current_session()

        try:
            with open(history_file) as f:
                for line in f:
                    try:
                        entry = json.loads(line.strip())
                        role = entry.get("role", "unknown")
                        content = entry.get("content", "")

                        if isinstance(content, str):
                            count = self.estimator.count(content)
                        elif isinstance(content, list):
                            # Handle structured content
                            count = sum(
                                self.estimator.count(str(c.get("text", "")))
                                for c in content if isinstance(c, dict)
                            )
                        else:
                            count = 0

                        # Categorize by role
                        if role == "system":
                            tokens["system"] += count
                        elif role == "user":
                            tokens["user"] += count
                        elif role == "assistant":
                            tokens["assistant"] += count
                        elif role in ["tool", "tool_result"]:
                            tokens["tool_results"] += count

                        # Check for tool calls in assistant messages
                        if role == "assistant" and "tool_calls" in entry:
                            for tc in entry.get("tool_calls", []):
                                tokens["tool_calls"] += self.estimator.count(
                                    json.dumps(tc)
                                )

                    except json.JSONDecodeError:
                        continue

        except Exception:
            return self._estimate_current_session()

        return tokens

    def _estimate_current_session(self) -> Dict[str, int]:
        """Estimate tokens for current session when history unavailable."""
        # Base estimates for a typical session
        return {
            "system": 8000,  # System prompt + instructions
            "user": 5000,    # Estimated user messages
            "assistant": 15000,  # Estimated responses
            "tool_calls": 3000,
            "tool_results": 20000,
            "files_read": 10000,
        }

    def get_status(self) -> Dict[str, Any]:
        """Get current context status."""
        tokens = self.get_conversation_tokens()
        total = sum(tokens.values())
        percentage = (total / self.max_context) * 100

        return {
            "used": total,
            "max": self.max_context,
            "percentage": percentage,
            "remaining": self.max_context - total,
            "breakdown": tokens,
            "warning_level": self._get_warning_level(percentage),
        }

    def _get_warning_level(self, percentage: float) -> str:
        """Determine warning level based on usage percentage."""
        if percentage < 50:
            return "ok"
        elif percentage < 70:
            return "moderate"
        elif percentage < 85:
            return "high"
        else:
            return "critical"

    def get_files_in_context(self) -> List[Dict[str, Any]]:
        """Get list of files that have been read into context."""
        files = []

        history_file = self._find_history()
        if not history_file:
            return files

        try:
            with open(history_file) as f:
                for line in f:
                    try:
                        entry = json.loads(line.strip())
                        if entry.get("role") == "tool_result":
                            # Check if this is a file read result
                            content = entry.get("content", "")
                            tool_name = entry.get("name", "")

                            if tool_name in ["Read", "read_file", "cat"]:
                                # Extract file path from tool call
                                file_path = entry.get("file_path", "unknown")
                                tokens = self.estimator.count(content)
                                files.append({
                                    "path": file_path,
                                    "tokens": tokens,
                                    "size": len(content),
                                })
                    except json.JSONDecodeError:
                        continue

        except Exception:
            pass

        # Sort by token count descending
        return sorted(files, key=lambda x: x["tokens"], reverse=True)

    def get_compaction_suggestions(self) -> List[Dict[str, Any]]:
        """Get suggestions for reducing context usage."""
        suggestions = []
        status = self.get_status()

        # Large files suggestion
        files = self.get_files_in_context()
        large_files = [f for f in files if f["tokens"] > 5000]
        if large_files:
            savings = sum(f["tokens"] for f in large_files[:3])
            suggestions.append({
                "type": "large_files",
                "description": f"Consider re-reading only relevant sections of large files",
                "savings": int(savings * 0.7),
                "files": large_files[:3],
            })

        # Tool results suggestion
        if status["breakdown"].get("tool_results", 0) > 30000:
            suggestions.append({
                "type": "tool_results",
                "description": "Summarize verbose tool results instead of keeping full output",
                "savings": int(status["breakdown"]["tool_results"] * 0.5),
            })

        # Conversation history suggestion
        if status["percentage"] > 70:
            suggestions.append({
                "type": "conversation",
                "description": "Consider starting a new session with a summary",
                "savings": int(status["used"] * 0.6),
            })

        return suggestions

    def checkpoint(self, name: str) -> Dict[str, Any]:
        """Create a context checkpoint for later restoration."""
        checkpoint_dir = self.claude_dir / "checkpoints"
        checkpoint_dir.mkdir(parents=True, exist_ok=True)

        status = self.get_status()
        files = self.get_files_in_context()

        checkpoint_data = {
            "name": name,
            "created": datetime.now().isoformat(),
            "project": str(self.project_path) if self.project_path else None,
            "status": status,
            "files": files,
        }

        checkpoint_file = checkpoint_dir / f"{name}.json"
        with open(checkpoint_file, "w") as f:
            json.dump(checkpoint_data, f, indent=2)

        return checkpoint_data

    def list_checkpoints(self) -> List[Dict[str, Any]]:
        """List available checkpoints."""
        checkpoint_dir = self.claude_dir / "checkpoints"
        if not checkpoint_dir.exists():
            return []

        checkpoints = []
        for f in checkpoint_dir.glob("*.json"):
            try:
                with open(f) as fp:
                    data = json.load(fp)
                    checkpoints.append({
                        "name": data.get("name", f.stem),
                        "created": data.get("created", "unknown"),
                        "tokens": data.get("status", {}).get("used", 0),
                    })
            except Exception:
                continue

        return sorted(checkpoints, key=lambda x: x["created"], reverse=True)

    def restore_checkpoint(self, name: str) -> Optional[Dict[str, Any]]:
        """Restore from a checkpoint (returns checkpoint data)."""
        checkpoint_file = self.claude_dir / "checkpoints" / f"{name}.json"
        if not checkpoint_file.exists():
            return None

        with open(checkpoint_file) as f:
            return json.load(f)
