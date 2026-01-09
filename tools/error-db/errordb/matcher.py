"""Fuzzy pattern matching for error messages."""

import re
from typing import List, Dict, Any, Optional
from difflib import SequenceMatcher

from .database import ErrorDatabase, Pattern, Solution


class PatternMatcher:
    """Match error messages against patterns."""

    def __init__(self, db: ErrorDatabase):
        self.db = db

    def match(self, error_text: str, threshold: float = 0.5) -> List[Dict[str, Any]]:
        """Find patterns matching the error text."""
        patterns = self.db.get_all_patterns()
        normalized_error = self._normalize(error_text)

        matches = []
        for pattern in patterns:
            # Try exact variable matching first
            score = self._variable_match(pattern.pattern, error_text)

            if score == 0:
                # Fall back to fuzzy matching
                normalized_pattern = self._normalize(pattern.pattern)
                score = self._fuzzy_match(normalized_pattern, normalized_error)

            if score >= threshold:
                matches.append({
                    "pattern": pattern,
                    "score": score
                })

        # Sort by score descending
        matches.sort(key=lambda x: x["score"], reverse=True)
        return matches

    def search(self, error_text: str) -> Optional[Dict[str, Any]]:
        """Search for best matching pattern with solutions."""
        matches = self.match(error_text)

        if not matches:
            return None

        best = matches[0]
        solutions = self.db.get_solutions(best["pattern"].id)

        return {
            "pattern": best["pattern"],
            "score": best["score"],
            "solutions": solutions
        }

    def _normalize(self, text: str) -> str:
        """Normalize text for comparison."""
        # Remove line numbers
        text = re.sub(r':\d+:\d+', ':N:N', text)
        text = re.sub(r' line \d+', ' line N', text)

        # Remove file paths
        text = re.sub(r'/[\w/.-]+', '/PATH', text)
        text = re.sub(r'[A-Za-z]:\\[\w\\.-]+', 'PATH', text)

        # Remove specific values in quotes
        text = re.sub(r"'[^']*'", "'X'", text)
        text = re.sub(r'"[^"]*"', '"X"', text)

        # Normalize whitespace
        text = ' '.join(text.split())

        return text.lower()

    def _fuzzy_match(self, pattern: str, error: str) -> float:
        """Fuzzy string matching using SequenceMatcher."""
        # Check if pattern is substring
        if pattern in error:
            return 0.9

        # Use SequenceMatcher for fuzzy matching
        ratio = SequenceMatcher(None, pattern, error).ratio()

        # Also check partial matching
        words = pattern.split()
        if len(words) > 2:
            # Check if key words are present
            matches = sum(1 for w in words if w in error)
            word_ratio = matches / len(words)
            ratio = max(ratio, word_ratio)

        return ratio

    def _variable_match(self, pattern: str, error: str) -> float:
        """Match pattern with variable placeholders (X, Y, Z)."""
        # Convert pattern to regex
        regex_pattern = re.escape(pattern)
        regex_pattern = regex_pattern.replace("X", ".+?")
        regex_pattern = regex_pattern.replace("Y", ".+?")
        regex_pattern = regex_pattern.replace("Z", ".+?")

        try:
            if re.search(regex_pattern, error, re.IGNORECASE):
                return 1.0
        except re.error:
            pass

        return 0


def format_match(match: Dict[str, Any], verbose: bool = False) -> str:
    """Format a match result for display."""
    lines = []
    pattern = match["pattern"]
    score = match["score"]
    solutions = match.get("solutions", [])

    # Header
    confidence = int(score * 100)
    if confidence >= 80:
        indicator = "HIGH"
    elif confidence >= 60:
        indicator = "MED"
    else:
        indicator = "LOW"

    lines.append(f"[{indicator}] Match ({confidence}%): {pattern.pattern}")

    if pattern.language:
        lines.append(f"     Language: {pattern.language}")

    lines.append("")

    # Best solution
    if solutions:
        best = solutions[0]
        lines.append("SOLUTION:")
        lines.append("-" * 50)
        for line in best.solution.split("\n"):
            lines.append(f"  {line}")
        lines.append("-" * 50)

        # Stats
        total = best.success_count + best.failure_count
        if total > 0:
            success_rate = best.success_count / total * 100
            lines.append(f"Success rate: {success_rate:.0f}% ({best.success_count}/{total})")

        # Auto-fix command
        if best.command:
            lines.append(f"\nAuto-fix command: {best.command}")

        if verbose and len(solutions) > 1:
            lines.append(f"\n({len(solutions) - 1} more solutions available)")
    else:
        lines.append("No solutions recorded for this pattern.")

    return "\n".join(lines)
