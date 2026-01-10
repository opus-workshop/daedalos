"""
Daedalos Loop Tool Library

The core iteration primitive for AI-assisted development.

"A loop is not a feature. A loop is how intelligent work gets done."
"""

__version__ = "1.0.0"

from .promise import verify_promise, verify_promise_with_output, verify_promise_detailed
from .checkpoint import CheckpointBackend, BtrfsCheckpoint, GitCheckpoint, NoneCheckpoint, get_backend
from .agent import AgentAdapter, get_agent, detect_agent, list_available_agents
from .state import Loop, LoopState, LoopStatus, LoopIteration, get_loop, list_loops
from .bestofn import run_best_of_n, BranchResult, BestOfNResult
from .workflow import Workflow, WorkflowRunner, load_workflow
from .notify import notify, notify_loop_complete, NotifyLevel
from .workspace import Workspace, WorkspaceState, Finding, Handoff, SubagentStatus, SubagentRecord
from .subagent import Subagent, SubagentTask, SubagentResult, ParallelSubagentRunner, TEMPLATES
from .orchestrator import Orchestrator, OrchestratorConfig, OrchestratorPhase, run_orchestrated_loop

__all__ = [
    # Promise verification
    "verify_promise",
    "verify_promise_with_output",
    "verify_promise_detailed",
    # Checkpoint backends
    "CheckpointBackend",
    "BtrfsCheckpoint",
    "GitCheckpoint",
    "NoneCheckpoint",
    "get_backend",
    # Agent adapters
    "AgentAdapter",
    "get_agent",
    "detect_agent",
    "list_available_agents",
    # Core loop
    "Loop",
    "LoopState",
    "LoopStatus",
    "LoopIteration",
    "get_loop",
    "list_loops",
    # Best-of-N
    "run_best_of_n",
    "BranchResult",
    "BestOfNResult",
    # Workflows
    "Workflow",
    "WorkflowRunner",
    "load_workflow",
    # Notifications
    "notify",
    "notify_loop_complete",
    "NotifyLevel",
    # Workspace (orchestration)
    "Workspace",
    "WorkspaceState",
    "Finding",
    "Handoff",
    "SubagentStatus",
    "SubagentRecord",
    # Subagents
    "Subagent",
    "SubagentTask",
    "SubagentResult",
    "ParallelSubagentRunner",
    "TEMPLATES",
    # Orchestrator
    "Orchestrator",
    "OrchestratorConfig",
    "OrchestratorPhase",
    "run_orchestrated_loop",
]
