"""Gates configuration management."""

import os
from dataclasses import dataclass, field
from pathlib import Path
from typing import Dict, List, Optional, Any
import json

try:
    import yaml
    HAS_YAML = True
except ImportError:
    HAS_YAML = False


# Supervision levels from most autonomous to most manual
SUPERVISION_LEVELS = [
    "autonomous",   # AI runs freely, only catastrophic actions gated
    "supervised",   # AI runs, human gets notifications, can intervene
    "collaborative", # AI proposes, human approves major actions
    "assisted",     # Human drives, AI suggests and helps
    "manual",       # AI only responds to direct commands
]

# Gate actions
GATE_ACTIONS = [
    "allow",    # Proceed without asking
    "notify",   # Notify but don't block
    "approve",  # Require explicit approval
    "deny",     # Always deny
]

# Default gates for each supervision level
DEFAULT_GATES = {
    "autonomous": {
        "file_delete": "notify",
        "file_create": "allow",
        "file_modify": "allow",
        "git_commit": "allow",
        "git_push": "notify",
        "git_force_push": "approve",
        "loop_start": "allow",
        "agent_spawn": "allow",
        "shell_command": "allow",
        "sensitive_file": "approve",
    },
    "supervised": {
        "file_delete": "approve",
        "file_create": "notify",
        "file_modify": "notify",
        "git_commit": "notify",
        "git_push": "approve",
        "git_force_push": "deny",
        "loop_start": "notify",
        "agent_spawn": "notify",
        "shell_command": "notify",
        "sensitive_file": "approve",
    },
    "collaborative": {
        "file_delete": "approve",
        "file_create": "approve",
        "file_modify": "notify",
        "git_commit": "approve",
        "git_push": "approve",
        "git_force_push": "deny",
        "loop_start": "approve",
        "agent_spawn": "approve",
        "shell_command": "approve",
        "sensitive_file": "approve",
    },
    "assisted": {
        "file_delete": "approve",
        "file_create": "approve",
        "file_modify": "approve",
        "git_commit": "approve",
        "git_push": "approve",
        "git_force_push": "deny",
        "loop_start": "approve",
        "agent_spawn": "approve",
        "shell_command": "approve",
        "sensitive_file": "approve",
    },
    "manual": {
        "file_delete": "approve",
        "file_create": "approve",
        "file_modify": "approve",
        "git_commit": "approve",
        "git_push": "approve",
        "git_force_push": "deny",
        "loop_start": "approve",
        "agent_spawn": "approve",
        "shell_command": "approve",
        "sensitive_file": "approve",
    },
}

# Default autonomy limits
DEFAULT_AUTONOMY = {
    "max_iterations": 50,
    "max_file_changes": 100,
    "max_lines_changed": 1000,
    "sensitive_paths": [
        "*.env",
        "*.env.*",
        ".env*",
        "**/secrets/**",
        "**/credentials/**",
        "**/.ssh/**",
        "**/id_rsa*",
        "**/*.pem",
        "**/*.key",
    ],
}


@dataclass
class SupervisionConfig:
    """Supervision configuration."""

    level: str = "supervised"
    gates: Dict[str, str] = field(default_factory=dict)
    autonomy: Dict[str, Any] = field(default_factory=dict)
    overrides: Dict[str, str] = field(default_factory=dict)  # Per-project overrides

    def __post_init__(self):
        """Apply defaults based on level."""
        if self.level not in SUPERVISION_LEVELS:
            self.level = "supervised"

        # Merge defaults with explicit config
        default_gates = DEFAULT_GATES.get(self.level, DEFAULT_GATES["supervised"])
        self.gates = {**default_gates, **self.gates}

        self.autonomy = {**DEFAULT_AUTONOMY, **self.autonomy}

    def get_gate(self, gate_name: str) -> str:
        """Get the action for a gate, checking overrides first."""
        if gate_name in self.overrides:
            return self.overrides[gate_name]
        return self.gates.get(gate_name, "approve")

    def is_sensitive_path(self, path: str) -> bool:
        """Check if a path matches sensitive patterns."""
        import fnmatch
        path_str = str(path)
        for pattern in self.autonomy.get("sensitive_paths", []):
            if fnmatch.fnmatch(path_str, pattern):
                return True
            # Also check just the filename
            if fnmatch.fnmatch(os.path.basename(path_str), pattern):
                return True
        return False

    def to_dict(self) -> Dict[str, Any]:
        """Convert to dictionary."""
        return {
            "level": self.level,
            "gates": self.gates,
            "autonomy": self.autonomy,
            "overrides": self.overrides,
        }


def get_config_path() -> Path:
    """Get the supervision config file path."""
    config_dir = Path(os.environ.get("XDG_CONFIG_HOME", Path.home() / ".config")) / "daedalos"
    return config_dir / "supervision.yaml"


def load_config() -> SupervisionConfig:
    """Load supervision config from file."""
    config_path = get_config_path()

    if not config_path.exists():
        return SupervisionConfig()

    try:
        content = config_path.read_text()

        if HAS_YAML:
            data = yaml.safe_load(content) or {}
        else:
            # Fallback to JSON if no YAML
            data = json.loads(content)

        return SupervisionConfig(
            level=data.get("level", "supervised"),
            gates=data.get("gates", {}),
            autonomy=data.get("autonomy", {}),
            overrides=data.get("overrides", {}),
        )
    except Exception:
        return SupervisionConfig()


def save_config(config: SupervisionConfig) -> None:
    """Save supervision config to file."""
    config_path = get_config_path()
    config_path.parent.mkdir(parents=True, exist_ok=True)

    data = config.to_dict()

    if HAS_YAML:
        content = yaml.dump(data, default_flow_style=False, sort_keys=False)
    else:
        content = json.dumps(data, indent=2)

    config_path.write_text(content)


def load_project_config(project_path: Optional[Path] = None) -> SupervisionConfig:
    """Load config with project-level overrides."""
    config = load_config()

    if project_path is None:
        project_path = Path.cwd()

    # Check for project-level override file
    project_config = project_path / ".daedalos" / "supervision.yaml"
    if not project_config.exists():
        project_config = project_path / ".daedalos" / "supervision.json"

    if project_config.exists():
        try:
            content = project_config.read_text()
            if project_config.suffix == ".yaml" and HAS_YAML:
                data = yaml.safe_load(content) or {}
            else:
                data = json.loads(content)

            # Apply project overrides
            config.overrides.update(data.get("gates", {}))

            # Project can be MORE restrictive, never less
            project_level = data.get("level")
            if project_level and project_level in SUPERVISION_LEVELS:
                if SUPERVISION_LEVELS.index(project_level) > SUPERVISION_LEVELS.index(config.level):
                    config.level = project_level
                    # Re-apply defaults for new level
                    config = SupervisionConfig(
                        level=config.level,
                        gates=config.gates,
                        autonomy=config.autonomy,
                        overrides=config.overrides,
                    )
        except Exception:
            pass

    return config
