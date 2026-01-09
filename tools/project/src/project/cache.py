"""Cache management for project index."""

import hashlib
import json
import time
from pathlib import Path
from typing import Optional

from .database import ProjectDatabase


def get_cache_dir() -> Path:
    """Get the cache directory."""
    xdg_cache = Path.home() / ".cache"
    if "XDG_CACHE_HOME" in __import__("os").environ:
        xdg_cache = Path(__import__("os").environ["XDG_CACHE_HOME"])
    return xdg_cache / "daedalos" / "project"


class CacheManager:
    """Manages the cache for a project."""

    def __init__(self, project_path: Path):
        self.project_path = project_path.resolve()
        self.cache_dir = self._get_cache_path()
        self.cache_dir.mkdir(parents=True, exist_ok=True)
        self.meta_file = self.cache_dir / "meta.json"
        self.db_file = self.cache_dir / "index.db"
        self._db: Optional[ProjectDatabase] = None

    def _get_cache_path(self) -> Path:
        """Get cache path for this project."""
        # Hash the project path to create unique cache directory
        path_hash = hashlib.sha256(str(self.project_path).encode()).hexdigest()[:16]
        return get_cache_dir() / path_hash

    def get_database(self) -> ProjectDatabase:
        """Get or create the database."""
        if self._db is None:
            self._db = ProjectDatabase(self.db_file)
        return self._db

    def close(self):
        """Close database connection."""
        if self._db:
            self._db.close()
            self._db = None

    def is_stale(self, max_age_seconds: int = 3600) -> bool:
        """Check if cache is stale."""
        if not self.meta_file.exists():
            return True

        try:
            meta = json.loads(self.meta_file.read_text())
            last_indexed = meta.get("last_indexed", 0)
            return time.time() - last_indexed > max_age_seconds
        except (json.JSONDecodeError, KeyError):
            return True

    def mark_fresh(self):
        """Mark cache as freshly indexed."""
        meta = self._load_meta()
        meta["last_indexed"] = time.time()
        meta["project_path"] = str(self.project_path)
        self._save_meta(meta)

    def get_last_indexed(self) -> Optional[float]:
        """Get timestamp of last indexing."""
        meta = self._load_meta()
        return meta.get("last_indexed")

    def set_project_type(self, project_type: str):
        """Store detected project type."""
        meta = self._load_meta()
        meta["project_type"] = project_type
        self._save_meta(meta)

    def get_project_type(self) -> Optional[str]:
        """Get stored project type."""
        meta = self._load_meta()
        return meta.get("project_type")

    def _load_meta(self) -> dict:
        """Load metadata from file."""
        if self.meta_file.exists():
            try:
                return json.loads(self.meta_file.read_text())
            except json.JSONDecodeError:
                return {}
        return {}

    def _save_meta(self, meta: dict):
        """Save metadata to file."""
        self.meta_file.write_text(json.dumps(meta, indent=2))

    def clear(self):
        """Clear all cached data."""
        if self._db:
            self._db.close()
            self._db = None

        if self.db_file.exists():
            self.db_file.unlink()
        if self.meta_file.exists():
            self.meta_file.unlink()
