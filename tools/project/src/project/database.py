"""SQLite database operations for project index."""

import sqlite3
from pathlib import Path
from typing import Optional, Dict, Any, List

SCHEMA = """
CREATE TABLE IF NOT EXISTS files (
    id INTEGER PRIMARY KEY,
    path TEXT UNIQUE NOT NULL,
    type TEXT,
    size INTEGER,
    lines INTEGER,
    modified REAL,
    hash TEXT
);

CREATE TABLE IF NOT EXISTS symbols (
    id INTEGER PRIMARY KEY,
    file_id INTEGER REFERENCES files(id),
    name TEXT NOT NULL,
    type TEXT,
    line_start INTEGER,
    line_end INTEGER,
    signature TEXT,
    visibility TEXT
);

CREATE TABLE IF NOT EXISTS dependencies (
    id INTEGER PRIMARY KEY,
    source_file_id INTEGER REFERENCES files(id),
    target_path TEXT,
    target_file_id INTEGER,
    import_type TEXT
);

CREATE TABLE IF NOT EXISTS conventions (
    id INTEGER PRIMARY KEY,
    pattern TEXT,
    type TEXT,
    occurrences INTEGER,
    examples TEXT
);

CREATE TABLE IF NOT EXISTS metadata (
    key TEXT PRIMARY KEY,
    value TEXT
);

CREATE INDEX IF NOT EXISTS idx_files_path ON files(path);
CREATE INDEX IF NOT EXISTS idx_symbols_name ON symbols(name);
CREATE INDEX IF NOT EXISTS idx_symbols_file ON symbols(file_id);
CREATE INDEX IF NOT EXISTS idx_deps_source ON dependencies(source_file_id);
CREATE INDEX IF NOT EXISTS idx_deps_target ON dependencies(target_file_id);
"""


