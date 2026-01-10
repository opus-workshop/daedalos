"""Project indexing logic."""

import hashlib
import fnmatch
from pathlib import Path
from typing import List, Dict, Any, Optional, Iterator

from .database import ProjectDatabase
from .cache import CacheManager
from .parsers import get_parser, supported_extensions
from .detectors import detect_project_type, detect_architecture, detect_conventions


# Default ignore patterns
DEFAULT_IGNORE = [
    "node_modules",
    ".git",
    "__pycache__",
    "*.pyc",
    ".pytest_cache",
    ".mypy_cache",
    "build",
    "dist",
    "target",
    "venv",
    ".venv",
    ".tox",
    "vendor",
    "Pods",
    ".DS_Store",
    "*.lock",
    "*.log",
]


class ProjectIndex:
    """Index and query project codebase."""

    def __init__(self, path: str, refresh: bool = False):
        """Initialize project index.

        Args:
            path: Path to project root
            refresh: Force re-index even if cache is fresh
        """
        self.path = Path(path).resolve()
        self.cache = CacheManager(self.path)
        self.db = self.cache.get_database()

        if refresh or self.cache.is_stale():
            self.reindex()

    def close(self):
        """Close database connections."""
        self.cache.close()

    def reindex(self, full: bool = False):
        """Index or re-index the project.

        Args:
            full: If True, clear and rebuild entire index
        """
        if full:
            self.cache.clear()
            self.db = self.cache.get_database()

        # Detect and store project type
        project_type = detect_project_type(self.path)
        self.cache.set_project_type(project_type)

        # Index all files
        for file_path in self._iter_files():
            self._index_file(file_path)

        # Detect patterns
        detect_conventions(self.db)

        # Mark as fresh
        self.cache.mark_fresh()

    def _iter_files(self) -> Iterator[Path]:
        """Iterate over indexable files."""
        supported = set(supported_extensions())

        for path in self.path.rglob("*"):
            if path.is_file() and not self._should_ignore(path):
                # Only process files we have parsers for OR common text files
                if path.suffix.lower() in supported:
                    yield path

    def _should_ignore(self, path: Path) -> bool:
        """Check if path should be ignored."""
        rel_path = str(path.relative_to(self.path))

        for pattern in DEFAULT_IGNORE:
            if fnmatch.fnmatch(rel_path, pattern):
                return True
            if fnmatch.fnmatch(path.name, pattern):
                return True
            # Check if any parent directory matches
            for part in path.relative_to(self.path).parts:
                if fnmatch.fnmatch(part, pattern):
                    return True

        return False

    def _index_file(self, file_path: Path):
        """Index a single file."""
        try:
            content = file_path.read_text(errors="ignore")
        except Exception:
            return

        # Calculate hash for change detection
        hash_ = hashlib.sha256(content.encode()).hexdigest()[:16]
        rel_path = str(file_path.relative_to(self.path))

        # Check if file is unchanged
        existing = self.db.get_file(rel_path)
        if existing and existing["hash"] == hash_:
            return

        # Get parser for this file type
        parser = get_parser(file_path)
        if not parser:
            return

        # Parse file
        result = parser.parse(content)

        # Store file record
        file_id = self.db.upsert_file(
            rel_path,
            result.file_type,
            file_path.stat().st_size,
            content.count("\n") + 1,
            file_path.stat().st_mtime,
            hash_
        )

        # Clear old symbols and dependencies
        self.db.clear_symbols_for_file(file_id)
        self.db.clear_dependencies_for_file(file_id)

        # Store symbols
        for sym in result.symbols:
            self.db.add_symbol(
                file_id,
                sym.name,
                sym.type,
                sym.line_start,
                sym.line_end,
                sym.signature,
                sym.visibility
            )

        # Store dependencies
        for dep in result.dependencies:
            # Try to resolve target file ID
            target_file_id = self._resolve_dependency(dep.target_path)
            self.db.add_dependency(
                file_id,
                dep.target_path,
                target_file_id,
                dep.import_type
            )

    def _resolve_dependency(self, target_path: str) -> Optional[int]:
        """Try to resolve a dependency to a file ID."""
        # This is a simplified resolution - real implementation would handle
        # module resolution for each language
        possible_paths = [
            target_path,
            f"{target_path}.ts",
            f"{target_path}.js",
            f"{target_path}/index.ts",
            f"{target_path}/index.js",
            f"{target_path}.swift",
            f"{target_path}.py",
            target_path.replace(".", "/") + ".py",
        ]

        for path in possible_paths:
            file = self.db.get_file(path)
            if file:
                return file["id"]

        return None

    def get_summary(self) -> Dict[str, Any]:
        """Get project summary."""
        return {
            "name": self.path.name,
            "path": str(self.path),
            "type": self.cache.get_project_type() or detect_project_type(self.path),
            "architecture": detect_architecture(self.db),
            "entry_points": self._find_entry_points(),
            "modules": self._find_key_modules(),
            "dependencies": self._find_external_deps(),
            "conventions": [c["pattern"] for c in self.db.get_conventions()[:5]],
            "stats": self.db.get_stats(),
        }

    def _find_entry_points(self) -> List[str]:
        """Find main entry points."""
        entry_points = []

        # Common entry point patterns by language
        patterns = [
            "main.py", "__main__.py", "app.py",
            "main.ts", "index.ts", "app.ts",
            "main.js", "index.js", "app.js",
            "main.swift", "App.swift", "*App.swift",
            "main.rs", "lib.rs",
            "main.go", "cmd/*/main.go",
        ]

        files = self.db.get_all_files()
        for f in files:
            name = f["path"].split("/")[-1]
            for pattern in patterns:
                if fnmatch.fnmatch(name, pattern) or fnmatch.fnmatch(f["path"], pattern):
                    entry_points.append(f["path"])
                    break

        return entry_points[:5]

    def _find_key_modules(self) -> List[Dict[str, str]]:
        """Find key modules/directories."""
        modules = []

        # Group files by top-level directory
        dir_files = {}
        files = self.db.get_all_files()

        for f in files:
            parts = f["path"].split("/")
            if len(parts) > 1:
                top_dir = parts[0]
                if top_dir not in dir_files:
                    dir_files[top_dir] = []
                dir_files[top_dir].append(f)

        # Generate descriptions for key directories
        for dir_name, dir_file_list in sorted(dir_files.items(), key=lambda x: -len(x[1]))[:5]:
            description = self._describe_directory(dir_name, dir_file_list)
            modules.append({
                "name": dir_name,
                "description": description,
                "file_count": len(dir_file_list),
            })

        return modules

    def _describe_directory(self, name: str, files: List[Dict]) -> str:
        """Generate description for a directory."""
        # Count file types
        types = {}
        for f in files:
            t = f.get("type", "unknown")
            types[t] = types.get(t, 0) + 1

        if not types:
            return f"{len(files)} files"

        primary_type = max(types, key=types.get)

        # Common directory descriptions
        descriptions = {
            "src": "Source code",
            "lib": "Library code",
            "test": "Tests",
            "tests": "Tests",
            "spec": "Specifications/Tests",
            "docs": "Documentation",
            "config": "Configuration",
            "scripts": "Scripts",
            "components": "UI Components",
            "views": "Views",
            "models": "Data models",
            "services": "Services",
            "utils": "Utilities",
            "helpers": "Helper functions",
            "api": "API handlers",
        }

        return descriptions.get(name.lower(), f"{len(files)} {primary_type} files")

    def _find_external_deps(self) -> List[str]:
        """Find external dependencies."""
        external = set()

        deps = self.db.conn.execute(
            "SELECT DISTINCT target_path FROM dependencies WHERE target_file_id IS NULL"
        ).fetchall()

        for row in deps:
            path = row["target_path"]
            # Filter to likely external deps (not relative paths)
            if not path.startswith(".") and "/" not in path:
                external.add(path)

        return sorted(external)[:10]

    def get_file_deps(self, file_path: str) -> Dict[str, Any]:
        """Get dependencies for a specific file."""
        file = self.db.get_file(file_path)
        if not file:
            return {"error": f"File not found: {file_path}"}

        deps = self.db.get_file_dependencies(file["id"])
        return {
            "file": file_path,
            "imports": [d["target_path"] for d in deps],
        }

    def get_file_dependents(self, file_path: str) -> Dict[str, Any]:
        """Get files that depend on a specific file."""
        file = self.db.get_file(file_path)
        if not file:
            return {"error": f"File not found: {file_path}"}

        deps = self.db.get_file_dependents(file["id"])
        return {
            "file": file_path,
            "imported_by": [d["source_path"] for d in deps],
        }

    def search_symbols(self, pattern: str) -> List[Dict[str, Any]]:
        """Search for symbols matching pattern."""
        return self.db.search_symbols(pattern)
