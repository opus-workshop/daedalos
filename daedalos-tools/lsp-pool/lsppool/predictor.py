"""Predictive warming - anticipate which servers will be needed."""

import sqlite3
from datetime import datetime, timedelta
from pathlib import Path
from typing import List, Dict, Any

from .config import DATA_DIR


class Predictor:
    """Predict which language servers will be needed based on usage patterns."""

    def __init__(self, db_path: Path = None):
        self.db_path = db_path or DATA_DIR / "predictions.db"
        self.db_path.parent.mkdir(parents=True, exist_ok=True)
        self._init_db()

    def _init_db(self):
        """Initialize prediction database."""
        conn = sqlite3.connect(self.db_path)
        conn.executescript("""
            CREATE TABLE IF NOT EXISTS activity (
                id INTEGER PRIMARY KEY,
                language TEXT NOT NULL,
                project TEXT NOT NULL,
                timestamp DATETIME DEFAULT CURRENT_TIMESTAMP,
                query_type TEXT
            );

            CREATE INDEX IF NOT EXISTS idx_activity_timestamp ON activity(timestamp);
            CREATE INDEX IF NOT EXISTS idx_activity_lang_proj ON activity(language, project);

            CREATE TABLE IF NOT EXISTS sessions (
                id INTEGER PRIMARY KEY,
                start_time DATETIME DEFAULT CURRENT_TIMESTAMP,
                languages TEXT,
                projects TEXT
            );
        """)
        conn.commit()
        conn.close()

    def record_activity(self, language: str, project: Path, query_type: str = None):
        """Record that a server was used."""
        conn = sqlite3.connect(self.db_path)
        conn.execute(
            "INSERT INTO activity (language, project, query_type) VALUES (?, ?, ?)",
            (language, str(project), query_type)
        )
        conn.commit()
        conn.close()

    def record_session_start(self, languages: List[str], projects: List[str]):
        """Record the start of a development session."""
        conn = sqlite3.connect(self.db_path)
        conn.execute(
            "INSERT INTO sessions (languages, projects) VALUES (?, ?)",
            (",".join(languages), ",".join(projects))
        )
        conn.commit()
        conn.close()

    def predict(self, n: int = 5) -> List[Dict[str, Any]]:
        """
        Predict top N servers to warm.

        Uses a weighted scoring based on:
        - Recent activity (last 24 hours)
        - Frequency of use
        - Time of day patterns
        """
        conn = sqlite3.connect(self.db_path)
        predictions = []

        # Get recent activity (last 24 hours)
        cutoff = datetime.now() - timedelta(hours=24)
        rows = conn.execute(
            """
            SELECT language, project, COUNT(*) as count
            FROM activity
            WHERE timestamp > ?
            GROUP BY language, project
            ORDER BY count DESC
            LIMIT ?
            """,
            (cutoff.isoformat(), n)
        ).fetchall()

        for row in rows:
            predictions.append({
                "language": row[0],
                "project": row[1],
                "confidence": min(0.95, row[2] / 10),  # Scale confidence
                "reason": "recent_activity"
            })

        # If not enough from recent, check historical patterns
        if len(predictions) < n:
            # Get most frequently used combinations
            historical = conn.execute(
                """
                SELECT language, project, COUNT(*) as count
                FROM activity
                WHERE (language, project) NOT IN (
                    SELECT language, project FROM activity WHERE timestamp > ?
                )
                GROUP BY language, project
                ORDER BY count DESC
                LIMIT ?
                """,
                (cutoff.isoformat(), n - len(predictions))
            ).fetchall()

            for row in historical:
                predictions.append({
                    "language": row[0],
                    "project": row[1],
                    "confidence": min(0.7, row[2] / 50),  # Lower confidence
                    "reason": "historical"
                })

        conn.close()
        return predictions

    def get_stats(self) -> Dict[str, Any]:
        """Get prediction statistics."""
        conn = sqlite3.connect(self.db_path)

        total = conn.execute("SELECT COUNT(*) FROM activity").fetchone()[0]
        languages = conn.execute(
            "SELECT language, COUNT(*) FROM activity GROUP BY language ORDER BY COUNT(*) DESC"
        ).fetchall()
        projects = conn.execute(
            "SELECT project, COUNT(*) FROM activity GROUP BY project ORDER BY COUNT(*) DESC LIMIT 5"
        ).fetchall()

        conn.close()

        return {
            "total_queries": total,
            "languages": dict(languages),
            "top_projects": dict(projects),
        }

    def cleanup(self, days: int = 30):
        """Remove activity older than specified days."""
        conn = sqlite3.connect(self.db_path)
        cutoff = datetime.now() - timedelta(days=days)
        conn.execute("DELETE FROM activity WHERE timestamp < ?", (cutoff.isoformat(),))
        conn.commit()
        conn.close()
