"""TypeScript and JavaScript parser."""

import re
from typing import List

from .base import BaseParser, ParseResult, Symbol, Dependency


class TypeScriptParser(BaseParser):
    """Parser for TypeScript and JavaScript files."""

    @classmethod
    def extensions(cls) -> List[str]:
        return [".ts", ".tsx", ".js", ".jsx", ".mjs", ".cjs"]

    def parse(self, content: str) -> ParseResult:
        symbols = []
        dependencies = []
        lines = content.split("\n")

        is_typescript = any(
            line.strip().startswith(("interface ", "type ", ": "))
            for line in lines[:50]
        )

        for i, line in enumerate(lines, 1):
            stripped = line.strip()

            # Skip comments
            if stripped.startswith("//") or stripped.startswith("/*"):
                continue

            # Imports
            if match := re.match(r"import\s+.*?from\s+['\"](.+?)['\"]", stripped):
                dependencies.append(Dependency(
                    target_path=match.group(1),
                    import_type="import"
                ))
            elif match := re.match(r"import\s+['\"](.+?)['\"]", stripped):
                dependencies.append(Dependency(
                    target_path=match.group(1),
                    import_type="import"
                ))
            elif match := re.match(r"(?:const|let|var)\s+\w+\s*=\s*require\(['\"](.+?)['\"]\)", stripped):
                dependencies.append(Dependency(
                    target_path=match.group(1),
                    import_type="require"
                ))

            # Export detection for visibility
            is_exported = stripped.startswith("export ")
            visibility = "public" if is_exported else "internal"

            # Classes
            if match := re.match(r"(?:export\s+)?(?:abstract\s+)?class\s+(\w+)", stripped):
                symbols.append(Symbol(
                    name=match.group(1),
                    type="class",
                    line_start=i,
                    line_end=self._find_block_end(lines, i),
                    signature=self._clean_signature(stripped),
                    visibility=visibility
                ))

            # Interfaces (TypeScript)
            elif match := re.match(r"(?:export\s+)?interface\s+(\w+)", stripped):
                symbols.append(Symbol(
                    name=match.group(1),
                    type="interface",
                    line_start=i,
                    line_end=self._find_block_end(lines, i),
                    signature=self._clean_signature(stripped),
                    visibility=visibility
                ))

            # Type aliases (TypeScript)
            elif match := re.match(r"(?:export\s+)?type\s+(\w+)\s*=", stripped):
                symbols.append(Symbol(
                    name=match.group(1),
                    type="type",
                    line_start=i,
                    line_end=i,
                    signature=self._clean_signature(stripped),
                    visibility=visibility
                ))

            # Functions (various forms)
            elif match := re.match(r"(?:export\s+)?(?:async\s+)?function\s+(\w+)", stripped):
                symbols.append(Symbol(
                    name=match.group(1),
                    type="function",
                    line_start=i,
                    line_end=self._find_block_end(lines, i),
                    signature=self._clean_signature(stripped),
                    visibility=visibility
                ))

            # Arrow functions assigned to const/let
            elif match := re.match(r"(?:export\s+)?const\s+(\w+)\s*=\s*(?:async\s+)?\(", stripped):
                symbols.append(Symbol(
                    name=match.group(1),
                    type="function",
                    line_start=i,
                    line_end=self._find_block_end(lines, i),
                    signature=self._clean_signature(stripped),
                    visibility=visibility
                ))

            # React components (function returning JSX)
            elif match := re.match(r"(?:export\s+)?(?:default\s+)?function\s+([A-Z]\w+)", stripped):
                symbols.append(Symbol(
                    name=match.group(1),
                    type="component",
                    line_start=i,
                    line_end=self._find_block_end(lines, i),
                    signature=self._clean_signature(stripped),
                    visibility=visibility
                ))

            # Enums (TypeScript)
            elif match := re.match(r"(?:export\s+)?(?:const\s+)?enum\s+(\w+)", stripped):
                symbols.append(Symbol(
                    name=match.group(1),
                    type="enum",
                    line_start=i,
                    line_end=self._find_block_end(lines, i),
                    signature=self._clean_signature(stripped),
                    visibility=visibility
                ))

        file_type = "typescript" if is_typescript else "javascript"

        return ParseResult(
            file_type=file_type,
            symbols=symbols,
            dependencies=dependencies
        )
