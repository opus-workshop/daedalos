"""Base parser class and data structures."""

from abc import ABC, abstractmethod
from dataclasses import dataclass, field
from typing import List, Optional


@dataclass
class Symbol:
    """Represents a code symbol (function, class, etc.)."""
    name: str
    type: str  # function, class, interface, struct, enum, etc.
    line_start: int
    line_end: int
    signature: str = ""
    visibility: str = "internal"  # public, private, internal, protected


@dataclass
class Dependency:
    """Represents an import/dependency."""
    target_path: str
    import_type: str = "import"  # import, require, use, include


@dataclass
class ParseResult:
    """Result of parsing a file."""
    file_type: str
    symbols: List[Symbol] = field(default_factory=list)
    dependencies: List[Dependency] = field(default_factory=list)


class BaseParser(ABC):
    """Base class for language parsers."""

    @abstractmethod
    def parse(self, content: str) -> ParseResult:
        """Parse file content and extract symbols and dependencies."""
        pass

    @classmethod
    @abstractmethod
    def extensions(cls) -> List[str]:
        """Return list of file extensions this parser handles."""
        pass

    def _find_block_end(self, lines: List[str], start: int, open_char: str = "{", close_char: str = "}") -> int:
        """Find end of code block by matching braces."""
        depth = 0
        found_open = False

        for i in range(start - 1, len(lines)):
            line = lines[i]
            for char in line:
                if char == open_char:
                    depth += 1
                    found_open = True
                elif char == close_char:
                    depth -= 1
                    if found_open and depth == 0:
                        return i + 1

        return start  # If no matching brace found, return start

    def _clean_signature(self, line: str) -> str:
        """Clean up a signature line."""
        # Remove leading/trailing whitespace
        sig = line.strip()
        # Remove trailing braces
        sig = sig.rstrip("{").strip()
        return sig
