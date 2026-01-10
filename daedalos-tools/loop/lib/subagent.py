"""
Subagent execution for orchestrated loops.

A subagent is a specialized loop that:
1. Runs with a specific objective and template
2. Reports findings back to the workspace
3. Has isolated context but shares results
4. Can run in parallel with other subagents

Templates define the subagent's behavior:
- explorer: Research and find information
- implementer: Make changes to code
- reviewer: Review and verify code
- debugger: Debug and fix issues
- tester: Write and run tests
"""

import os
import base64
from pathlib import Path
from datetime import datetime
from dataclasses import dataclass
from typing import Optional, List, Dict, Any, Callable
from enum import Enum
import subprocess
import threading
import time

from .workspace import (
    Workspace, Finding, Handoff, SubagentStatus,
    SubagentRecord
)
from .agent import AgentAdapter, get_agent, detect_agent
from .checkpoint import CheckpointBackend, get_backend
from .promise import verify_promise, verify_promise_detailed


@dataclass
class SubagentTemplate:
    """Template defining subagent behavior."""
    name: str
    objective_prefix: str
    tools: List[str]
    boundaries: str
    output_format: str

    def format_objective(self, task: str) -> str:
        """Format the full objective."""
        return f"{self.objective_prefix}: {task}"


# Built-in templates
TEMPLATES: Dict[str, SubagentTemplate] = {
    "explorer": SubagentTemplate(
        name="explorer",
        objective_prefix="Research and find information about",
        tools=["read", "grep", "glob", "web_search"],
        boundaries="Read-only. Do not modify any files.",
        output_format="Markdown summary with relevant file paths and code snippets"
    ),
    "implementer": SubagentTemplate(
        name="implementer",
        objective_prefix="Implement the following",
        tools=["read", "write", "edit", "bash"],
        boundaries="Modify only files relevant to the task.",
        output_format="List of files modified and summary of changes"
    ),
    "reviewer": SubagentTemplate(
        name="reviewer",
        objective_prefix="Review and verify",
        tools=["read", "grep", "bash"],
        boundaries="Do not modify files. Report issues only.",
        output_format="List of issues found with severity and location"
    ),
    "debugger": SubagentTemplate(
        name="debugger",
        objective_prefix="Debug and fix",
        tools=["read", "write", "edit", "bash"],
        boundaries="Focus on the specific error. Minimal changes.",
        output_format="Root cause analysis and fix description"
    ),
    "tester": SubagentTemplate(
        name="tester",
        objective_prefix="Write tests for",
        tools=["read", "write", "bash"],
        boundaries="Only create/modify test files.",
        output_format="Test file paths and coverage summary"
    ),
}


@dataclass
class SubagentTask:
    """A task for a subagent to execute."""
    id: str
    template: str
    objective: str
    promise: str
    context: str  # Additional context from orchestrator
    timeout: int = 180  # 3 minutes default
    max_iterations: int = 3


@dataclass
class SubagentResult:
    """Result from a subagent execution."""
    subagent_id: str
    success: bool
    iterations: int
    output: str
    error: Optional[str]
    findings: List[Finding]
    duration_ms: int


