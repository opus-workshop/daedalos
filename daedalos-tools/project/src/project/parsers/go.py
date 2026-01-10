"""Go language parser."""

import re
from typing import List

from .base import BaseParser, ParseResult, Symbol, Dependency


class GoParser(BaseParser):
    """Parser for Go files."""

    @classmethod
    def extensions(cls) -> List[str]:
        return [".go"]

    def parse(self, content: str) -> ParseResult:
        symbols = []
        dependencies = []
        lines = content.split("\n")

        in_import_block = False

        for i, line in enumerate(lines, 1):
            stripped = line.strip()

            # Skip comments
            if stripped.startswith("//") or stripped.startswith("/*"):
                continue

            # Import block start
            if stripped == "import (":
                in_import_block = True
                continue

            # Import block end
            if in_import_block and stripped == ")":
                in_import_block = False
                continue

            # Imports in block
            if in_import_block:
                if match := re.match(r'(?:\w+\s+)?"(.+?)"', stripped):
                    dependencies.append(Dependency(
                        target_path=match.group(1),
                        import_type="import"
                    ))
                continue

            # Single-line imports
            if match := re.match(r'import\s+(?:\w+\s+)?"(.+?)"', stripped):
                dependencies.append(Dependency(
                    target_path=match.group(1),
                    import_type="import"
                ))

            # Package declaration
            elif match := re.match(r"package\s+(\w+)", stripped):
                symbols.append(Symbol(
                    name=match.group(1),
                    type="package",
                    line_start=i,
                    line_end=i,
                    signature=stripped,
                    visibility="public"
                ))

            # Type definitions (struct, interface)
            elif match := re.match(r"type\s+(\w+)\s+(struct|interface)", stripped):
                name = match.group(1)
                type_ = match.group(2)
                # Go exports are capitalized
                visibility = "public" if name[0].isupper() else "private"
                symbols.append(Symbol(
                    name=name,
                    type=type_,
                    line_start=i,
                    line_end=self._find_block_end(lines, i),
                    signature=self._clean_signature(stripped),
                    visibility=visibility
                ))

            # Type alias
            elif match := re.match(r"type\s+(\w+)\s+=?\s*\w+", stripped):
                name = match.group(1)
                visibility = "public" if name[0].isupper() else "private"
                symbols.append(Symbol(
                    name=name,
                    type="type",
                    line_start=i,
                    line_end=i,
                    signature=self._clean_signature(stripped),
                    visibility=visibility
                ))

            # Functions
            elif match := re.match(r"func\s+(\w+)", stripped):
                name = match.group(1)
                visibility = "public" if name[0].isupper() else "private"
                symbols.append(Symbol(
                    name=name,
                    type="function",
                    line_start=i,
                    line_end=self._find_block_end(lines, i),
                    signature=self._clean_signature(stripped),
                    visibility=visibility
                ))

            # Methods (func with receiver)
            elif match := re.match(r"func\s+\([^)]+\)\s+(\w+)", stripped):
                name = match.group(1)
                visibility = "public" if name[0].isupper() else "private"
                symbols.append(Symbol(
                    name=name,
                    type="method",
                    line_start=i,
                    line_end=self._find_block_end(lines, i),
                    signature=self._clean_signature(stripped),
                    visibility=visibility
                ))

            # Constants
            elif match := re.match(r"const\s+(\w+)", stripped):
                name = match.group(1)
                visibility = "public" if name[0].isupper() else "private"
                symbols.append(Symbol(
                    name=name,
                    type="constant",
                    line_start=i,
                    line_end=i,
                    signature=self._clean_signature(stripped),
                    visibility=visibility
                ))

            # Variables (package-level)
            elif match := re.match(r"var\s+(\w+)", stripped):
                name = match.group(1)
                visibility = "public" if name[0].isupper() else "private"
                symbols.append(Symbol(
                    name=name,
                    type="variable",
                    line_start=i,
                    line_end=i,
                    signature=self._clean_signature(stripped),
                    visibility=visibility
                ))

        return ParseResult(
            file_type="go",
            symbols=symbols,
            dependencies=dependencies
        )