class ProjectDatabase:
    """Database for storing project index."""

    def __init__(self, db_path: Path):
        self.db_path = db_path
        db_path.parent.mkdir(parents=True, exist_ok=True)
        self.conn = sqlite3.connect(db_path)
        self.conn.row_factory = sqlite3.Row
        self._init_schema()

    def _init_schema(self):
        """Initialize database schema."""
        self.conn.executescript(SCHEMA)
        self.conn.commit()

    def close(self):
        """Close database connection."""
        self.conn.close()

    # File operations

    def upsert_file(
        self,
        path: str,
        type_: str,
        size: int,
        lines: int,
        modified: float,
        hash_: str
    ) -> int:
        """Insert or update a file record."""
        cursor = self.conn.execute(
            """
            INSERT INTO files (path, type, size, lines, modified, hash)
            VALUES (?, ?, ?, ?, ?, ?)
            ON CONFLICT(path) DO UPDATE SET
                type=excluded.type,
                size=excluded.size,
                lines=excluded.lines,
                modified=excluded.modified,
                hash=excluded.hash
            """,
            (path, type_, size, lines, modified, hash_)
        )
        self.conn.commit()

        # Get the file ID
        result = self.conn.execute(
            "SELECT id FROM files WHERE path = ?", (path,)
        ).fetchone()
        return result["id"] if result else cursor.lastrowid

    def get_file(self, path: str) -> Optional[Dict[str, Any]]:
        """Get file by path."""
        row = self.conn.execute(
            "SELECT * FROM files WHERE path = ?", (path,)
        ).fetchone()
        return dict(row) if row else None

    def get_file_by_id(self, file_id: int) -> Optional[Dict[str, Any]]:
        """Get file by ID."""
        row = self.conn.execute(
            "SELECT * FROM files WHERE id = ?", (file_id,)
        ).fetchone()
        return dict(row) if row else None

    def get_all_files(self) -> List[Dict[str, Any]]:
        """Get all files."""
        return [dict(row) for row in self.conn.execute("SELECT * FROM files")]

    def delete_file(self, path: str):
        """Delete a file and its related records."""
        file = self.get_file(path)
        if file:
            self.conn.execute("DELETE FROM symbols WHERE file_id = ?", (file["id"],))
            self.conn.execute("DELETE FROM dependencies WHERE source_file_id = ?", (file["id"],))
            self.conn.execute("DELETE FROM files WHERE id = ?", (file["id"],))
            self.conn.commit()

    # Symbol operations

    def add_symbol(
        self,
        file_id: int,
        name: str,
        type_: str,
        line_start: int,
        line_end: int,
        signature: str,
        visibility: str
    ) -> int:
        """Add a symbol record."""
        cursor = self.conn.execute(
            """
            INSERT INTO symbols (file_id, name, type, line_start, line_end, signature, visibility)
            VALUES (?, ?, ?, ?, ?, ?, ?)
            """,
            (file_id, name, type_, line_start, line_end, signature, visibility)
        )
        self.conn.commit()
        return cursor.lastrowid

    def get_symbols_for_file(self, file_id: int) -> List[Dict[str, Any]]:
        """Get all symbols for a file."""
        return [
            dict(row) for row in
            self.conn.execute("SELECT * FROM symbols WHERE file_id = ?", (file_id,))
        ]

    def search_symbols(self, pattern: str) -> List[Dict[str, Any]]:
        """Search symbols by name pattern."""
        return [
            dict(row) for row in
            self.conn.execute(
                "SELECT s.*, f.path as file_path FROM symbols s JOIN files f ON s.file_id = f.id WHERE s.name LIKE ?",
                (f"%{pattern}%",)
            )
        ]

    def clear_symbols_for_file(self, file_id: int):
        """Clear all symbols for a file."""
        self.conn.execute("DELETE FROM symbols WHERE file_id = ?", (file_id,))
        self.conn.commit()

    # Dependency operations

    def add_dependency(
        self,
        source_file_id: int,
        target_path: str,
        target_file_id: Optional[int],
        import_type: str
    ) -> int:
        """Add a dependency record."""
        cursor = self.conn.execute(
            """
            INSERT INTO dependencies (source_file_id, target_path, target_file_id, import_type)
            VALUES (?, ?, ?, ?)
            """,
            (source_file_id, target_path, target_file_id, import_type)
        )
        self.conn.commit()
        return cursor.lastrowid

    def get_file_dependencies(self, file_id: int) -> List[Dict[str, Any]]:
        """Get dependencies for a file (what it imports)."""
        return [
            dict(row) for row in
            self.conn.execute(
                "SELECT * FROM dependencies WHERE source_file_id = ?",
                (file_id,)
            )
        ]

    def get_file_dependents(self, file_id: int) -> List[Dict[str, Any]]:
        """Get dependents of a file (what imports it)."""
        return [
            dict(row) for row in
            self.conn.execute(
                """
                SELECT d.*, f.path as source_path
                FROM dependencies d
                JOIN files f ON d.source_file_id = f.id
                WHERE d.target_file_id = ?
                """,
                (file_id,)
            )
        ]

    def clear_dependencies_for_file(self, file_id: int):
        """Clear all dependencies for a file."""
        self.conn.execute("DELETE FROM dependencies WHERE source_file_id = ?", (file_id,))
        self.conn.commit()

    # Convention operations

    def add_convention(self, pattern: str, type_: str, occurrences: int, examples: str):
        """Add or update a convention."""
        self.conn.execute(
            """
            INSERT INTO conventions (pattern, type, occurrences, examples)
            VALUES (?, ?, ?, ?)
            ON CONFLICT DO NOTHING
            """,
            (pattern, type_, occurrences, examples)
        )
        self.conn.commit()

    def get_conventions(self) -> List[Dict[str, Any]]:
        """Get all detected conventions."""
        return [dict(row) for row in self.conn.execute("SELECT * FROM conventions ORDER BY occurrences DESC")]

    def clear_conventions(self):
        """Clear all conventions."""
        self.conn.execute("DELETE FROM conventions")
        self.conn.commit()

    # Metadata operations

    def set_metadata(self, key: str, value: str):
        """Set metadata value."""
        self.conn.execute(
            "INSERT OR REPLACE INTO metadata (key, value) VALUES (?, ?)",
            (key, value)
        )
        self.conn.commit()

    def get_metadata(self, key: str) -> Optional[str]:
        """Get metadata value."""
        row = self.conn.execute(
            "SELECT value FROM metadata WHERE key = ?", (key,)
        ).fetchone()
        return row["value"] if row else None

    # Statistics

    def get_stats(self) -> Dict[str, Any]:
        """Get database statistics."""
        file_count = self.conn.execute("SELECT COUNT(*) FROM files").fetchone()[0]
        symbol_count = self.conn.execute("SELECT COUNT(*) FROM symbols").fetchone()[0]
        dep_count = self.conn.execute("SELECT COUNT(*) FROM dependencies").fetchone()[0]

        # Lines by type
        lines_by_type = {}
        for row in self.conn.execute("SELECT type, SUM(lines) FROM files GROUP BY type"):
            if row[0]:
                lines_by_type[row[0]] = row[1]

        return {
            "files": file_count,
            "symbols": symbol_count,
            "dependencies": dep_count,
            "lines_by_type": lines_by_type,
        }
