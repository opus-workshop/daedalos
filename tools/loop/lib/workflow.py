"""
Multi-loop workflow engine.

Workflows define multiple coordinated loops with dependencies.
This enables complex, multi-stage development tasks like TDD cycles.
"""

import yaml
import re
from pathlib import Path
from dataclasses import dataclass, field
from typing import Optional, List, Dict, Any, Callable
from enum import Enum
import subprocess

from .state import Loop, LoopState, LoopStatus
from .checkpoint import get_backend
from .agent import get_agent, detect_agent


class WorkflowStatus(Enum):
    """Status of a workflow execution."""
    PENDING = "pending"
    RUNNING = "running"
    COMPLETED = "completed"
    FAILED = "failed"
    CANCELLED = "cancelled"


@dataclass
class WorkflowLoop:
    """Definition of a loop within a workflow."""
    id: str
    prompt: str
    promise: str
    max_iterations: int = 10
    depends_on: List[str] = field(default_factory=list)
    agent: Optional[str] = None
    timeout: int = 300


@dataclass
class Workflow:
    """
    A multi-loop workflow definition.

    Workflows coordinate multiple loops with dependencies,
    enabling complex multi-stage tasks.
    """
    name: str
    description: str
    loops: List[WorkflowLoop]
    env: Dict[str, str] = field(default_factory=dict)
    on_complete: List[str] = field(default_factory=list)
    on_failure: List[str] = field(default_factory=list)
    defaults: Dict[str, Any] = field(default_factory=dict)

    @classmethod
    def from_yaml(cls, path: Path, variables: Dict[str, str] = None) -> "Workflow":
        """
        Load workflow from YAML file.

        Args:
            path: Path to YAML file
            variables: Variables to substitute ({{var}} syntax)

        Returns:
            Workflow instance
        """
        with open(path) as f:
            data = yaml.safe_load(f)

        # Substitute variables
        variables = variables or {}
        data_str = yaml.dump(data)
        for key, value in variables.items():
            data_str = data_str.replace(f"{{{{{key}}}}}", value)
        data = yaml.safe_load(data_str)

        # Parse loops
        loops = []
        for loop_data in data.get("loops", []):
            loops.append(WorkflowLoop(
                id=loop_data["id"],
                prompt=loop_data["prompt"],
                promise=loop_data["promise"],
                max_iterations=loop_data.get("max_iterations", 10),
                depends_on=loop_data.get("depends_on", []),
                agent=loop_data.get("agent"),
                timeout=loop_data.get("timeout", 300)
            ))

        return cls(
            name=data.get("name", path.stem),
            description=data.get("description", ""),
            loops=loops,
            env=data.get("env", {}),
            on_complete=data.get("on_complete", []),
            on_failure=data.get("on_failure", []),
            defaults=data.get("defaults", {})
        )

    def to_yaml(self) -> str:
        """Convert workflow to YAML string."""
        data = {
            "name": self.name,
            "description": self.description,
            "loops": [
                {
                    "id": loop.id,
                    "prompt": loop.prompt,
                    "promise": loop.promise,
                    "max_iterations": loop.max_iterations,
                    "depends_on": loop.depends_on,
                    "agent": loop.agent,
                    "timeout": loop.timeout
                }
                for loop in self.loops
            ],
            "env": self.env,
            "on_complete": self.on_complete,
            "on_failure": self.on_failure,
            "defaults": self.defaults
        }
        return yaml.dump(data, default_flow_style=False)


@dataclass
class WorkflowExecution:
    """State of a workflow execution."""
    workflow: Workflow
    status: WorkflowStatus
    completed_loops: List[str]
    failed_loop: Optional[str]
    loop_states: Dict[str, LoopState]
    current_loop: Optional[str]
    error_message: Optional[str] = None