class Subagent:
    """
    Executes a subagent loop with a specific objective.

    The subagent:
    1. Receives a task from the orchestrator
    2. Runs a loop with its own promise
    3. Reports findings back to the workspace
    4. Returns a result for synthesis
    """

    def __init__(
        self,
        task: SubagentTask,
        workspace: Workspace,
        working_dir: Path,
        agent: Optional[AgentAdapter] = None,
        checkpoint: Optional[CheckpointBackend] = None,
        on_iteration: Optional[Callable[[int], None]] = None
    ):
        """
        Initialize a subagent.

        Args:
            task: The task to execute
            workspace: Shared workspace
            working_dir: Directory to work in
            agent: Agent adapter (auto-detected if not provided)
            checkpoint: Checkpoint backend (auto-detected if not provided)
            on_iteration: Callback after each iteration
        """
        self.task = task
        self.workspace = workspace
        self.working_dir = Path(working_dir).resolve()
        self.agent = agent or detect_agent()
        self.checkpoint = checkpoint or get_backend(self.working_dir)
        self.on_iteration = on_iteration

        self.template = TEMPLATES.get(task.template, TEMPLATES["implementer"])
        self.iterations = 0
        self.findings: List[Finding] = []

    def _generate_id(self) -> str:
        """Generate a unique ID."""
        return base64.b64encode(os.urandom(4)).decode().replace('+', '-').replace('/', '_')

    def _build_prompt(self, iteration: int) -> str:
        """Build the prompt for the subagent."""
        lines = []

        # Header
        lines.append("=" * 60)
        lines.append(f"SUBAGENT: {self.template.name.upper()}")
        lines.append(f"Iteration {iteration}/{self.task.max_iterations}")
        lines.append("=" * 60)

        # Objective
        objective = self.template.format_objective(self.task.objective)
        lines.append(f"\nOBJECTIVE:\n{objective}")

        # Promise
        lines.append(f"\nSUCCESS CONDITION:")
        lines.append(f"The following command must exit with code 0:")
        lines.append(f"  {self.task.promise}")

        # Boundaries
        lines.append(f"\nBOUNDARIES:")
        lines.append(f"  {self.template.boundaries}")

        # Expected output
        lines.append(f"\nEXPECTED OUTPUT FORMAT:")
        lines.append(f"  {self.template.output_format}")

        # Context from orchestrator/workspace
        if self.task.context:
            lines.append(f"\nCONTEXT FROM ORCHESTRATOR:")
            lines.append(self.task.context)

        # Workspace context
        workspace_context = self.workspace.build_context_for_subagent(self.task.id)
        if workspace_context:
            lines.append(f"\nWORKSPACE CONTEXT:")
            lines.append(workspace_context)

        # Previous iteration feedback
        if iteration > 1 and self.findings:
            lines.append(f"\nPREVIOUS FINDINGS:")
            for f in self.findings[-3:]:  # Last 3 findings
                lines.append(f"- {f.content[:200]}...")

        # Instructions
        lines.append("\n" + "=" * 60)
        lines.append("Complete the objective above. Stay within boundaries.")
        lines.append("=" * 60)

        return "\n".join(lines)

    def _extract_findings(self, output: str) -> List[Finding]:
        """Extract findings from agent output."""
        # Simple extraction - look for file paths and summaries
        findings = []

        # Create a finding from the output
        finding = Finding(
            id=self._generate_id(),
            from_agent=self.task.id,
            type=self.template.name,
            content=output[:2000],  # Truncate long outputs
            files=self._extract_file_paths(output),
            timestamp=datetime.now().isoformat()
        )
        findings.append(finding)

        return findings

    def _extract_file_paths(self, text: str) -> List[str]:
        """Extract file paths from text."""
        import re
        # Match common file path patterns
        patterns = [
            r'(?:^|\s)([./][\w/.-]+\.\w+)',  # Paths starting with . or /
            r'`([^`]+\.\w+)`',  # Paths in backticks
        ]

        paths = set()
        for pattern in patterns:
            for match in re.finditer(pattern, text):
                path = match.group(1)
                # Filter out obvious non-paths
                if not any(x in path for x in ['http', 'www', '<', '>']):
                    paths.add(path)

        return list(paths)[:10]  # Limit to 10 paths

    def run(self) -> SubagentResult:
        """
        Execute the subagent loop.

        Returns:
            SubagentResult with success status and findings
        """
        start_time = time.monotonic()

        # Update workspace with running status
        self.workspace.update_subagent(
            self.task.id,
            status=SubagentStatus.RUNNING
        )

        # Check if promise already met
        if verify_promise(self.task.promise, self.working_dir):
            result = SubagentResult(
                subagent_id=self.task.id,
                success=True,
                iterations=0,
                output="Promise already satisfied",
                error=None,
                findings=[],
                duration_ms=int((time.monotonic() - start_time) * 1000)
            )
            self.workspace.update_subagent(
                self.task.id,
                status=SubagentStatus.COMPLETED,
                promise_result=True,
                output_summary="Promise already satisfied"
            )
            return result

        last_output = ""
        last_error = None

        while self.iterations < self.task.max_iterations:
            self.iterations += 1
            self.workspace.record_subagent_iteration()

            # Build prompt
            prompt = self._build_prompt(self.iterations)

            # Run agent
            try:
                agent_result = self.agent.run(
                    prompt,
                    self.working_dir,
                    timeout=self.task.timeout
                )
                last_output = agent_result.output
                if agent_result.error:
                    last_error = agent_result.error
            except Exception as e:
                last_error = str(e)
                last_output = f"Error running agent: {e}"

            # Extract and store findings
            iteration_findings = self._extract_findings(last_output)
            self.findings.extend(iteration_findings)
            for finding in iteration_findings:
                self.workspace.add_finding(finding)

            # Callback
            if self.on_iteration:
                self.on_iteration(self.iterations)

            # Check promise
            promise_result = verify_promise(self.task.promise, self.working_dir)

            if promise_result:
                # Success!
                result = SubagentResult(
                    subagent_id=self.task.id,
                    success=True,
                    iterations=self.iterations,
                    output=last_output,
                    error=None,
                    findings=self.findings,
                    duration_ms=int((time.monotonic() - start_time) * 1000)
                )
                self.workspace.update_subagent(
                    self.task.id,
                    status=SubagentStatus.COMPLETED,
                    promise_result=True,
                    output_summary=last_output[:500]
                )
                return result

        # Max iterations reached
        result = SubagentResult(
            subagent_id=self.task.id,
            success=False,
            iterations=self.iterations,
            output=last_output,
            error=last_error or "Max iterations reached",
            findings=self.findings,
            duration_ms=int((time.monotonic() - start_time) * 1000)
        )
        self.workspace.update_subagent(
            self.task.id,
            status=SubagentStatus.FAILED,
            promise_result=False,
            output_summary=last_output[:500],
            error=last_error
        )
        return result


