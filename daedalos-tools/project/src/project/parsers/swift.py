"""Swift language parser."""

import re
from typing import List

from .base import BaseParser, ParseResult, Symbol, Dependency


class SwiftParser(BaseParser):
    """Parser for Swift files."""

    @classmethod
    def extensions(cls) -> List[str]:
        return [".swift"]

    def parse(self, content: str) -> ParseResult:
        symbols = []
        dependencies = []
        lines = content.split("\n")

        for i, line in enumerate(lines, 1):
            stripped = line.strip()

            # Skip comments
            if stripped.startswith("//") or stripped.startswith("/*"):
                continue

            # Imports
            if match := re.match(r"import\s+(\w+)", stripped):
                dependencies.append(Dependency(
                    target_path=match.group(1),
                    import_type="import"
                ))

            # Classes, structs, enums, protocols, actors
            if match := re.match(
                r"(public|internal|private|fileprivate|open)?\s*"
                r"(final\s+)?(class|struct|enum|protocol|actor)\s+(\w+)",
                stripped
            ):
                visibility = match.group(1) or "internal"
                symbols.append(Symbol(
                    name=match.group(4),
                    type=match.group(3),
                    line_start=i,
                    line_end=self._find_block_end(lines, i),
                    signature=self._clean_signature(stripped),
                    visibility=visibility
                ))

            # Functions and methods
            elif match := re.match(
                r"(public|internal|private|fileprivate|open)?\s*"
                r"(static\s+|class\s+)?(override\s+)?"
                r"func\s+(\w+)",
                stripped
            ):
                visibility = match.group(1) or "internal"
                symbols.append(Symbol(
                    name=match.group(4),
                    type="function",
                    line_start=i,
                    line_end=self._find_block_end(lines, i),
                    signature=self._clean_signature(stripped),
                    visibility=visibility
                ))

            # Properties
            elif match := re.match(
                r"(public|internal|private|fileprivate|open)?\s*"
                r"(static\s+|class\s+)?(let|var)\s+(\w+)",
                stripped
            ):
                # Only capture top-level or type-level properties
                if not stripped.startswith("guard") and "=" not in stripped[:stripped.find("let" if "let" in stripped else "var")]:
                    visibility = match.group(1) or "internal"
                    symbols.append(Symbol(
                        name=match.group(4),
                        type="property",
                        line_start=i,
                        line_end=i,
                        signature=self._clean_signature(stripped),
                        visibility=visibility
                    ))

            # Extensions
            elif match := re.match(r"extension\s+(\w+)", stripped):
                symbols.append(Symbol(
                    name=match.group(1),
                    type="extension",
                    line_start=i,
                    line_end=self._find_block_end(lines, i),
                    signature=self._clean_signature(stripped),
                    visibility="internal"
                ))

        return ParseResult(
            file_type="swift",
            symbols=symbols,
            dependencies=dependencies
        )