class WorkflowRunner:
    """
    Executes multi-loop workflows.

    Handles:
    - Dependency resolution
    - Loop sequencing
    - Failure handling
    - Hook execution
    """

    def __init__(
        self,
        workflow: Workflow,
        working_dir: Path,
        agent_name: Optional[str] = None,
        on_loop_start: Optional[Callable[[str], None]] = None,
        on_loop_complete: Optional[Callable[[str, bool], None]] = None
    ):
        self.workflow = workflow
        self.working_dir = Path(working_dir).resolve()
        self.agent_name = agent_name
        self.on_loop_start = on_loop_start
        self.on_loop_complete = on_loop_complete

        self.execution = WorkflowExecution(
            workflow=workflow,
            status=WorkflowStatus.PENDING,
            completed_loops=[],
            failed_loop=None,
            loop_states={},
            current_loop=None
        )

    def _get_ready_loops(self) -> List[WorkflowLoop]:
        """Get loops whose dependencies are satisfied."""
        ready = []
        for loop in self.workflow.loops:
            if loop.id in self.execution.completed_loops:
                continue
            if loop.id == self.execution.failed_loop:
                continue

            # Check if all dependencies are met
            deps_met = all(
                dep in self.execution.completed_loops
                for dep in loop.depends_on
            )
            if deps_met:
                ready.append(loop)

        return ready

    def _run_hook(self, command: str):
        """Run a hook command."""
        # Substitute variables
        cmd = command
        if self.execution.failed_loop:
            cmd = cmd.replace("{{failed_loop}}", self.execution.failed_loop)

        try:
            subprocess.run(
                cmd,
                shell=True,
                cwd=self.working_dir,
                timeout=60,
                capture_output=True
            )
        except:
            pass

    def _run_loop(self, wf_loop: WorkflowLoop) -> bool:
        """Run a single workflow loop."""
        self.execution.current_loop = wf_loop.id

        if self.on_loop_start:
            self.on_loop_start(wf_loop.id)

        # Get agent
        agent_name = wf_loop.agent or self.agent_name
        if agent_name:
            agent = get_agent(agent_name)
        else:
            agent = detect_agent()
            if not agent:
                self.execution.error_message = "No agent available"
                return False

        # Get checkpoint backend
        checkpoint = get_backend(self.working_dir)

        # Get max iterations from loop or workflow defaults
        max_iter = wf_loop.max_iterations
        if max_iter == 10 and "max_iterations" in self.workflow.defaults:
            max_iter = self.workflow.defaults["max_iterations"]

        # Create and run loop
        loop = Loop(
            prompt=wf_loop.prompt,
            promise_cmd=wf_loop.promise,
            working_dir=self.working_dir,
            agent=agent,
            checkpoint=checkpoint,
            max_iterations=max_iter,
            timeout=wf_loop.timeout
        )

        success = loop.run()

        # Store state
        self.execution.loop_states[wf_loop.id] = loop.state

        if self.on_loop_complete:
            self.on_loop_complete(wf_loop.id, success)

        return success

    def run(self) -> bool:
        """
        Execute the workflow.

        Returns:
            True if all loops completed successfully
        """
        self.execution.status = WorkflowStatus.RUNNING

        # Set environment variables
        import os
        for key, value in self.workflow.env.items():
            os.environ[key] = value

        while True:
            ready_loops = self._get_ready_loops()

            if not ready_loops:
                # Check if we're done or stuck
                if len(self.execution.completed_loops) == len(self.workflow.loops):
                    # All loops completed
                    self.execution.status = WorkflowStatus.COMPLETED
                    for cmd in self.workflow.on_complete:
                        self._run_hook(cmd)
                    return True
                else:
                    # No loops ready and not all complete = failure
                    break

            # Run the first ready loop (could parallelize independent loops)
            wf_loop = ready_loops[0]
            success = self._run_loop(wf_loop)

            if success:
                self.execution.completed_loops.append(wf_loop.id)
            else:
                self.execution.failed_loop = wf_loop.id
                self.execution.status = WorkflowStatus.FAILED
                for cmd in self.workflow.on_failure:
                    self._run_hook(cmd)
                return False

        self.execution.status = WorkflowStatus.FAILED
        return False

    def cancel(self):
        """Cancel the workflow."""
        self.execution.status = WorkflowStatus.CANCELLED


def load_workflow(path: Path, variables: Dict[str, str] = None) -> Workflow:
    """
    Load a workflow from file.

    Args:
        path: Path to workflow YAML file
        variables: Variables to substitute

    Returns:
        Workflow instance
    """
    return Workflow.from_yaml(path, variables)


def list_builtin_workflows(templates_dir: Optional[Path] = None) -> List[str]:
    """List available built-in workflow templates."""
    if templates_dir is None:
        templates_dir = Path(__file__).parent.parent / "templates"

    workflows = []
    if templates_dir.exists():
        for path in templates_dir.glob("*.yaml"):
            workflows.append(path.stem)

    return workflows
