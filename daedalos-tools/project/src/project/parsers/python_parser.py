"""Python language parser."""

import re
from typing import List

from .base import BaseParser, ParseResult, Symbol, Dependency


class PythonParser(BaseParser):
    """Parser for Python files."""

    @classmethod
    def extensions(cls) -> List[str]:
        return [".py", ".pyi"]

    def parse(self, content: str) -> ParseResult:
        symbols = []
        dependencies = []
        lines = content.split("\n")

        current_class = None

        for i, line in enumerate(lines, 1):
            stripped = line.strip()
            indent = len(line) - len(line.lstrip())

            # Skip comments and docstrings
            if stripped.startswith("#"):
                continue

            # Imports
            if match := re.match(r"from\s+([\w.]+)\s+import", stripped):
                dependencies.append(Dependency(
                    target_path=match.group(1),
                    import_type="import"
                ))
            elif match := re.match(r"import\s+([\w.]+)", stripped):
                dependencies.append(Dependency(
                    target_path=match.group(1),
                    import_type="import"
                ))

            # Classes
            if match := re.match(r"class\s+(\w+)", stripped):
                current_class = match.group(1)
                # Check for visibility (underscore prefix)
                visibility = "private" if match.group(1).startswith("_") else "public"
                symbols.append(Symbol(
                    name=match.group(1),
                    type="class",
                    line_start=i,
                    line_end=self._find_python_block_end(lines, i, indent),
                    signature=self._clean_signature(stripped),
                    visibility=visibility
                ))

            # Functions and methods
            elif match := re.match(r"(?:async\s+)?def\s+(\w+)", stripped):
                name = match.group(1)
                # Determine type and visibility
                if indent > 0 and current_class:
                    sym_type = "method"
                else:
                    sym_type = "function"
                    current_class = None

                if name.startswith("__") and name.endswith("__"):
                    visibility = "public"  # Dunder methods
                elif name.startswith("_"):
                    visibility = "private"
                else:
                    visibility = "public"

                symbols.append(Symbol(
                    name=name,
                    type=sym_type,
                    line_start=i,
                    line_end=self._find_python_block_end(lines, i, indent),
                    signature=self._clean_signature(stripped),
                    visibility=visibility
                ))

            # Decorators (capture for context)
            elif stripped.startswith("@") and not stripped.startswith("@@"):
                # Could track decorators if needed
                pass

            # Reset class context at module level
            if indent == 0 and not stripped.startswith("class "):
                if not stripped.startswith("def ") and stripped and not stripped.startswith("@"):
                    current_class = None

        return ParseResult(
            file_type="python",
            symbols=symbols,
            dependencies=dependencies
        )

    def _find_python_block_end(self, lines: List[str], start: int, start_indent: int) -> int:
        """Find end of Python block based on indentation."""
        for i in range(start, len(lines)):
            line = lines[i]
            if not line.strip():  # Empty line
                continue
            if line.strip().startswith("#"):  # Comment
                continue

            current_indent = len(line) - len(line.lstrip())

            # If we find a line with same or less indentation (and it's not empty/comment)
            # then the block ended on the previous line
            if i > start and current_indent <= start_indent and line.strip():
                return i

        return len(lines)
