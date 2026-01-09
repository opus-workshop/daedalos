"""SQLite database for error patterns and solutions."""

import sqlite3
from pathlib import Path
from datetime import datetime
from typing import Optional, List, Dict, Any
from dataclasses import dataclass
import uuid
import os


@dataclass
class Pattern:
    id: str
    pattern: str
    scope: str
    language: Optional[str]
    framework: Optional[str]
    tags: List[str]
    created_at: str
    updated_at: str


@dataclass
class Solution:
    id: str
    pattern_id: str
    solution: str
    command: Optional[str]
    confidence: float
    success_count: int
    failure_count: int
    created_at: str
    last_confirmed: Optional[str]


def get_db_path() -> Path:
    """Get path to database file."""
    data_dir = Path(os.environ.get(
        "XDG_DATA_HOME",
        Path.home() / ".local" / "share"
    )) / "daedalos" / "error-db"
    data_dir.mkdir(parents=True, exist_ok=True)
    return data_dir / "errors.db"


class ErrorDatabase:
    """SQLite database for error patterns."""

    SCHEMA = """
    CREATE TABLE IF NOT EXISTS patterns (
        id TEXT PRIMARY KEY,
        pattern TEXT NOT NULL,
        scope TEXT NOT NULL DEFAULT 'global',
        language TEXT,
        framework TEXT,
        tags TEXT,
        created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
        updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
    );

    CREATE TABLE IF NOT EXISTS solutions (
        id TEXT PRIMARY KEY,
        pattern_id TEXT NOT NULL REFERENCES patterns(id),
        solution TEXT NOT NULL,
        command TEXT,
        confidence REAL DEFAULT 0.5,
        success_count INTEGER DEFAULT 0,
        failure_count INTEGER DEFAULT 0,
        created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
        last_confirmed TIMESTAMP
    );

    CREATE TABLE IF NOT EXISTS usage_log (
        id INTEGER PRIMARY KEY AUTOINCREMENT,
        pattern_id TEXT,
        solution_id TEXT,
        outcome TEXT,
        timestamp TIMESTAMP DEFAULT CURRENT_TIMESTAMP
    );

    CREATE INDEX IF NOT EXISTS idx_patterns_language ON patterns(language);
    CREATE INDEX IF NOT EXISTS idx_patterns_scope ON patterns(scope);
    CREATE INDEX IF NOT EXISTS idx_solutions_pattern ON solutions(pattern_id);
    """

    SEED_PATTERNS = [
        # Node.js / npm
        {
            "pattern": "Cannot find module 'X'",
            "scope": "language",
            "language": "javascript",
            "solution": "The module is not installed. Run:\n\n  npm install <module>\n\nOr if it's a dev dependency:\n\n  npm install --save-dev <module>",
            "command": "npm install"
        },
        {
            "pattern": "ENOENT: no such file or directory",
            "scope": "global",
            "solution": "The file or directory doesn't exist. Check:\n1. Path spelling\n2. Working directory (pwd)\n3. Whether the file was actually created",
        },
        {
            "pattern": "EADDRINUSE",
            "scope": "global",
            "solution": "Port is already in use. Either:\n1. Kill the process: lsof -i :PORT then kill <PID>\n2. Use a different port\n3. Wait for the port to be released",
            "command": "lsof -i :"
        },
        # Python
        {
            "pattern": "ModuleNotFoundError: No module named",
            "scope": "language",
            "language": "python",
            "solution": "The module is not installed. Run:\n\n  pip install <module>\n\nOr in a virtual environment:\n\n  python -m pip install <module>",
            "command": "pip install"
        },
        {
            "pattern": "IndentationError",
            "scope": "language",
            "language": "python",
            "solution": "Python requires consistent indentation. Check:\n1. Tabs vs spaces (use 4 spaces)\n2. Proper indentation after : characters\n3. No mixing of indentation styles",
        },
        {
            "pattern": "SyntaxError: invalid syntax",
            "scope": "language",
            "language": "python",
            "solution": "Python syntax error. Common causes:\n1. Missing colon after if/for/def/class\n2. Unmatched parentheses/brackets\n3. Missing quotes around strings\n4. Python 2 vs 3 incompatibility",
        },
        # Rust
        {
            "pattern": "error[E0382]: borrow of moved value",
            "scope": "language",
            "language": "rust",
            "solution": "Value was moved and can't be used again. Options:\n1. Clone: value.clone()\n2. Use references: &value\n3. Restructure to avoid multiple uses",
        },
        {
            "pattern": "error[E0277]: the trait bound",
            "scope": "language",
            "language": "rust",
            "solution": "Type doesn't implement required trait. Options:\n1. Derive the trait: #[derive(Trait)]\n2. Implement manually\n3. Use a different type that implements it",
        },
        # Git
        {
            "pattern": "fatal: not a git repository",
            "scope": "global",
            "solution": "You're not in a git repository. Either:\n1. cd to a git repository\n2. Initialize: git init\n3. Clone: git clone <url>",
            "command": "git init"
        },
        {
            "pattern": "Your branch is behind",
            "scope": "global",
            "solution": "Remote has new commits. Pull them:\n\n  git pull\n\nOr if you have local changes:\n\n  git pull --rebase",
            "command": "git pull"
        },
        {
            "pattern": "CONFLICT (content): Merge conflict",
            "scope": "global",
            "solution": "Files have conflicting changes. Steps:\n1. Open conflicting files\n2. Look for <<<< ==== >>>> markers\n3. Choose which changes to keep\n4. Remove markers\n5. git add <files>\n6. git commit",
        },
        # TypeScript
        {
            "pattern": "Property 'X' does not exist on type",
            "scope": "language",
            "language": "typescript",
            "solution": "TypeScript doesn't know about this property. Options:\n1. Add it to the type definition\n2. Use type assertion: (obj as ExtendedType).X\n3. Check if the property name is correct",
        },
        {
            "pattern": "Type 'X' is not assignable to type 'Y'",
            "scope": "language",
            "language": "typescript",
            "solution": "Type mismatch. Options:\n1. Fix the type of the value\n2. Update the type annotation\n3. Use a type assertion if you're sure\n4. Check if it's a nullable type issue",
        },
        # Swift
        {
            "pattern": "Cannot convert value of type",
            "scope": "language",
            "language": "swift",
            "solution": "Type conversion needed. Options:\n1. Cast explicitly: value as Type\n2. Initialize new type: Type(value)\n3. Check optional unwrapping\n4. Use map/compactMap for collections",
        },
        # General
        {
            "pattern": "Permission denied",
            "scope": "global",
            "solution": "Insufficient permissions. Options:\n1. Check file permissions: ls -la\n2. Change ownership: chown user:group file\n3. Use sudo (if appropriate)\n4. Check if file is locked/open elsewhere",
        },
        {
            "pattern": "Connection refused",
            "scope": "global",
            "solution": "Can't connect to the service. Check:\n1. Is the service running?\n2. Correct host/port?\n3. Firewall rules?\n4. Network connectivity?",
        },
        {
            "pattern": "command not found",
            "scope": "global",
            "solution": "Command is not installed or not in PATH. Options:\n1. Install the command\n2. Check spelling\n3. Add to PATH: export PATH=$PATH:/path/to/bin\n4. Use full path to command",
        },
        {
            "pattern": "out of memory",
            "scope": "global",
            "solution": "System ran out of memory. Options:\n1. Close other applications\n2. Increase swap space\n3. Optimize the program's memory usage\n4. Add more RAM",
        },
    ]

    def __init__(self, db_path: Optional[Path] = None):
        self.db_path = db_path or get_db_path()
        self.conn = sqlite3.connect(self.db_path)
        self.conn.row_factory = sqlite3.Row
        self._init_db()

    def _init_db(self):
        """Initialize database schema."""
        self.conn.executescript(self.SCHEMA)
        self.conn.commit()

        # Check if we need to seed
        count = self.conn.execute("SELECT COUNT(*) FROM patterns").fetchone()[0]
        if count == 0:
            self._seed_patterns()

    def _seed_patterns(self):
        """Seed database with common error patterns."""
        for p in self.SEED_PATTERNS:
            self.add_pattern(
                pattern=p["pattern"],
                scope=p.get("scope", "global"),
                language=p.get("language"),
                solution=p.get("solution"),
                command=p.get("command")
            )

    def add_pattern(
        self,
        pattern: str,
        scope: str = "global",
        language: Optional[str] = None,
        framework: Optional[str] = None,
        tags: Optional[List[str]] = None,
        solution: Optional[str] = None,
        command: Optional[str] = None
    ) -> str:
        """Add a new pattern and optional solution."""
        pattern_id = str(uuid.uuid4())

        self.conn.execute(
            """
            INSERT INTO patterns (id, pattern, scope, language, framework, tags)
            VALUES (?, ?, ?, ?, ?, ?)
            """,
            (pattern_id, pattern, scope, language, framework, ",".join(tags or []))
        )

        if solution:
            solution_id = str(uuid.uuid4())
            self.conn.execute(
                """
                INSERT INTO solutions (id, pattern_id, solution, command)
                VALUES (?, ?, ?, ?)
                """,
                (solution_id, pattern_id, solution, command)
            )

        self.conn.commit()
        return pattern_id

    def get_all_patterns(self) -> List[Pattern]:
        """Get all patterns."""
        rows = self.conn.execute("SELECT * FROM patterns").fetchall()
        return [self._row_to_pattern(row) for row in rows]

    def get_pattern(self, pattern_id: str) -> Optional[Pattern]:
        """Get pattern by ID."""
        row = self.conn.execute(
            "SELECT * FROM patterns WHERE id = ?", (pattern_id,)
        ).fetchone()
        return self._row_to_pattern(row) if row else None

    def get_solutions(self, pattern_id: str) -> List[Solution]:
        """Get solutions for a pattern."""
        rows = self.conn.execute(
            """
            SELECT * FROM solutions
            WHERE pattern_id = ?
            ORDER BY confidence DESC
            """,
            (pattern_id,)
        ).fetchall()
        return [self._row_to_solution(row) for row in rows]

    def add_solution(
        self,
        pattern_id: str,
        solution: str,
        command: Optional[str] = None
    ) -> str:
        """Add solution to existing pattern."""
        solution_id = str(uuid.uuid4())
        self.conn.execute(
            """
            INSERT INTO solutions (id, pattern_id, solution, command)
            VALUES (?, ?, ?, ?)
            """,
            (solution_id, pattern_id, solution, command)
        )
        self.conn.commit()
        return solution_id

    def confirm_solution(self, solution_id: str):
        """Mark solution as successful."""
        self.conn.execute(
            """
            UPDATE solutions
            SET success_count = success_count + 1,
                confidence = CAST(success_count + 1 AS REAL) / (success_count + failure_count + 1),
                last_confirmed = CURRENT_TIMESTAMP
            WHERE id = ?
            """,
            (solution_id,)
        )
        self.conn.commit()

    def report_failure(self, solution_id: str):
        """Mark solution as failed."""
        self.conn.execute(
            """
            UPDATE solutions
            SET failure_count = failure_count + 1,
                confidence = CAST(success_count AS REAL) / (success_count + failure_count + 1)
            WHERE id = ?
            """,
            (solution_id,)
        )
        self.conn.commit()

    def stats(self) -> Dict[str, Any]:
        """Get database statistics."""
        stats = {
            "total_patterns": self.conn.execute("SELECT COUNT(*) FROM patterns").fetchone()[0],
            "total_solutions": self.conn.execute("SELECT COUNT(*) FROM solutions").fetchone()[0],
            "by_scope": {},
            "by_language": {}
        }

        for row in self.conn.execute("SELECT scope, COUNT(*) FROM patterns GROUP BY scope"):
            stats["by_scope"][row[0]] = row[1]

        for row in self.conn.execute(
            "SELECT language, COUNT(*) FROM patterns WHERE language IS NOT NULL GROUP BY language"
        ):
            stats["by_language"][row[0]] = row[1]

        return stats

    def close(self):
        """Close database connection."""
        self.conn.close()

    def _row_to_pattern(self, row) -> Pattern:
        return Pattern(
            id=row["id"],
            pattern=row["pattern"],
            scope=row["scope"],
            language=row["language"],
            framework=row["framework"],
            tags=row["tags"].split(",") if row["tags"] else [],
            created_at=row["created_at"],
            updated_at=row["updated_at"]
        )

    def _row_to_solution(self, row) -> Solution:
        return Solution(
            id=row["id"],
            pattern_id=row["pattern_id"],
            solution=row["solution"],
            command=row["command"],
            confidence=row["confidence"],
            success_count=row["success_count"],
            failure_count=row["failure_count"],
            created_at=row["created_at"],
            last_confirmed=row["last_confirmed"]
        )


# CLI interface for stats
if __name__ == "__main__":
    import sys
    if len(sys.argv) > 1 and sys.argv[1] == "stats":
        db = ErrorDatabase()
        stats = db.stats()
        print(f"Patterns: {stats['total_patterns']}")
        print(f"Solutions: {stats['total_solutions']}")
        print("\nBy scope:")
        for scope, count in stats['by_scope'].items():
            print(f"  {scope}: {count}")
        print("\nBy language:")
        for lang, count in stats['by_language'].items():
            print(f"  {lang}: {count}")
        db.close()
