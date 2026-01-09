"""Rust language parser."""

import re
from typing import List

from .base import BaseParser, ParseResult, Symbol, Dependency


class RustParser(BaseParser):
    """Parser for Rust files."""

    @classmethod
    def extensions(cls) -> List[str]:
        return [".rs"]

    def parse(self, content: str) -> ParseResult:
        symbols = []
        dependencies = []
        lines = content.split("\n")

        for i, line in enumerate(lines, 1):
            stripped = line.strip()

            # Skip comments
            if stripped.startswith("//") or stripped.startswith("/*"):
                continue

            # Use statements
            if match := re.match(r"use\s+([\w:]+)", stripped):
                dependencies.append(Dependency(
                    target_path=match.group(1),
                    import_type="use"
                ))

            # Extern crate
            elif match := re.match(r"extern\s+crate\s+(\w+)", stripped):
                dependencies.append(Dependency(
                    target_path=match.group(1),
                    import_type="extern"
                ))

            # Modules
            if match := re.match(r"(pub(?:\(.+?\))?\s+)?mod\s+(\w+)", stripped):
                visibility = "public" if match.group(1) else "private"
                symbols.append(Symbol(
                    name=match.group(2),
                    type="module",
                    line_start=i,
                    line_end=self._find_block_end(lines, i) if "{" in stripped else i,
                    signature=self._clean_signature(stripped),
                    visibility=visibility
                ))

            # Structs
            elif match := re.match(r"(pub(?:\(.+?\))?\s+)?struct\s+(\w+)", stripped):
                visibility = "public" if match.group(1) else "private"
                symbols.append(Symbol(
                    name=match.group(2),
                    type="struct",
                    line_start=i,
                    line_end=self._find_block_end(lines, i),
                    signature=self._clean_signature(stripped),
                    visibility=visibility
                ))

            # Enums
            elif match := re.match(r"(pub(?:\(.+?\))?\s+)?enum\s+(\w+)", stripped):
                visibility = "public" if match.group(1) else "private"
                symbols.append(Symbol(
                    name=match.group(2),
                    type="enum",
                    line_start=i,
                    line_end=self._find_block_end(lines, i),
                    signature=self._clean_signature(stripped),
                    visibility=visibility
                ))

            # Traits
            elif match := re.match(r"(pub(?:\(.+?\))?\s+)?trait\s+(\w+)", stripped):
                visibility = "public" if match.group(1) else "private"
                symbols.append(Symbol(
                    name=match.group(2),
                    type="trait",
                    line_start=i,
                    line_end=self._find_block_end(lines, i),
                    signature=self._clean_signature(stripped),
                    visibility=visibility
                ))

            # Impl blocks
            elif match := re.match(r"impl(?:<.+?>)?\s+(?:(\w+)\s+for\s+)?(\w+)", stripped):
                trait_name = match.group(1)
                type_name = match.group(2)
                name = f"{trait_name} for {type_name}" if trait_name else type_name
                symbols.append(Symbol(
                    name=name,
                    type="impl",
                    line_start=i,
                    line_end=self._find_block_end(lines, i),
                    signature=self._clean_signature(stripped),
                    visibility="internal"
                ))

            # Functions
            elif match := re.match(r"(pub(?:\(.+?\))?\s+)?(async\s+)?(unsafe\s+)?fn\s+(\w+)", stripped):
                visibility = "public" if match.group(1) else "private"
                symbols.append(Symbol(
                    name=match.group(4),
                    type="function",
                    line_start=i,
                    line_end=self._find_block_end(lines, i),
                    signature=self._clean_signature(stripped),
                    visibility=visibility
                ))

            # Constants
            elif match := re.match(r"(pub(?:\(.+?\))?\s+)?const\s+(\w+)", stripped):
                visibility = "public" if match.group(1) else "private"
                symbols.append(Symbol(
                    name=match.group(2),
                    type="constant",
                    line_start=i,
                    line_end=i,
                    signature=self._clean_signature(stripped),
                    visibility=visibility
                ))

            # Statics
            elif match := re.match(r"(pub(?:\(.+?\))?\s+)?static\s+(\w+)", stripped):
                visibility = "public" if match.group(1) else "private"
                symbols.append(Symbol(
                    name=match.group(2),
                    type="static",
                    line_start=i,
                    line_end=i,
                    signature=self._clean_signature(stripped),
                    visibility=visibility
                ))

        return ParseResult(
            file_type="rust",
            symbols=symbols,
            dependencies=dependencies
        )
