"""Detectors for project type, architecture, and conventions."""

from .project_type import detect_project_type, PROJECT_INDICATORS
from .architecture import detect_architecture, ARCHITECTURE_PATTERNS
from .conventions import detect_conventions

__all__ = [
    "detect_project_type",
    "detect_architecture",
    "detect_conventions",
    "PROJECT_INDICATORS",
    "ARCHITECTURE_PATTERNS",
]
