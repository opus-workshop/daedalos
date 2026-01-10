"""Token estimation for context tracking."""

from typing import Optional


class TokenEstimator:
    """Estimate token counts for text content."""

    def __init__(self):
        """Initialize estimator, trying tiktoken first."""
        self.encoder = None
        try:
            import tiktoken
            self.encoder = tiktoken.encoding_for_model("gpt-4")
        except ImportError:
            pass
        except Exception:
            pass

    def count(self, text: str) -> int:
        """Count tokens in text.

        Uses tiktoken if available, otherwise falls back to character heuristic.
        """
        if not text:
            return 0

        if self.encoder:
            try:
                return len(self.encoder.encode(text))
            except Exception:
                pass

        # Fallback: ~4 characters per token (reasonable estimate for English)
        return len(text) // 4

    def count_file(self, content: str, file_type: str = "") -> int:
        """Count tokens in file content with type-specific adjustments."""
        base_count = self.count(content)

        # Code tends to have more tokens per character due to syntax
        if file_type in [".py", ".js", ".ts", ".swift", ".rs", ".go"]:
            return int(base_count * 1.1)

        return base_count

    def estimate_remaining(self, used: int, max_context: int = 200000) -> int:
        """Estimate remaining tokens available."""
        return max(0, max_context - used)

    def format_count(self, count: int) -> str:
        """Format token count for display."""
        if count >= 1000000:
            return f"{count / 1000000:.1f}M"
        elif count >= 1000:
            return f"{count / 1000:.1f}K"
        return str(count)
