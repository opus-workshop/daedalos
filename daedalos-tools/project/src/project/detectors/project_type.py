"""Project type detection."""

from pathlib import Path
from typing import Optional

PROJECT_INDICATORS = {
    "swift": ["*.xcodeproj", "*.xcworkspace", "Package.swift"],
    "typescript": ["tsconfig.json"],
    "nodejs": ["package.json"],
    "rust": ["Cargo.toml"],
    "go": ["go.mod"],
    "python": ["pyproject.toml", "setup.py", "requirements.txt"],
    "java-maven": ["pom.xml"],
    "java-gradle": ["build.gradle", "build.gradle.kts"],
    "ruby": ["Gemfile"],
    "php": ["composer.json"],
    "elixir": ["mix.exs"],
    "deno": ["deno.json", "deno.jsonc"],
    "dotnet": ["*.csproj", "*.fsproj", "*.sln"],
}

EXTENSION_FALLBACKS = {
    ".py": "python",
    ".js": "nodejs",
    ".ts": "typescript",
    ".swift": "swift",
    ".rs": "rust",
    ".go": "go",
    ".rb": "ruby",
    ".php": "php",
    ".ex": "elixir",
    ".exs": "elixir",
    ".java": "java",
    ".cs": "dotnet",
}


def detect_project_type(path: Path) -> str:
    """Detect project type from indicator files.

    Args:
        path: Path to the project root

    Returns:
        Detected project type string
    """
    path = Path(path)

    # Check indicator files in priority order
    for project_type, indicators in PROJECT_INDICATORS.items():
        for indicator in indicators:
            if "*" in indicator:
                # Glob pattern
                if list(path.glob(indicator)):
                    return project_type
            elif (path / indicator).exists():
                return project_type

    # Fallback: detect from most common file extension
    return _detect_from_extensions(path)


def _detect_from_extensions(path: Path) -> str:
    """Detect project type from file extensions."""
    extensions = {}

    # Count extensions (limit to reasonable depth)
    for f in path.rglob("*"):
        if f.is_file() and not _is_ignored(f):
            ext = f.suffix.lower()
            if ext:
                extensions[ext] = extensions.get(ext, 0) + 1

    if not extensions:
        return "unknown"

    # Get most common extension
    top_ext = max(extensions, key=extensions.get)
    return EXTENSION_FALLBACKS.get(top_ext, "unknown")


def _is_ignored(path: Path) -> bool:
    """Check if path should be ignored."""
    ignore_dirs = {
        "node_modules", ".git", "__pycache__", "build", "dist",
        "target", "venv", ".venv", ".tox", "vendor", "Pods"
    }
    return any(part in ignore_dirs for part in path.parts)


def get_project_description(project_type: str) -> str:
    """Get human-readable description of project type."""
    descriptions = {
        "swift": "Swift/Xcode Project",
        "typescript": "TypeScript Project",
        "nodejs": "Node.js Project",
        "rust": "Rust/Cargo Project",
        "go": "Go Project",
        "python": "Python Project",
        "java-maven": "Java (Maven) Project",
        "java-gradle": "Java (Gradle) Project",
        "ruby": "Ruby Project",
        "php": "PHP Project",
        "elixir": "Elixir Project",
        "deno": "Deno Project",
        "dotnet": ".NET Project",
        "unknown": "Unknown Project Type",
    }
    return descriptions.get(project_type, project_type)
