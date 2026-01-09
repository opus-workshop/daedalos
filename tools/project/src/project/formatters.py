"""Output formatters for project commands."""

import json
from typing import Dict, Any, List


class SummaryFormatter:
    """Format project summary output."""

    def __init__(self, as_json: bool = False, brief: bool = False, no_color: bool = False):
        self.as_json = as_json
        self.brief = brief
        self.no_color = no_color

    def format(self, summary: Dict[str, Any]) -> str:
        """Format summary for output."""
        if self.as_json:
            return json.dumps(summary, indent=2)

        if self.brief:
            return self._format_brief(summary)

        return self._format_box(summary)

    def _format_brief(self, summary: Dict[str, Any]) -> str:
        """Format one-line brief summary."""
        arch = summary.get("architecture", {}).get("description", "Unknown")
        return f"{summary['name']}: {summary['type']} ({arch})"

    def _format_box(self, summary: Dict[str, Any]) -> str:
        """Format full box summary."""
        width = 65
        lines = []

        # Header
        lines.append("+" + "-" * width + "+")
        title = f" PROJECT SUMMARY: {summary['name']} "
        lines.append("|" + title.center(width) + "|")
        lines.append("+" + "-" * width + "+")

        # Type and architecture
        arch = summary.get("architecture", {})
        lines.append(f"| Type: {summary['type']:<{width-8}} |")
        lines.append(f"| Architecture: {arch.get('description', 'Unknown'):<{width-16}} |")

        # Entry points
        entry_points = summary.get("entry_points", [])
        if entry_points:
            entry = " -> ".join(entry_points[:3])
            if len(entry) > width - 9:
                entry = entry[:width-12] + "..."
            lines.append(f"| Entry: {entry:<{width-9}} |")

        lines.append("|" + " " * width + "|")

        # Key modules
        modules = summary.get("modules", [])
        if modules:
            lines.append("| Key Modules:" + " " * (width - 13) + "|")
            for module in modules[:5]:
                line = f"  - {module['name']}: {module['description']}"
                if len(line) > width - 2:
                    line = line[:width-5] + "..."
                lines.append(f"|{line:<{width}}|")

        lines.append("|" + " " * width + "|")

        # Dependencies
        deps = summary.get("dependencies", [])
        if deps:
            dep_str = ", ".join(deps[:5])
            if len(dep_str) > width - 16:
                dep_str = dep_str[:width-19] + "..."
            lines.append(f"| Dependencies: {dep_str:<{width-16}} |")

        # Conventions
        conventions = summary.get("conventions", [])
        if conventions:
            lines.append("|" + " " * width + "|")
            lines.append("| Conventions:" + " " * (width - 13) + "|")
            for conv in conventions[:3]:
                line = f"  - {conv}"
                if len(line) > width - 2:
                    line = line[:width-5] + "..."
                lines.append(f"|{line:<{width}}|")

        # Stats
        stats = summary.get("stats", {})
        if stats:
            lines.append("|" + " " * width + "|")
            stats_str = f"Files: {stats.get('files', 0)} | Symbols: {stats.get('symbols', 0)}"
            lines.append(f"| {stats_str:<{width-2}} |")

        # Footer
        lines.append("+" + "-" * width + "+")

        return "\n".join(lines)


class TreeFormatter:
    """Format directory tree output."""

    def __init__(self, max_depth: int = 3, no_color: bool = False):
        self.max_depth = max_depth
        self.no_color = no_color

    def format(self, files: List[Dict[str, Any]]) -> str:
        """Format file list as tree."""
        # Build tree structure
        tree = {}
        for f in files:
            parts = f["path"].split("/")
            current = tree
            for part in parts:
                if part not in current:
                    current[part] = {}
                current = current[part]

        return self._render_tree(tree, "", 0)

    def _render_tree(self, tree: dict, prefix: str, depth: int) -> str:
        """Recursively render tree."""
        if depth >= self.max_depth:
            return ""

        lines = []
        items = sorted(tree.items())

        for i, (name, children) in enumerate(items):
            is_last = i == len(items) - 1
            connector = "\\-- " if is_last else "|-- "
            lines.append(f"{prefix}{connector}{name}")

            if children:
                next_prefix = prefix + ("    " if is_last else "|   ")
                subtree = self._render_tree(children, next_prefix, depth + 1)
                if subtree:
                    lines.append(subtree)

        return "\n".join(lines)


class MapFormatter:
    """Format dependency map output."""

    def __init__(self, format_type: str = "tree", no_color: bool = False):
        self.format_type = format_type
        self.no_color = no_color

    def format(self, deps: List[Dict[str, Any]]) -> str:
        """Format dependencies."""
        if self.format_type == "json":
            return json.dumps(deps, indent=2)
        elif self.format_type == "dot":
            return self._format_dot(deps)
        else:
            return self._format_tree(deps)

    def _format_tree(self, deps: List[Dict[str, Any]]) -> str:
        """Format as text tree."""
        lines = []
        for dep in deps:
            lines.append(f"{dep['source']} -> {dep['target']}")
        return "\n".join(lines) if lines else "No dependencies found"

    def _format_dot(self, deps: List[Dict[str, Any]]) -> str:
        """Format as GraphViz DOT."""
        lines = ["digraph deps {"]
        for dep in deps:
            source = dep["source"].replace("/", "_").replace(".", "_")
            target = dep["target"].replace("/", "_").replace(".", "_")
            lines.append(f'  "{source}" -> "{target}";')
        lines.append("}")
        return "\n".join(lines)


class StatsFormatter:
    """Format statistics output."""

    def __init__(self, as_json: bool = False, no_color: bool = False):
        self.as_json = as_json
        self.no_color = no_color

    def format(self, stats: Dict[str, Any]) -> str:
        """Format stats for output."""
        if self.as_json:
            return json.dumps(stats, indent=2)

        lines = ["Project Statistics", "=" * 40]

        lines.append(f"Total Files: {stats.get('files', 0)}")
        lines.append(f"Total Symbols: {stats.get('symbols', 0)}")
        lines.append(f"Total Dependencies: {stats.get('dependencies', 0)}")

        lines_by_type = stats.get("lines_by_type", {})
        if lines_by_type:
            lines.append("")
            lines.append("Lines by Type:")
            for file_type, count in sorted(lines_by_type.items(), key=lambda x: -x[1]):
                lines.append(f"  {file_type}: {count:,}")

        return "\n".join(lines)
