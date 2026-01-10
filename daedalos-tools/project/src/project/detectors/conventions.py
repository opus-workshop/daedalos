"""Convention detection from code patterns."""

import re
import json
from collections import Counter
from typing import List, Dict, Any
from ..database import ProjectDatabase


def detect_conventions(db: ProjectDatabase) -> List[str]:
    """Detect naming and organizational conventions.

    Args:
        db: Project database with indexed files

    Returns:
        List of detected convention descriptions
    """
    conventions = []

    # Analyze file naming patterns
    file_patterns = _analyze_file_names(db)
    conventions.extend(file_patterns)

    # Analyze symbol naming patterns
    symbol_patterns = _analyze_symbol_names(db)
    conventions.extend(symbol_patterns)

    # Analyze directory organization
    dir_patterns = _analyze_directories(db)
    conventions.extend(dir_patterns)

    # Store in database
    db.clear_conventions()
    for conv in conventions[:20]:  # Limit to top 20
        db.add_convention(conv, "detected", 1, "[]")

    return conventions


def _analyze_file_names(db: ProjectDatabase) -> List[str]:
    """Analyze file naming conventions."""
    patterns = []

    files = db.get_all_files()
    if not files:
        return patterns

    # Extract file names (without path)
    names = [f["path"].split("/")[-1] for f in files]

    # Check for common suffixes
    suffix_counter = Counter()
    for name in names:
        base = name.rsplit(".", 1)[0]  # Remove extension

        # Common suffixes
        for suffix in ["View", "Controller", "Service", "Model", "Repository",
                       "Handler", "Manager", "Helper", "Utils", "Test", "Spec",
                       "Component", "Screen", "Page", "Engine", "Provider"]:
            if base.endswith(suffix):
                suffix_counter[suffix] += 1

    # Report patterns with significant occurrences
    total_files = len(names)
    for suffix, count in suffix_counter.most_common(5):
        if count >= 2 and count / total_files >= 0.05:  # At least 5% of files
            patterns.append(f"Files ending in '{suffix}' ({count} files)")

    # Check for common prefixes (like I for interfaces)
    prefix_counter = Counter()
    for name in names:
        base = name.rsplit(".", 1)[0]
        if len(base) > 1:
            if base[0] == "I" and base[1].isupper():
                prefix_counter["I (Interface)"] += 1
            elif base.startswith("use") and base[3:4].isupper():
                prefix_counter["use (Hook)"] += 1

    for prefix, count in prefix_counter.most_common(3):
        if count >= 2:
            patterns.append(f"Files with '{prefix}' prefix ({count} files)")

    return patterns


def _analyze_symbol_names(db: ProjectDatabase) -> List[str]:
    """Analyze symbol naming conventions."""
    patterns = []

    # Get all symbols
    symbols = []
    for row in db.conn.execute("SELECT name, type FROM symbols"):
        symbols.append({"name": row["name"], "type": row["type"]})

    if not symbols:
        return patterns

    # Check naming style (camelCase, snake_case, PascalCase)
    styles = {"camelCase": 0, "snake_case": 0, "PascalCase": 0, "SCREAMING_SNAKE": 0}

    for sym in symbols:
        name = sym["name"]
        if re.match(r"^[a-z][a-zA-Z0-9]*$", name) and any(c.isupper() for c in name):
            styles["camelCase"] += 1
        elif re.match(r"^[a-z][a-z0-9_]*$", name):
            styles["snake_case"] += 1
        elif re.match(r"^[A-Z][a-zA-Z0-9]*$", name):
            styles["PascalCase"] += 1
        elif re.match(r"^[A-Z][A-Z0-9_]*$", name):
            styles["SCREAMING_SNAKE"] += 1

    # Report dominant style
    total = sum(styles.values())
    if total > 0:
        dominant = max(styles, key=styles.get)
        percentage = styles[dominant] / total * 100
        if percentage > 50:
            patterns.append(f"Naming: {dominant} style ({percentage:.0f}% of symbols)")

    # Check for test naming patterns
    test_patterns = Counter()
    for sym in symbols:
        name = sym["name"]
        if name.startswith("test_"):
            test_patterns["test_*"] += 1
        elif name.startswith("test"):
            test_patterns["test*"] += 1
        elif name.startswith("it_"):
            test_patterns["it_*"] += 1

    for pattern, count in test_patterns.most_common(1):
        if count >= 3:
            patterns.append(f"Tests: {pattern} naming pattern")

    return patterns


def _analyze_directories(db: ProjectDatabase) -> List[str]:
    """Analyze directory organization patterns."""
    patterns = []

    files = db.get_all_files()
    if not files:
        return patterns

    # Get top-level directories
    top_dirs = Counter()
    for f in files:
        parts = f["path"].split("/")
        if len(parts) > 1:
            top_dirs[parts[0]] += 1

    # Report significant directories
    total = sum(top_dirs.values())
    significant = [(d, c) for d, c in top_dirs.most_common(5) if c / total >= 0.1]

    if significant:
        dir_list = ", ".join(d for d, _ in significant)
        patterns.append(f"Key directories: {dir_list}")

    # Check for test mirroring
    src_dirs = set()
    test_dirs = set()
    for f in files:
        parts = f["path"].split("/")
        if "test" in parts[0].lower() or "tests" in parts[0].lower():
            if len(parts) > 1:
                test_dirs.add(parts[1])
        elif parts[0] in ["src", "lib", "app"]:
            if len(parts) > 1:
                src_dirs.add(parts[1])

    overlap = src_dirs & test_dirs
    if len(overlap) >= 2:
        patterns.append("Tests mirror source structure")

    return patterns
