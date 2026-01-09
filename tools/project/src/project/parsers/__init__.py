"""Language parsers for extracting symbols and dependencies."""

from pathlib import Path
from typing import Optional

from .base import BaseParser, ParseResult, Symbol, Dependency
from .swift import SwiftParser
from .typescript import TypeScriptParser
from .python_parser import PythonParser
from .rust import RustParser
from .go import GoParser

# Registry of parsers by extension
_PARSERS = {}

def _register_parsers():
    """Register all parsers."""
    parser_classes = [
        SwiftParser,
        TypeScriptParser,
        PythonParser,
        RustParser,
        GoParser,
    ]

    for parser_class in parser_classes:
        for ext in parser_class.extensions():
            _PARSERS[ext] = parser_class

_register_parsers()


def get_parser(file_path: Path) -> Optional[BaseParser]:
    """Get appropriate parser for a file."""
    ext = file_path.suffix.lower()
    parser_class = _PARSERS.get(ext)
    if parser_class:
        return parser_class()
    return None


def supported_extensions() -> list:
    """Get list of supported file extensions."""
    return list(_PARSERS.keys())


__all__ = [
    "BaseParser",
    "ParseResult",
    "Symbol",
    "Dependency",
    "get_parser",
    "supported_extensions",
]
