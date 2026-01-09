"""Index management for semantic code search."""

import hashlib
import json
import os
import sqlite3
import sys
from pathlib import Path
from typing import List, Optional, Set

from .chunker import CodeChunk, CodeChunker
from .embedder import HybridEmbedder, get_embedder


# File extensions to index
INDEXABLE_EXTENSIONS = {
    # Languages
    "py", "pyw",                          # Python
    "js", "jsx", "mjs", "cjs",            # JavaScript
    "ts", "tsx", "mts", "cts",            # TypeScript
    "swift",                               # Swift
    "rs",                                  # Rust
    "go",                                  # Go
    "rb", "rake",                          # Ruby
    "java", "kt", "kts",                   # JVM
    "c", "h", "cpp", "hpp", "cc", "cxx",  # C/C++
    "cs",                                  # C#
    "php",                                 # PHP
    "lua",                                 # Lua
    "sh", "bash", "zsh", "fish",          # Shell
    "pl", "pm",                            # Perl
    "r",                                   # R
    "scala", "sc",                         # Scala
    "ex", "exs",                           # Elixir
    "erl", "hrl",                          # Erlang
    "hs", "lhs",                           # Haskell
    "ml", "mli",                           # OCaml
    "clj", "cljs", "cljc",                # Clojure
    "nim",                                 # Nim
    "zig",                                 # Zig
    "v",                                   # V
    "dart",                                # Dart
    "vue", "svelte",                       # Frontend frameworks
    # Config/Data
    "json", "yaml", "yml", "toml",
    "xml", "html", "css", "scss", "sass",
    "sql",
    "md", "txt", "rst",
    "nix", "dhall",
}

# Directories to skip
SKIP_DIRS = {
    ".git", ".hg", ".svn",
    "node_modules", "vendor", "venv", ".venv",
    "__pycache__", ".cache", ".tox",
    "build", "dist", "target", "out",
    ".next", ".nuxt", ".output",
    "coverage", ".coverage",
    ".idea", ".vscode",
}