class ParallelSubagentRunner:
    """
    Runs multiple subagents in parallel.

    Usage:
        runner = ParallelSubagentRunner(workspace, working_dir)
        results = runner.run_parallel([task1, task2, task3])
    """

    def __init__(
        self,
        workspace: Workspace,
        working_dir: Path,
        max_concurrent: int = 4,
        agent: Optional[AgentAdapter] = None,
        checkpoint: Optional[CheckpointBackend] = None
    ):
        """
        Initialize the parallel runner.

        Args:
            workspace: Shared workspace
            working_dir: Working directory
            max_concurrent: Maximum concurrent subagents
            agent: Agent adapter (shared across subagents)
            checkpoint: Checkpoint backend
        """
        self.workspace = workspace
        self.working_dir = Path(working_dir).resolve()
        self.max_concurrent = max_concurrent
        self.agent = agent
        self.checkpoint = checkpoint

    def run_parallel(
        self,
        tasks: List[SubagentTask],
        on_progress: Optional[Callable[[str, int], None]] = None
    ) -> Dict[str, SubagentResult]:
        """
        Run multiple subagent tasks in parallel.

        Args:
            tasks: List of tasks to run
            on_progress: Callback(subagent_id, iteration) for progress

        Returns:
            Dict mapping subagent_id to result
        """
        from concurrent.futures import ThreadPoolExecutor, as_completed

        results: Dict[str, SubagentResult] = {}

        def run_task(task: SubagentTask) -> SubagentResult:
            subagent = Subagent(
                task=task,
                workspace=self.workspace,
                working_dir=self.working_dir,
                agent=self.agent,
                checkpoint=self.checkpoint,
                on_iteration=lambda i: on_progress(task.id, i) if on_progress else None
            )
            return subagent.run()

        with ThreadPoolExecutor(max_workers=self.max_concurrent) as executor:
            future_to_task = {
                executor.submit(run_task, task): task
                for task in tasks
            }

            for future in as_completed(future_to_task):
                task = future_to_task[future]
                try:
                    result = future.result()
                    results[task.id] = result
                except Exception as e:
                    # Create failure result
                    results[task.id] = SubagentResult(
                        subagent_id=task.id,
                        success=False,
                        iterations=0,
                        output="",
                        error=str(e),
                        findings=[],
                        duration_ms=0
                    )

        return results

    def run_sequential(
        self,
        tasks: List[SubagentTask],
        on_progress: Optional[Callable[[str, int], None]] = None
    ) -> Dict[str, SubagentResult]:
        """
        Run subagent tasks sequentially.

        Useful when tasks depend on each other's results.
        """
        results: Dict[str, SubagentResult] = {}

        for task in tasks:
            subagent = Subagent(
                task=task,
                workspace=self.workspace,
                working_dir=self.working_dir,
                agent=self.agent,
                checkpoint=self.checkpoint,
                on_iteration=lambda i: on_progress(task.id, i) if on_progress else None
            )
            results[task.id] = subagent.run()

            # Stop if critical task failed
            if not results[task.id].success and task.template in ["implementer", "debugger"]:
                break

        return results
