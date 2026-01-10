"""
Daedalos Project - Pre-computed codebase intelligence

A command-line tool that provides instant codebase intelligence
through pre-computed indexes.
"""

__version__ = "1.0.0"

from .cli import cli
from .index import ProjectIndex

__all__ = ["cli", "ProjectIndex", "__version__"]