class CodeIndex:
    """Manages the code index for a project."""

    def __init__(self, project_path: str, embedder: Optional[HybridEmbedder] = None):
        self.project_path = Path(project_path).resolve()
        self.index_dir = Path.home() / ".local" / "share" / "daedalos" / "codex"
        self.db_path = self.index_dir / f"{self._hash_path()}.db"
        self.chunker = CodeChunker()
        self.embedder = embedder or get_embedder()
        self._conn: Optional[sqlite3.Connection] = None

        # Load embedder state if index exists
        if self.db_path.exists():
            self._load_embedder_state()

    def _hash_path(self) -> str:
        """Generate a unique hash for the project path."""
        return hashlib.sha256(str(self.project_path).encode()).hexdigest()[:16]

    def _load_embedder_state(self):
        """Load the embedder state from the database."""
        try:
            cursor = self.conn.execute(
                "SELECT value FROM metadata WHERE key = 'embedder_state'"
            )
            row = cursor.fetchone()
            if row:
                state = json.loads(row[0])
                self.embedder.load_state(state)
        except (sqlite3.Error, json.JSONDecodeError):
            pass

    def _save_embedder_state(self):
        """Save the embedder state to the database."""
        state = self.embedder.to_dict()
        self.conn.execute(
            "INSERT OR REPLACE INTO metadata (key, value) VALUES (?, ?)",
            ("embedder_state", json.dumps(state))
        )
        self.conn.commit()

    @property
    def conn(self) -> sqlite3.Connection:
        """Get database connection, creating if needed."""
        if self._conn is None:
            self._init_db()
        return self._conn

    def _init_db(self):
        """Initialize the database."""
        self.index_dir.mkdir(parents=True, exist_ok=True)
        self._conn = sqlite3.connect(str(self.db_path))
        self._conn.row_factory = sqlite3.Row

        self._conn.executescript("""
            CREATE TABLE IF NOT EXISTS chunks (
                id INTEGER PRIMARY KEY,
                file_path TEXT NOT NULL,
                start_line INTEGER NOT NULL,
                end_line INTEGER NOT NULL,
                content TEXT NOT NULL,
                chunk_type TEXT NOT NULL,
                name TEXT NOT NULL,
                embedding BLOB,
                file_hash TEXT,
                created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
            );

            CREATE INDEX IF NOT EXISTS idx_chunks_file ON chunks(file_path);
            CREATE INDEX IF NOT EXISTS idx_chunks_hash ON chunks(file_hash);

            CREATE TABLE IF NOT EXISTS metadata (
                key TEXT PRIMARY KEY,
                value TEXT
            );

            CREATE TABLE IF NOT EXISTS file_hashes (
                file_path TEXT PRIMARY KEY,
                hash TEXT NOT NULL,
                indexed_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
            );
        """)
        self._conn.commit()

    def is_indexed(self) -> bool:
        """Check if the project has been indexed."""
        if not self.db_path.exists():
            return False

        cursor = self.conn.execute("SELECT COUNT(*) FROM chunks")
        count = cursor.fetchone()[0]
        return count > 0

    def _should_index(self, path: Path) -> bool:
        """Check if a file should be indexed."""
        # Check extension
        ext = path.suffix.lstrip(".").lower()
        if ext not in INDEXABLE_EXTENSIONS:
            return False

        # Check path components for skip directories
        for part in path.parts:
            if part in SKIP_DIRS:
                return False

        # Skip hidden files (but not directories we already checked)
        if path.name.startswith("."):
            return False

        # Skip large files (> 500KB)
        try:
            if path.stat().st_size > 500_000:
                return False
        except OSError:
            return False

        return True

    def _file_hash(self, path: Path) -> str:
        """Get hash of file contents."""
        try:
            content = path.read_bytes()
            return hashlib.sha256(content).hexdigest()[:16]
        except OSError:
            return ""

    def _needs_reindex(self, path: Path) -> bool:
        """Check if a file needs to be re-indexed."""
        rel_path = str(path.relative_to(self.project_path))
        current_hash = self._file_hash(path)

        cursor = self.conn.execute(
            "SELECT hash FROM file_hashes WHERE file_path = ?",
            (rel_path,)
        )
        row = cursor.fetchone()
        if not row:
            return True
        return row[0] != current_hash

    def _get_indexable_files(self) -> List[Path]:
        """Get list of files to index."""
        files = []
        for path in self.project_path.rglob("*"):
            if path.is_file() and self._should_index(path):
                files.append(path)
        return sorted(files)

    def index_project(self, force: bool = False, progress_callback=None):
        """Index the entire project."""
        files = self._get_indexable_files()
        total = len(files)

        if total == 0:
            print("No files to index.")
            return

        # Collect all content for TF-IDF fitting
        all_content = []
        files_to_index = []

        for path in files:
            if force or self._needs_reindex(path):
                try:
                    content = path.read_text(errors="ignore")
                    all_content.append(content)
                    files_to_index.append((path, content))
                except OSError:
                    continue

        if not files_to_index:
            print("Index is up to date.")
            return

        # Fit TF-IDF if using that backend
        if not self.embedder.use_ollama and all_content:
            print("Building vocabulary...")
            self.embedder.fit(all_content)

        print(f"Indexing {len(files_to_index)} files using {self.embedder.backend_name}...")

        indexed = 0
        for path, content in files_to_index:
            try:
                self._index_file(path, content)
                indexed += 1

                if progress_callback:
                    progress_callback(indexed, len(files_to_index), path.name)
                elif indexed % 10 == 0 or indexed == len(files_to_index):
                    pct = (indexed / len(files_to_index)) * 100
                    print(f"\r  [{pct:5.1f}%] {indexed}/{len(files_to_index)} files", end="")
                    sys.stdout.flush()

            except Exception as e:
                print(f"\nError indexing {path}: {e}", file=sys.stderr)

        print(f"\nIndexed {indexed} files.")

        # Save backend info
        self.conn.execute(
            "INSERT OR REPLACE INTO metadata (key, value) VALUES (?, ?)",
            ("backend", self.embedder.backend_name)
        )
        self.conn.commit()

        # Save embedder state (for TF-IDF vocabulary persistence)
        self._save_embedder_state()

    def _index_file(self, path: Path, content: Optional[str] = None):
        """Index a single file."""
        if content is None:
            content = path.read_text(errors="ignore")

        rel_path = str(path.relative_to(self.project_path))
        file_hash = self._file_hash(path)

        # Remove old chunks for this file
        self.conn.execute("DELETE FROM chunks WHERE file_path = ?", (rel_path,))

        # Chunk the file
        chunks = self.chunker.chunk_file(rel_path, content)

        # Generate embeddings and store
        for chunk in chunks:
            # Truncate content for embedding
            embed_text = chunk.content[:2000]
            embedding = self.embedder.embed(embed_text)

            self.conn.execute("""
                INSERT INTO chunks (file_path, start_line, end_line, content,
                                    chunk_type, name, embedding, file_hash)
                VALUES (?, ?, ?, ?, ?, ?, ?, ?)
            """, (
                chunk.file_path,
                chunk.start_line,
                chunk.end_line,
                chunk.content,
                chunk.chunk_type,
                chunk.name,
                json.dumps(embedding),
                file_hash
            ))

        # Update file hash
        self.conn.execute(
            "INSERT OR REPLACE INTO file_hashes (file_path, hash) VALUES (?, ?)",
            (rel_path, file_hash)
        )
        self.conn.commit()

    def get_stats(self) -> dict:
        """Get index statistics."""
        cursor = self.conn.execute("SELECT COUNT(*) FROM chunks")
        chunk_count = cursor.fetchone()[0]

        cursor = self.conn.execute("SELECT COUNT(DISTINCT file_path) FROM chunks")
        file_count = cursor.fetchone()[0]

        cursor = self.conn.execute(
            "SELECT value FROM metadata WHERE key = 'backend'"
        )
        row = cursor.fetchone()
        backend = row[0] if row else "unknown"

        return {
            "chunks": chunk_count,
            "files": file_count,
            "backend": backend,
            "db_path": str(self.db_path),
            "project_path": str(self.project_path),
        }

    def clear(self):
        """Clear the index."""
        self.conn.execute("DELETE FROM chunks")
        self.conn.execute("DELETE FROM file_hashes")
        self.conn.execute("DELETE FROM metadata")
        self.conn.commit()
        print("Index cleared.")

    def close(self):
        """Close database connection."""
        if self._conn:
            self._conn.close()
            self._conn = None
