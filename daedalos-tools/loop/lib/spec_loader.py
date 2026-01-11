"""
Spec loader for loop tool - automatically loads relevant specs before starting a loop.

The key insight: agents work better with rich context. When starting a loop,
we parse the task description, find relevant specs, and inject their intent,
interface, and anti-patterns into the agent's context.
"""

import subprocess
import re
from pathlib import Path
from typing import Optional, List, Dict, Any


def find_spec_tool() -> Optional[Path]:
    """Find the spec tool binary."""
    # Check common locations
    locations = [
        Path.home() / ".local/bin/spec",
        Path("/usr/local/bin/spec"),
        Path(__file__).parent.parent.parent / "spec/bin/spec",
    ]

    for loc in locations:
        if loc.exists() and loc.is_file():
            return loc

    # Try PATH
    try:
        result = subprocess.run(
            ["which", "spec"],
            capture_output=True,
            text=True
        )
        if result.returncode == 0:
            return Path(result.stdout.strip())
    except Exception:
        pass

    return None


def extract_components_from_task(task: str) -> List[str]:
    """
    Extract likely component names from a task description.

    Uses simple heuristics:
    - Known Daedalos tool names
    - Words that look like component names (lowercase, no spaces)
    """
    known_components = [
        "undo", "loop", "verify", "project", "codex", "context",
        "scratch", "agent", "sandbox", "mcp", "lsp", "error",
        "gates", "journal", "observe", "spec",
        "env", "notify", "session", "secrets", "pair", "handoff",
        "review", "focus", "metrics", "template", "container",
        "remote", "backup"
    ]

    task_lower = task.lower()
    found = []

    # Check for known components
    for component in known_components:
        if component in task_lower:
            found.append(component)

    # Also look for patterns like "fix the X command" or "add to X"
    patterns = [
        r"fix (?:the )?(\w+)",
        r"add (?:to )?(\w+)",
        r"update (?:the )?(\w+)",
        r"modify (?:the )?(\w+)",
        r"implement (\w+)",
        r"(\w+) tool",
        r"(\w+) command",
    ]

    for pattern in patterns:
        matches = re.findall(pattern, task_lower)
        for match in matches:
            if match in known_components and match not in found:
                found.append(match)

    return found


def load_spec_context(task: str, working_dir: Path) -> Optional[str]:
    """
    Load relevant spec context for a task.

    Returns formatted context string or None if no specs found.
    """
    spec_tool = find_spec_tool()
    if not spec_tool:
        return None

    # Try spec context command first
    try:
        result = subprocess.run(
            [str(spec_tool), "context", task],
            capture_output=True,
            text=True,
            cwd=str(working_dir),
            timeout=10
        )

        if result.returncode == 0 and result.stdout.strip():
            return result.stdout.strip()
    except Exception:
        pass

    # Fall back to loading specific components
    components = extract_components_from_task(task)
    if not components:
        return None

    context_parts = []

    for component in components:
        try:
            # Get intent
            intent_result = subprocess.run(
                [str(spec_tool), "show", component, "--section", "intent"],
                capture_output=True,
                text=True,
                cwd=str(working_dir),
                timeout=5
            )

            if intent_result.returncode == 0:
                context_parts.append(f"## {component}\n\n### Intent\n{intent_result.stdout.strip()}")

            # Get anti-patterns
            anti_result = subprocess.run(
                [str(spec_tool), "show", component, "--section", "anti_patterns"],
                capture_output=True,
                text=True,
                cwd=str(working_dir),
                timeout=5
            )

            if anti_result.returncode == 0 and "not found" not in anti_result.stderr.lower():
                context_parts.append(f"### Anti-patterns (AVOID)\n{anti_result.stdout.strip()}")

        except Exception:
            continue

    if context_parts:
        header = "# Relevant Specifications\n\n"
        header += "The following specs are relevant to your task. Follow the intent and avoid the anti-patterns.\n\n"
        return header + "\n\n".join(context_parts)

    return None


def format_spec_for_agent(spec_content: str) -> str:
    """
    Format spec content for injection into agent prompt.
    """
    return f"""
<specifications>
{spec_content}
</specifications>

Use the above specifications to guide your work. Pay special attention to:
- The INTENT section explains WHY this component exists
- The ANTI-PATTERNS section lists things you should NOT do
- Follow the interface contracts described in the specs
"""


def get_spec_context_for_loop(task: str, working_dir: Path) -> Optional[str]:
    """
    Main entry point: get formatted spec context for a loop task.

    Returns formatted context string ready for agent injection, or None.
    """
    context = load_spec_context(task, working_dir)

    if context:
        return format_spec_for_agent(context)

    return None
