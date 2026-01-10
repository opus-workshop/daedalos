"""Vector search for semantic code search."""

import json
import math
from typing import List, Optional

from .indexer import CodeIndex


def cosine_similarity(a: List[float], b: List[float]) -> float:
    """Calculate cosine similarity between two vectors."""
    if not a or not b:
        return 0.0

    # Handle different vector sizes (TF-IDF vectors may vary)
    min_len = min(len(a), len(b))
    a = a[:min_len]
    b = b[:min_len]

    dot_product = sum(x * y for x, y in zip(a, b))
    norm_a = math.sqrt(sum(x * x for x in a))
    norm_b = math.sqrt(sum(x * x for x in b))

    if norm_a == 0 or norm_b == 0:
        return 0.0

    return dot_product / (norm_a * norm_b)


class SearchResult:
    """A search result with ranking information."""

    def __init__(
        self,
        file_path: str,
        start_line: int,
        end_line: int,
        content: str,
        chunk_type: str,
        name: str,
        similarity: float,
    ):
        self.file_path = file_path
        self.start_line = start_line
        self.end_line = end_line
        self.content = content
        self.chunk_type = chunk_type
        self.name = name
        self.similarity = similarity

    @property
    def location(self) -> str:
        """Get file:line location string."""
        return f"{self.file_path}:{self.start_line}"

    @property
    def preview(self) -> str:
        """Get a preview of the content."""
        # First line or first 100 chars
        first_line = self.content.split("\n")[0].strip()
        if len(first_line) > 100:
            return first_line[:97] + "..."
        return first_line

    def __repr__(self) -> str:
        pct = self.similarity * 100
        return f"SearchResult({pct:.0f}% {self.location} {self.name})"


class CodeSearcher:
    """Search the code index."""

    def __init__(self, index: CodeIndex):
        self.index = index
        self.embedder = index.embedder

    def search(
        self,
        query: str,
        limit: int = 10,
        file_filter: Optional[str] = None,
        type_filter: Optional[str] = None,
        min_similarity: float = 0.1,
    ) -> List[SearchResult]:
        """
        Search for code matching the query.

        Args:
            query: Natural language query
            limit: Maximum number of results
            file_filter: Only search in files matching this pattern
            type_filter: Only search chunks of this type (function, class, etc.)
            min_similarity: Minimum similarity threshold

        Returns:
            List of SearchResult objects sorted by similarity
        """
        # Embed the query
        query_embedding = self.embedder.embed(query)

        if not query_embedding:
            return []

        # Build SQL query
        sql = "SELECT file_path, start_line, end_line, content, chunk_type, name, embedding FROM chunks WHERE 1=1"
        params = []

        if file_filter:
            sql += " AND file_path LIKE ?"
            params.append(f"%{file_filter}%")

        if type_filter:
            sql += " AND chunk_type = ?"
            params.append(type_filter)

        cursor = self.index.conn.execute(sql, params)

        # Calculate similarities
        results = []
        for row in cursor:
            try:
                chunk_embedding = json.loads(row["embedding"])
            except (json.JSONDecodeError, TypeError):
                continue

            similarity = cosine_similarity(query_embedding, chunk_embedding)

            if similarity >= min_similarity:
                results.append(SearchResult(
                    file_path=row["file_path"],
                    start_line=row["start_line"],
                    end_line=row["end_line"],
                    content=row["content"],
                    chunk_type=row["chunk_type"],
                    name=row["name"],
                    similarity=similarity,
                ))

        # Sort by similarity (descending)
        results.sort(key=lambda r: r.similarity, reverse=True)

        return results[:limit]

    def search_file(self, query: str, file_path: str, limit: int = 5) -> List[SearchResult]:
        """Search within a specific file."""
        return self.search(query, limit=limit, file_filter=file_path)

    def find_similar(
        self,
        file_path: str,
        line: int,
        limit: int = 5,
    ) -> List[SearchResult]:
        """
        Find code similar to a specific location.

        Args:
            file_path: Path to the reference file
            line: Line number in the reference file
            limit: Maximum number of results

        Returns:
            List of similar code chunks
        """
        # Find the chunk containing this line
        cursor = self.index.conn.execute("""
            SELECT content, embedding FROM chunks
            WHERE file_path = ? AND start_line <= ? AND end_line >= ?
            LIMIT 1
        """, (file_path, line, line))

        row = cursor.fetchone()
        if not row:
            return []

        try:
            reference_embedding = json.loads(row["embedding"])
        except (json.JSONDecodeError, TypeError):
            return []

        # Search for similar chunks
        cursor = self.index.conn.execute("""
            SELECT file_path, start_line, end_line, content, chunk_type, name, embedding
            FROM chunks
            WHERE NOT (file_path = ? AND start_line <= ? AND end_line >= ?)
        """, (file_path, line, line))

        results = []
        for row in cursor:
            try:
                chunk_embedding = json.loads(row["embedding"])
            except (json.JSONDecodeError, TypeError):
                continue

            similarity = cosine_similarity(reference_embedding, chunk_embedding)

            if similarity > 0.1:
                results.append(SearchResult(
                    file_path=row["file_path"],
                    start_line=row["start_line"],
                    end_line=row["end_line"],
                    content=row["content"],
                    chunk_type=row["chunk_type"],
                    name=row["name"],
                    similarity=similarity,
                ))

        results.sort(key=lambda r: r.similarity, reverse=True)
        return results[:limit]


def format_results(results: List[SearchResult], show_content: bool = False) -> str:
    """Format search results for display."""
    if not results:
        return "No results found."

    lines = []
    for i, r in enumerate(results, 1):
        pct = r.similarity * 100
        lines.append(f"{i}. [{pct:.0f}%] {r.location}")
        lines.append(f"   {r.chunk_type}: {r.name}")

        if show_content:
            # Show first few lines
            content_lines = r.content.split("\n")[:5]
            for line in content_lines:
                lines.append(f"   | {line[:80]}")
            if len(r.content.split("\n")) > 5:
                lines.append("   | ...")

        lines.append("")

    return "\n".join(lines)
