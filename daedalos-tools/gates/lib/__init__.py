"""Gates - Configurable approval checkpoints for Daedalos."""

from .config import (
    SupervisionConfig,
    load_config,
    save_config,
    load_project_config,
    SUPERVISION_LEVELS,
    GATE_ACTIONS,
    DEFAULT_GATES,
)

from .checker import (
    GateRequest,
    GateResult,
    check_gate,
    check_autonomy_limits,
    get_gate_history,
)

__all__ = [
    "SupervisionConfig",
    "load_config",
    "save_config",
    "load_project_config",
    "SUPERVISION_LEVELS",
    "GATE_ACTIONS",
    "DEFAULT_GATES",
    "GateRequest",
    "GateResult",
    "check_gate",
    "check_autonomy_limits",
    "get_gate_history",
]
