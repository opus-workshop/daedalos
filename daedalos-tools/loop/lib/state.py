"""
Core loop state management and execution.

This is the heart of Daedalos - the loop execution engine that implements
the Ralph Wiggum technique: iterate until done.

The Loop class orchestrates:
1. Checkpoint creation at each iteration
2. Agent execution with context
3. Promise verification
4. State persistence for pause/resume
"""

import json
import os
import sys
import base64
import time
from pathlib import Path
from datetime import datetime
from dataclasses import dataclass, asdict, field
from typing import Optional, Callable, List, Any
from enum import Enum
import subprocess

from .checkpoint import CheckpointBackend, get_backend
from .agent import AgentAdapter, get_agent, detect_agent
from .promise import verify_promise, verify_promise_detailed, PromiseResult


class LoopStatus(Enum):
    """Status of a loop execution."""
    PENDING = "pending"
    RUNNING = "running"
    PAUSED = "paused"
    COMPLETED = "completed"
    FAILED = "failed"
    CANCELLED = "cancelled"


@dataclass
class LoopIteration:
    """Record of a single loop iteration."""
    number: int
    started_at: str
    completed_at: Optional[str]
    checkpoint_id: str
    promise_result: Optional[bool]
    promise_output: str
    agent_output: str
    agent_error: str
    changes_summary: str
    duration_ms: int


@dataclass
class LoopState:
    """
    Persistent state for a loop.

    This state is saved to disk after each iteration, enabling:
    - Pause and resume
    - Progress inspection
    - Post-mortem analysis
    """
    id: str
    prompt: str
    promise_cmd: str
    status: LoopStatus
    working_dir: str
    agent_name: str
    max_iterations: int
    current_iteration: int
    iterations: List[LoopIteration]
    created_at: str
    updated_at: str
    initial_checkpoint: str
    injected_context: List[str]
    template: Optional[str] = None
    error_message: Optional[str] = None

    def to_dict(self) -> dict:
        """Convert state to dictionary for JSON serialization."""
        d = asdict(self)
        d["status"] = self.status.value
        return d

    @classmethod
    def from_dict(cls, d: dict) -> "LoopState":
        """Create state from dictionary."""
        d["status"] = LoopStatus(d["status"])
        d["iterations"] = [
            LoopIteration(**i) if isinstance(i, dict) else i
            for i in d.get("iterations", [])
        ]
        return cls(**d)

    def save(self, state_dir: Path):
        """Save state to disk."""
        state_dir.mkdir(parents=True, exist_ok=True)
        path = state_dir / f"{self.id}.json"
        with open(path, "w") as f:
            json.dump(self.to_dict(), f, indent=2)

    @classmethod
    def load(cls, state_dir: Path, loop_id: str) -> "LoopState":
        """Load state from disk."""
        path = state_dir / f"{loop_id}.json"
        with open(path) as f:
            return cls.from_dict(json.load(f))

    @classmethod
    def list_all(cls, state_dir: Path) -> List["LoopState"]:
        """List all saved loop states."""
        states = []
        if state_dir.exists():
            for path in state_dir.glob("*.json"):
                try:
                    states.append(cls.load(state_dir, path.stem))
                except (json.JSONDecodeError, KeyError, TypeError):
                    continue
        return states


