"""Architecture pattern detection."""

from typing import Dict, Any, Set
from ..database import ProjectDatabase

ARCHITECTURE_PATTERNS = {
    "mvc": {
        "dirs": ["controllers", "models", "views"],
        "description": "Model-View-Controller",
        "weight": 3,
    },
    "mvvm": {
        "dirs": ["viewmodels", "views", "models"],
        "alt_dirs": ["viewmodel", "view", "model"],
        "description": "Model-View-ViewModel",
        "weight": 3,
    },
    "clean": {
        "dirs": ["domain", "application", "infrastructure"],
        "alt_dirs": ["core", "data", "presentation"],
        "description": "Clean Architecture",
        "weight": 3,
    },
    "hexagonal": {
        "dirs": ["adapters", "ports", "domain"],
        "description": "Hexagonal Architecture",
        "weight": 3,
    },
    "feature-based": {
        "dirs": ["features", "modules"],
        "description": "Feature-based Organization",
        "weight": 2,
    },
    "component-based": {
        "dirs": ["components", "pages"],
        "alt_dirs": ["components", "screens"],
        "description": "Component-based (React/Vue style)",
        "weight": 2,
    },
    "layered": {
        "dirs": ["api", "services", "repositories"],
        "alt_dirs": ["handlers", "service", "repository"],
        "description": "Layered Architecture",
        "weight": 2,
    },
    "monolith": {
        "dirs": ["src"],
        "description": "Monolith (Single Source)",
        "weight": 1,
    },
}


def detect_architecture(db: ProjectDatabase) -> Dict[str, Any]:
    """Detect architecture pattern from directory structure.

    Args:
        db: Project database with indexed files

    Returns:
        Dictionary with detected architecture info
    """
    # Collect all directory names from file paths
    dirs = _collect_directories(db)

    # Score each pattern
    scores = {}
    for pattern_name, config in ARCHITECTURE_PATTERNS.items():
        score = _score_pattern(dirs, config)
        if score > 0:
            scores[pattern_name] = score * config.get("weight", 1)

    if not scores:
        return {
            "type": "unknown",
            "description": "No recognized pattern",
            "confidence": "low",
        }

    # Get best match
    best_match = max(scores, key=scores.get)
    best_score = scores[best_match]
    config = ARCHITECTURE_PATTERNS[best_match]

    # Determine confidence
    if best_score >= 3:
        confidence = "high"
    elif best_score >= 2:
        confidence = "medium"
    else:
        confidence = "low"

    return {
        "type": best_match,
        "description": config["description"],
        "confidence": confidence,
        "score": best_score,
    }


def _collect_directories(db: ProjectDatabase) -> Set[str]:
    """Collect all directory names from indexed files."""
    dirs = set()

    for row in db.conn.execute("SELECT DISTINCT path FROM files"):
        path = row["path"]
        parts = path.split("/")
        # Add each directory level, normalized to lowercase
        for part in parts[:-1]:  # Exclude filename
            dirs.add(part.lower())

    return dirs


def _score_pattern(dirs: Set[str], config: Dict) -> int:
    """Score how well directories match a pattern."""
    score = 0

    # Check primary dirs
    primary_dirs = config.get("dirs", [])
    for d in primary_dirs:
        if d.lower() in dirs:
            score += 1

    # Check alternate dirs
    alt_dirs = config.get("alt_dirs", [])
    for d in alt_dirs:
        if d.lower() in dirs:
            score += 0.5

    return score


def get_architecture_recommendations(arch_type: str) -> list:
    """Get recommendations based on architecture type."""
    recommendations = {
        "mvc": [
            "Keep controllers thin",
            "Business logic belongs in models/services",
            "Views should be presentation-only",
        ],
        "mvvm": [
            "ViewModels should expose observable state",
            "Views bind to ViewModel properties",
            "Keep business logic in Models",
        ],
        "clean": [
            "Dependencies point inward",
            "Domain layer has no external dependencies",
            "Use interfaces for dependency inversion",
        ],
        "component-based": [
            "Keep components small and focused",
            "Lift state to nearest common ancestor",
            "Use composition over inheritance",
        ],
    }
    return recommendations.get(arch_type, [])
