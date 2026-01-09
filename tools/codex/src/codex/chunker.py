"""Code chunking for semantic search."""

import re
from dataclasses import dataclass
from typing import List, Optional
from pathlib import Path


@dataclass
class CodeChunk:
    """A chunk of code for indexing."""
    file_path: str
    start_line: int
    end_line: int
    content: str
    chunk_type: str  # function, class, block, file
    name: str  # function/class name if applicable

    @property
    def location(self) -> str:
        return f"{self.file_path}:{self.start_line}"

    def __str__(self) -> str:
        return f"[{self.chunk_type}] {self.name} at {self.location}"


class CodeChunker:
    """Chunk code files into searchable segments."""

    # Extensions and their chunkers
    CHUNKERS = {
        "py": "_chunk_python",
        "js": "_chunk_javascript",
        "ts": "_chunk_javascript",
        "tsx": "_chunk_javascript",
        "jsx": "_chunk_javascript",
        "swift": "_chunk_swift",
        "rs": "_chunk_rust",
        "go": "_chunk_go",
        "rb": "_chunk_ruby",
    }

    def chunk_file(self, path: str, content: str) -> List[CodeChunk]:
        """Chunk a file into searchable segments."""
        ext = path.split(".")[-1].lower()
        chunker_name = self.CHUNKERS.get(ext)

        if chunker_name:
            chunker = getattr(self, chunker_name)
            chunks = chunker(path, content)
        else:
            chunks = self._chunk_generic(path, content)

        # Always include whole file as a chunk for context
        if content.strip():
            chunks.append(CodeChunk(
                file_path=path,
                start_line=1,
                end_line=content.count("\n") + 1,
                content=content[:2000],  # Truncate for large files
                chunk_type="file",
                name=Path(path).name
            ))

        return chunks

    def _chunk_python(self, path: str, content: str) -> List[CodeChunk]:
        """Chunk Python code."""
        chunks = []
        lines = content.split("\n")

        # Pattern for function and class definitions
        pattern = r"^(class|def|async def)\s+(\w+)"
        current_chunk = None
        current_indent = 0

        for i, line in enumerate(lines, 1):
            match = re.match(pattern, line)
            if match:
                # Save previous chunk
                if current_chunk:
                    current_chunk.end_line = i - 1
                    current_chunk.content = "\n".join(
                        lines[current_chunk.start_line - 1:current_chunk.end_line]
                    )
                    chunks.append(current_chunk)

                chunk_type = "class" if match.group(1) == "class" else "function"
                current_chunk = CodeChunk(
                    file_path=path,
                    start_line=i,
                    end_line=i,
                    content="",
                    chunk_type=chunk_type,
                    name=match.group(2)
                )
                current_indent = len(line) - len(line.lstrip())

        # Don't forget the last chunk
        if current_chunk:
            current_chunk.end_line = len(lines)
            current_chunk.content = "\n".join(
                lines[current_chunk.start_line - 1:current_chunk.end_line]
            )
            chunks.append(current_chunk)

        return chunks

    def _chunk_javascript(self, path: str, content: str) -> List[CodeChunk]:
        """Chunk JavaScript/TypeScript code."""
        chunks = []
        lines = content.split("\n")

        # Patterns for functions, classes, and exports
        patterns = [
            (r"^(export\s+)?(async\s+)?function\s+(\w+)", "function", 3),
            (r"^(export\s+)?class\s+(\w+)", "class", 2),
            (r"^(export\s+)?const\s+(\w+)\s*=\s*(async\s+)?\(", "function", 2),
            (r"^(export\s+)?const\s+(\w+)\s*=\s*(async\s+)?function", "function", 2),
        ]

        current_chunk = None
        brace_depth = 0

        for i, line in enumerate(lines, 1):
            for pattern, chunk_type, name_group in patterns:
                match = re.match(pattern, line.strip())
                if match:
                    if current_chunk:
                        current_chunk.end_line = i - 1
                        current_chunk.content = "\n".join(
                            lines[current_chunk.start_line - 1:current_chunk.end_line]
                        )
                        chunks.append(current_chunk)

                    current_chunk = CodeChunk(
                        file_path=path,
                        start_line=i,
                        end_line=i,
                        content="",
                        chunk_type=chunk_type,
                        name=match.group(name_group)
                    )
                    break

        if current_chunk:
            current_chunk.end_line = len(lines)
            current_chunk.content = "\n".join(
                lines[current_chunk.start_line - 1:current_chunk.end_line]
            )
            chunks.append(current_chunk)

        return chunks

    def _chunk_swift(self, path: str, content: str) -> List[CodeChunk]:
        """Chunk Swift code."""
        chunks = []
        lines = content.split("\n")

        patterns = [
            (r"^\s*(public|private|internal|open)?\s*func\s+(\w+)", "function", 2),
            (r"^\s*(public|private|internal|open)?\s*class\s+(\w+)", "class", 2),
            (r"^\s*(public|private|internal|open)?\s*struct\s+(\w+)", "struct", 2),
            (r"^\s*(public|private|internal|open)?\s*enum\s+(\w+)", "enum", 2),
            (r"^\s*(public|private|internal|open)?\s*protocol\s+(\w+)", "protocol", 2),
        ]

        current_chunk = None

        for i, line in enumerate(lines, 1):
            for pattern, chunk_type, name_group in patterns:
                match = re.match(pattern, line)
                if match:
                    if current_chunk:
                        current_chunk.end_line = i - 1
                        current_chunk.content = "\n".join(
                            lines[current_chunk.start_line - 1:current_chunk.end_line]
                        )
                        chunks.append(current_chunk)

                    current_chunk = CodeChunk(
                        file_path=path,
                        start_line=i,
                        end_line=i,
                        content="",
                        chunk_type=chunk_type,
                        name=match.group(name_group)
                    )
                    break

        if current_chunk:
            current_chunk.end_line = len(lines)
            current_chunk.content = "\n".join(
                lines[current_chunk.start_line - 1:current_chunk.end_line]
            )
            chunks.append(current_chunk)

        return chunks

    def _chunk_rust(self, path: str, content: str) -> List[CodeChunk]:
        """Chunk Rust code."""
        chunks = []
        lines = content.split("\n")

        patterns = [
            (r"^\s*(pub\s+)?fn\s+(\w+)", "function", 2),
            (r"^\s*(pub\s+)?struct\s+(\w+)", "struct", 2),
            (r"^\s*(pub\s+)?enum\s+(\w+)", "enum", 2),
            (r"^\s*(pub\s+)?trait\s+(\w+)", "trait", 2),
            (r"^\s*impl\s+(\w+)", "impl", 1),
        ]

        current_chunk = None

        for i, line in enumerate(lines, 1):
            for pattern, chunk_type, name_group in patterns:
                match = re.match(pattern, line)
                if match:
                    if current_chunk:
                        current_chunk.end_line = i - 1
                        current_chunk.content = "\n".join(
                            lines[current_chunk.start_line - 1:current_chunk.end_line]
                        )
                        chunks.append(current_chunk)

                    current_chunk = CodeChunk(
                        file_path=path,
                        start_line=i,
                        end_line=i,
                        content="",
                        chunk_type=chunk_type,
                        name=match.group(name_group)
                    )
                    break

        if current_chunk:
            current_chunk.end_line = len(lines)
            current_chunk.content = "\n".join(
                lines[current_chunk.start_line - 1:current_chunk.end_line]
            )
            chunks.append(current_chunk)

        return chunks

    def _chunk_go(self, path: str, content: str) -> List[CodeChunk]:
        """Chunk Go code."""
        chunks = []
        lines = content.split("\n")

        patterns = [
            (r"^func\s+(\w+)", "function", 1),
            (r"^func\s+\([^)]+\)\s+(\w+)", "method", 1),
            (r"^type\s+(\w+)\s+struct", "struct", 1),
            (r"^type\s+(\w+)\s+interface", "interface", 1),
        ]

        current_chunk = None

        for i, line in enumerate(lines, 1):
            for pattern, chunk_type, name_group in patterns:
                match = re.match(pattern, line)
                if match:
                    if current_chunk:
                        current_chunk.end_line = i - 1
                        current_chunk.content = "\n".join(
                            lines[current_chunk.start_line - 1:current_chunk.end_line]
                        )
                        chunks.append(current_chunk)

                    current_chunk = CodeChunk(
                        file_path=path,
                        start_line=i,
                        end_line=i,
                        content="",
                        chunk_type=chunk_type,
                        name=match.group(name_group)
                    )
                    break

        if current_chunk:
            current_chunk.end_line = len(lines)
            current_chunk.content = "\n".join(
                lines[current_chunk.start_line - 1:current_chunk.end_line]
            )
            chunks.append(current_chunk)

        return chunks

    def _chunk_ruby(self, path: str, content: str) -> List[CodeChunk]:
        """Chunk Ruby code."""
        chunks = []
        lines = content.split("\n")

        patterns = [
            (r"^\s*def\s+(\w+)", "method", 1),
            (r"^\s*class\s+(\w+)", "class", 1),
            (r"^\s*module\s+(\w+)", "module", 1),
        ]

        current_chunk = None

        for i, line in enumerate(lines, 1):
            for pattern, chunk_type, name_group in patterns:
                match = re.match(pattern, line)
                if match:
                    if current_chunk:
                        current_chunk.end_line = i - 1
                        current_chunk.content = "\n".join(
                            lines[current_chunk.start_line - 1:current_chunk.end_line]
                        )
                        chunks.append(current_chunk)

                    current_chunk = CodeChunk(
                        file_path=path,
                        start_line=i,
                        end_line=i,
                        content="",
                        chunk_type=chunk_type,
                        name=match.group(name_group)
                    )
                    break

        if current_chunk:
            current_chunk.end_line = len(lines)
            current_chunk.content = "\n".join(
                lines[current_chunk.start_line - 1:current_chunk.end_line]
            )
            chunks.append(current_chunk)

        return chunks

    def _chunk_generic(self, path: str, content: str) -> List[CodeChunk]:
        """Generic chunking for unknown file types."""
        # Just split into blocks of ~50 lines
        chunks = []
        lines = content.split("\n")
        chunk_size = 50

        for i in range(0, len(lines), chunk_size):
            chunk_lines = lines[i:i + chunk_size]
            chunks.append(CodeChunk(
                file_path=path,
                start_line=i + 1,
                end_line=min(i + chunk_size, len(lines)),
                content="\n".join(chunk_lines),
                chunk_type="block",
                name=f"block_{i // chunk_size + 1}"
            ))

        return chunks