class Loop:
    """
    Main loop execution engine.

    This implements the core iteration primitive of Daedalos.

    Usage:
        loop = Loop(
            prompt="fix the failing tests",
            promise_cmd="npm test",
            working_dir=Path("."),
            agent=get_agent("claude"),
            checkpoint=get_backend(Path("."))
        )
        success = loop.run()
    """

    def __init__(
        self,
        prompt: str,
        promise_cmd: str,
        working_dir: Path,
        agent: AgentAdapter,
        checkpoint: CheckpointBackend,
        max_iterations: int = 10,
        timeout: int = 300,
        on_iteration: Optional[Callable[[LoopIteration], None]] = None,
        on_status_change: Optional[Callable[[LoopStatus], None]] = None,
        state_dir: Optional[Path] = None,
        loop_id: Optional[str] = None,
        template: Optional[str] = None
    ):
        """
        Initialize a new loop.

        Args:
            prompt: Natural language task description
            promise_cmd: Shell command that returns 0 when done
            working_dir: Directory for the agent to work in
            agent: Agent adapter to use
            checkpoint: Checkpoint backend to use
            max_iterations: Maximum iterations before giving up
            timeout: Per-iteration timeout in seconds
            on_iteration: Callback after each iteration
            on_status_change: Callback on status changes
            state_dir: Directory for state persistence
            loop_id: Optional ID (auto-generated if not provided)
            template: Name of template being used (for tracking)
        """
        self.prompt = prompt
        self.promise_cmd = promise_cmd
        self.working_dir = Path(working_dir).resolve()
        self.agent = agent
        self.checkpoint = checkpoint
        self.max_iterations = max_iterations
        self.timeout = timeout
        self.on_iteration = on_iteration
        self.on_status_change = on_status_change
        self.state_dir = state_dir or (
            Path.home() / ".local/share/daedalos/loop/states"
        )
        self.state_dir.mkdir(parents=True, exist_ok=True)

        # Initialize state
        self.state = LoopState(
            id=loop_id or self._generate_id(),
            prompt=prompt,
            promise_cmd=promise_cmd,
            status=LoopStatus.PENDING,
            working_dir=str(self.working_dir),
            agent_name=agent.name,
            max_iterations=max_iterations,
            current_iteration=0,
            iterations=[],
            created_at=datetime.now().isoformat(),
            updated_at=datetime.now().isoformat(),
            initial_checkpoint="",
            injected_context=[],
            template=template
        )

    def _generate_id(self) -> str:
        """Generate a unique loop ID."""
        return base64.b64encode(os.urandom(6)).decode().replace('+', '-').replace('/', '_')

    def _set_status(self, status: LoopStatus):
        """Update loop status and trigger callback."""
        self.state.status = status
        self.state.updated_at = datetime.now().isoformat()
        self.state.save(self.state_dir)
        if self.on_status_change:
            self.on_status_change(status)

    def _build_prompt(self, iteration: int) -> str:
        """
        Build the full prompt for an iteration.

        Includes:
        - Task description
        - Iteration number
        - Promise command
        - Injected context
        - Previous iteration feedback
        """
        parts = []

        # Header
        parts.append("=" * 60)
        parts.append(f"LOOP ITERATION {iteration}/{self.max_iterations}")
        parts.append("=" * 60)

        # Task
        parts.append(f"\nTASK:\n{self.prompt}")

        # Promise
        parts.append(f"\nSUCCESS CONDITION:")
        parts.append(f"The following command must exit with code 0:")
        parts.append(f"  {self.promise_cmd}")

        # Injected context
        if self.state.injected_context:
            parts.append("\nADDITIONAL CONTEXT:")
            for ctx in self.state.injected_context:
                parts.append(f"- {ctx}")

        # Previous iteration feedback
        if iteration > 1 and self.state.iterations:
            last = self.state.iterations[-1]
            parts.append(f"\nPREVIOUS ITERATION ({iteration - 1}) RESULT:")

            if last.promise_result:
                parts.append("  Status: PASSED")
            else:
                parts.append("  Status: FAILED")
                if last.promise_output:
                    parts.append("  Output:")
                    for line in last.promise_output.split('\n')[:20]:
                        parts.append(f"    {line}")

            parts.append("\nAnalyze what went wrong and try a different approach.")

        # Instructions
        parts.append("\n" + "=" * 60)
        parts.append("INSTRUCTIONS:")
        parts.append("Make changes to the codebase to satisfy the success condition.")
        parts.append("Focus on the specific task. Make minimal, targeted changes.")
        parts.append("=" * 60)

        return "\n".join(parts)

    def _get_changes_summary(self) -> str:
        """Get a summary of file changes since last checkpoint."""
        try:
            result = subprocess.run(
                ["git", "diff", "--stat", "HEAD"],
                cwd=self.working_dir,
                capture_output=True,
                text=True,
                timeout=10
            )
            if result.stdout.strip():
                return result.stdout.strip()

            # Try unstaged changes
            result = subprocess.run(
                ["git", "diff", "--stat"],
                cwd=self.working_dir,
                capture_output=True,
                text=True,
                timeout=10
            )
            return result.stdout.strip() if result.stdout else "No changes detected"
        except (subprocess.TimeoutExpired, FileNotFoundError):
            return "Unable to detect changes"

    def run(self) -> bool:
        """
        Execute the loop until promise is met or max iterations reached.

        Returns:
            True if promise was met, False otherwise
        """
        # Check if promise is already met
        if verify_promise(self.promise_cmd, self.working_dir):
            self._set_status(LoopStatus.COMPLETED)
            return True

        # Create initial checkpoint
        try:
            self.state.initial_checkpoint = self.checkpoint.create(
                f"{self.state.id}_initial",
                self.working_dir
            )
        except Exception as e:
            self.state.error_message = f"Failed to create initial checkpoint: {e}"
            self._set_status(LoopStatus.FAILED)
            return False

        self._set_status(LoopStatus.RUNNING)

        while self.state.current_iteration < self.max_iterations:
            # Check for pause/cancel
            if self.state.status == LoopStatus.PAUSED:
                time.sleep(1)
                # Reload state in case it changed
                try:
                    self.state = LoopState.load(self.state_dir, self.state.id)
                except:
                    pass
                continue

            if self.state.status == LoopStatus.CANCELLED:
                return False

            # Run iteration
            success = self._run_iteration()
            if success:
                self._set_status(LoopStatus.COMPLETED)
                return True

        # Max iterations reached
        self.state.error_message = f"Max iterations ({self.max_iterations}) reached without meeting promise"
        self._set_status(LoopStatus.FAILED)
        return False

    def _run_iteration(self) -> bool:
        """
        Execute a single loop iteration.

        Returns:
            True if promise was met, False otherwise
        """
        self.state.current_iteration += 1
        iteration_num = self.state.current_iteration
        start_time = time.monotonic()

        # Create checkpoint for this iteration
        try:
            checkpoint_id = self.checkpoint.create(
                f"{self.state.id}_iter{iteration_num}",
                self.working_dir
            )
        except Exception as e:
            checkpoint_id = f"failed_{iteration_num}"

        # Build iteration record
        iteration = LoopIteration(
            number=iteration_num,
            started_at=datetime.now().isoformat(),
            completed_at=None,
            checkpoint_id=checkpoint_id,
            promise_result=None,
            promise_output="",
            agent_output="",
            agent_error="",
            changes_summary="",
            duration_ms=0
        )

        # Build and run agent
        prompt = self._build_prompt(iteration_num)
        agent_result = self.agent.run(
            prompt,
            self.working_dir,
            timeout=self.timeout
        )

        iteration.agent_output = agent_result.output
        iteration.agent_error = agent_result.error

        # Check the promise
        promise_result = verify_promise_detailed(
            self.promise_cmd,
            self.working_dir
        )

        iteration.promise_result = promise_result.success
        iteration.promise_output = (
            promise_result.stdout +
            ("\n" + promise_result.stderr if promise_result.stderr else "")
        )

        # Finalize iteration
        iteration.completed_at = datetime.now().isoformat()
        iteration.changes_summary = self._get_changes_summary()
        iteration.duration_ms = int((time.monotonic() - start_time) * 1000)

        self.state.iterations.append(iteration)
        self.state.updated_at = datetime.now().isoformat()
        self.state.save(self.state_dir)

        # Trigger callback
        if self.on_iteration:
            self.on_iteration(iteration)

        return promise_result.success

    def pause(self):
        """Pause the loop after current iteration."""
        self._set_status(LoopStatus.PAUSED)

    def resume(self):
        """Resume a paused loop."""
        if self.state.status == LoopStatus.PAUSED:
            self._set_status(LoopStatus.RUNNING)

    def cancel(self):
        """Cancel the loop."""
        self._set_status(LoopStatus.CANCELLED)

    def inject_context(self, context: str):
        """Inject additional context for the next iteration."""
        self.state.injected_context.append(context)
        self.state.save(self.state_dir)

    def rollback(self, checkpoint_id: str) -> bool:
        """Rollback to a previous checkpoint."""
        return self.checkpoint.restore(checkpoint_id, self.working_dir)

    def rollback_to_initial(self) -> bool:
        """Rollback to the initial state before the loop started."""
        if self.state.initial_checkpoint:
            return self.rollback(self.state.initial_checkpoint)
        return False

    @classmethod
    def resume_from_state(
        cls,
        state: LoopState,
        agent: Optional[AgentAdapter] = None,
        checkpoint: Optional[CheckpointBackend] = None,
        **kwargs
    ) -> "Loop":
        """
        Resume a loop from saved state.

        Args:
            state: Previously saved loop state
            agent: Agent to use (auto-detects if not provided)
            checkpoint: Checkpoint backend (auto-detects if not provided)
            **kwargs: Additional arguments to override

        Returns:
            Loop instance ready to resume
        """
        working_dir = Path(state.working_dir)

        if agent is None:
            agent = get_agent(state.agent_name)
        if checkpoint is None:
            checkpoint = get_backend(working_dir)

        loop = cls(
            prompt=state.prompt,
            promise_cmd=state.promise_cmd,
            working_dir=working_dir,
            agent=agent,
            checkpoint=checkpoint,
            max_iterations=state.max_iterations,
            loop_id=state.id,
            template=state.template,
            **kwargs
        )

        # Restore state
        loop.state = state

        return loop


def get_loop(loop_id: str, state_dir: Optional[Path] = None) -> Optional[LoopState]:
    """
    Get loop state by ID.

    Args:
        loop_id: The loop identifier
        state_dir: Directory containing state files

    Returns:
        LoopState if found, None otherwise
    """
    state_dir = state_dir or Path.home() / ".local/share/daedalos/loop/states"
    try:
        return LoopState.load(state_dir, loop_id)
    except FileNotFoundError:
        return None


def list_loops(
    state_dir: Optional[Path] = None,
    status: Optional[LoopStatus] = None
) -> List[LoopState]:
    """
    List all loops, optionally filtered by status.

    Args:
        state_dir: Directory containing state files
        status: Filter to only this status

    Returns:
        List of LoopState objects
    """
    state_dir = state_dir or Path.home() / ".local/share/daedalos/loop/states"
    states = LoopState.list_all(state_dir)

    if status:
        states = [s for s in states if s.status == status]

    return sorted(states, key=lambda s: s.updated_at, reverse=True)
