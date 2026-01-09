"""
Orchestrator for multi-agent coordination.

The orchestrator:
1. Analyzes tasks and creates execution plans
2. Spawns subagents (parallel or sequential)
3. Synthesizes results from subagents
4. Decides next steps based on failures
5. Manages the overall loop iteration

This implements the orchestrator-worker pattern from Anthropic's
multi-agent research system (90% improvement over single-agent).
"""

import os
import base64
from pathlib import Path
from datetime import datetime
from dataclasses import dataclass
from typing import Optional, List, Dict, Callable, Tuple
from enum import Enum
import json

from .workspace import Workspace, Finding, Handoff, SubagentStatus
from .subagent import (
    Subagent, SubagentTask, SubagentResult,
    ParallelSubagentRunner, TEMPLATES
)
from .agent import AgentAdapter, get_agent, detect_agent
from .checkpoint import CheckpointBackend, get_backend
from .promise import verify_promise, verify_promise_detailed, PromiseResult


class OrchestratorPhase(Enum):
    """Phases of orchestrated execution."""
    PLANNING = "planning"
    RESEARCH = "research"
    IMPLEMENTATION = "implementation"
    VERIFICATION = "verification"
    SYNTHESIS = "synthesis"


@dataclass
class OrchestratorConfig:
    """Configuration for the orchestrator."""
    max_subagents: int = 4
    max_subagent_iterations: int = 3
    subagent_timeout: int = 180
    parallel_research: bool = True
    parallel_implementation: bool = False  # Usually sequential for coherence
    verbose: bool = False


class TaskAnalyzer:
    """
    Analyzes tasks to determine orchestration strategy.

    Uses heuristics to determine:
    - Whether task needs research phase
    - How many parallel explorers to spawn
    - Whether to use reviewers
    """

    # Keywords indicating research is needed
    RESEARCH_KEYWORDS = [
        "implement", "add", "create", "build", "new",
        "feature", "integration", "connect", "setup"
    ]

    # Keywords indicating debugging
    DEBUG_KEYWORDS = [
        "fix", "bug", "error", "broken", "failing", "crash",
        "issue", "problem", "wrong", "incorrect"
    ]

    # Keywords indicating refactoring
    REFACTOR_KEYWORDS = [
        "refactor", "clean", "improve", "optimize", "restructure",
        "rename", "move", "extract", "simplify"
    ]

    @classmethod
    def analyze(cls, task: str) -> Dict:
        """
        Analyze a task and return strategy recommendations.

        Returns:
            Dict with:
            - needs_research: bool
            - task_type: str (feature, bugfix, refactor)
            - suggested_phases: List[str]
            - parallel_explorers: int
        """
        task_lower = task.lower()

        # Determine task type
        if any(kw in task_lower for kw in cls.DEBUG_KEYWORDS):
            task_type = "bugfix"
            needs_research = True  # Need to understand the bug
            phases = ["research", "debug", "verify"]
            explorers = 2  # One for error context, one for related code
        elif any(kw in task_lower for kw in cls.REFACTOR_KEYWORDS):
            task_type = "refactor"
            needs_research = True  # Need to understand current structure
            phases = ["research", "implement", "verify"]
            explorers = 2
        elif any(kw in task_lower for kw in cls.RESEARCH_KEYWORDS):
            task_type = "feature"
            needs_research = True
            phases = ["research", "implement", "verify"]
            explorers = 3  # More exploration for new features
        else:
            task_type = "general"
            needs_research = False
            phases = ["implement", "verify"]
            explorers = 1

        return {
            "needs_research": needs_research,
            "task_type": task_type,
            "suggested_phases": phases,
            "parallel_explorers": explorers
        }


class Orchestrator:
    """
    Orchestrates multi-agent execution of complex tasks.

    The orchestrator implements the orchestrator-worker pattern:
    1. Analyze task and create plan
    2. Execute phases (research -> implement -> verify)
    3. Spawn parallel subagents where beneficial
    4. Synthesize results
    5. Handle failures with targeted retry

    Usage:
        orchestrator = Orchestrator(
            task="implement user authentication",
            promise="npm test",
            working_dir=Path("."),
            config=OrchestratorConfig()
        )
        success = orchestrator.run()
    """

    def __init__(
        self,
        task: str,
        promise: str,
        working_dir: Path,
        config: Optional[OrchestratorConfig] = None,
        agent: Optional[AgentAdapter] = None,
        checkpoint: Optional[CheckpointBackend] = None,
        loop_id: Optional[str] = None,
        on_phase: Optional[Callable[[OrchestratorPhase], None]] = None,
        on_subagent: Optional[Callable[[str, SubagentStatus], None]] = None
    ):
        """
        Initialize the orchestrator.

        Args:
            task: Main task description
            promise: Promise command (must exit 0 for success)
            working_dir: Working directory
            config: Orchestrator configuration
            agent: Agent adapter for subagents
            checkpoint: Checkpoint backend
            loop_id: Optional ID (auto-generated if not provided)
            on_phase: Callback when phase changes
            on_subagent: Callback when subagent status changes
        """
        self.task = task
        self.promise = promise
        self.working_dir = Path(working_dir).resolve()
        self.config = config or OrchestratorConfig()
        self.agent = agent or detect_agent()
        self.checkpoint = checkpoint or get_backend(self.working_dir)
        self.loop_id = loop_id or self._generate_id()
        self.on_phase = on_phase
        self.on_subagent = on_subagent

        # Create workspace
        self.workspace = Workspace.create(
            self.loop_id,
            task,
            promise
        )

        # State
        self.current_phase: Optional[OrchestratorPhase] = None
        self.iteration = 0
        self.subagent_counter = 0

    def _generate_id(self) -> str:
        """Generate a unique ID."""
        return base64.b64encode(os.urandom(6)).decode().replace('+', '-').replace('/', '_')

    def _set_phase(self, phase: OrchestratorPhase):
        """Update current phase and trigger callback."""
        self.current_phase = phase
        if self.on_phase:
            self.on_phase(phase)

    def _next_subagent_id(self, template: str) -> str:
        """Generate next subagent ID."""
        self.subagent_counter += 1
        return f"{template}-{self.subagent_counter}"

    def _create_research_tasks(self, analysis: Dict) -> List[SubagentTask]:
        """Create research tasks based on analysis."""
        tasks = []
        num_explorers = min(
            analysis["parallel_explorers"],
            self.config.max_subagents
        )

        if analysis["task_type"] == "bugfix":
            # Research the error and related code
            tasks.append(SubagentTask(
                id=self._next_subagent_id("explorer"),
                template="explorer",
                objective=f"Find the root cause of: {self.task}",
                promise=f"test -f {self.workspace.path}/findings/explorer-{self.subagent_counter}.txt",
                context="Focus on error messages, stack traces, and recent changes.",
                timeout=self.config.subagent_timeout,
                max_iterations=self.config.max_subagent_iterations
            ))
            tasks.append(SubagentTask(
                id=self._next_subagent_id("explorer"),
                template="explorer",
                objective=f"Find code related to: {self.task}",
                promise=f"test -f {self.workspace.path}/findings/explorer-{self.subagent_counter}.txt",
                context="Look for relevant functions, tests, and dependencies.",
                timeout=self.config.subagent_timeout,
                max_iterations=self.config.max_subagent_iterations
            ))
        else:
            # General research for feature/refactor
            aspects = [
                "existing patterns and conventions in the codebase",
                "relevant files and functions",
                "dependencies and imports needed"
            ]
            for i, aspect in enumerate(aspects[:num_explorers]):
                tasks.append(SubagentTask(
                    id=self._next_subagent_id("explorer"),
                    template="explorer",
                    objective=f"Research {aspect} for: {self.task}",
                    promise=f"test -f {self.workspace.path}/findings/explorer-{self.subagent_counter}.txt",
                    context=f"Focus on: {aspect}",
                    timeout=self.config.subagent_timeout,
                    max_iterations=self.config.max_subagent_iterations
                ))

        return tasks

    def _create_implementation_task(self, analysis: Dict) -> SubagentTask:
        """Create the main implementation task."""
        context_lines = []

        # Add findings summary
        findings_summary = self.workspace.get_findings_summary()
        if findings_summary != "No findings yet.":
            context_lines.append("RESEARCH FINDINGS:")
            context_lines.append(findings_summary)

        # Add task-specific guidance
        if analysis["task_type"] == "bugfix":
            template = "debugger"
            context_lines.append("\nApproach: Fix the root cause identified in research.")
        elif analysis["task_type"] == "refactor":
            template = "implementer"
            context_lines.append("\nApproach: Refactor while maintaining behavior.")
        else:
            template = "implementer"
            context_lines.append("\nApproach: Implement the feature using patterns found in research.")

        return SubagentTask(
            id=self._next_subagent_id(template),
            template=template,
            objective=self.task,
            promise=self.promise,
            context="\n".join(context_lines),
            timeout=self.config.subagent_timeout * 2,  # More time for implementation
            max_iterations=self.config.max_subagent_iterations * 2
        )

    def _create_verification_task(self) -> SubagentTask:
        """Create a verification/review task."""
        context_lines = []

        # Add implementation summary
        impl_subagents = [
            s for s in self.workspace.state.subagents.values()
            if s.type in ["implementer", "debugger"]
        ]
        if impl_subagents:
            context_lines.append("IMPLEMENTATION SUMMARY:")
            for s in impl_subagents:
                context_lines.append(f"- {s.id}: {s.output_summary[:200]}")

        return SubagentTask(
            id=self._next_subagent_id("reviewer"),
            template="reviewer",
            objective=f"Review the implementation of: {self.task}",
            promise=self.promise,
            context="\n".join(context_lines),
            timeout=self.config.subagent_timeout,
            max_iterations=2  # Quick review
        )

    def _run_phase(
        self,
        phase: OrchestratorPhase,
        tasks: List[SubagentTask],
        parallel: bool = True
    ) -> Dict[str, SubagentResult]:
        """Run a phase with given tasks."""
        self._set_phase(phase)

        # Register subagents in workspace
        for task in tasks:
            self.workspace.register_subagent(
                task.id,
                task.template,
                task.objective
            )

        # Create runner
        runner = ParallelSubagentRunner(
            workspace=self.workspace,
            working_dir=self.working_dir,
            max_concurrent=self.config.max_subagents if parallel else 1,
            agent=self.agent,
            checkpoint=self.checkpoint
        )

        # Progress callback
        def on_progress(subagent_id: str, iteration: int):
            if self.on_subagent:
                status = self.workspace.get_subagent(subagent_id)
                if status:
                    self.on_subagent(subagent_id, status.status)

        # Run tasks
        if parallel:
            results = runner.run_parallel(tasks, on_progress)
        else:
            results = runner.run_sequential(tasks, on_progress)

        return results

    def _synthesize_results(self, results: Dict[str, SubagentResult]) -> str:
        """Synthesize results from multiple subagents."""
        lines = ["SYNTHESIS OF SUBAGENT RESULTS:", "=" * 40]

        successes = [r for r in results.values() if r.success]
        failures = [r for r in results.values() if not r.success]

        lines.append(f"\nSuccessful: {len(successes)}/{len(results)}")

        if successes:
            lines.append("\nSUCCESSFUL SUBAGENTS:")
            for r in successes:
                lines.append(f"\n[{r.subagent_id}]")
                lines.append(f"  Iterations: {r.iterations}")
                # Summarize findings
                if r.findings:
                    lines.append("  Key findings:")
                    for f in r.findings[:2]:
                        lines.append(f"    - {f.content[:100]}...")

        if failures:
            lines.append("\nFAILED SUBAGENTS:")
            for r in failures:
                lines.append(f"\n[{r.subagent_id}]")
                lines.append(f"  Error: {r.error or 'Max iterations reached'}")

        return "\n".join(lines)

    def _plan_retry(
        self,
        failed_results: Dict[str, SubagentResult],
        promise_result: PromiseResult
    ) -> List[SubagentTask]:
        """Plan retry tasks based on failures."""
        tasks = []

        # Analyze the failure
        error_context = []
        if promise_result.stderr:
            error_context.append(f"Promise error: {promise_result.stderr[:500]}")
        if promise_result.stdout:
            error_context.append(f"Promise output: {promise_result.stdout[:500]}")

        # Create targeted debug task
        tasks.append(SubagentTask(
            id=self._next_subagent_id("debugger"),
            template="debugger",
            objective=f"Fix the failure in: {self.task}",
            promise=self.promise,
            context="\n".join([
                "PREVIOUS ATTEMPT FAILED",
                "",
                *error_context,
                "",
                "SYNTHESIS OF PREVIOUS WORK:",
                self._synthesize_results(failed_results)
            ]),
            timeout=self.config.subagent_timeout * 2,
            max_iterations=self.config.max_subagent_iterations * 2
        ))

        return tasks

    def run(self, max_iterations: int = 3) -> bool:
        """
        Run the orchestrated loop.

        Args:
            max_iterations: Maximum orchestration iterations

        Returns:
            True if main promise was met, False otherwise
        """
        # Check if promise already met
        if verify_promise(self.promise, self.working_dir):
            return True

        # Analyze task
        analysis = TaskAnalyzer.analyze(self.task)

        # Set plan in workspace
        self.workspace.set_plan(
            phases=analysis["suggested_phases"],
            strategy=f"Task type: {analysis['task_type']}, "
                     f"Explorers: {analysis['parallel_explorers']}"
        )

        all_results: Dict[str, SubagentResult] = {}

        for self.iteration in range(1, max_iterations + 1):
            self.workspace.start_iteration()

            # Phase 1: Research (if needed)
            if analysis["needs_research"] and self.iteration == 1:
                research_tasks = self._create_research_tasks(analysis)
                research_results = self._run_phase(
                    OrchestratorPhase.RESEARCH,
                    research_tasks,
                    parallel=self.config.parallel_research
                )
                all_results.update(research_results)

                # Synthesize research
                self._set_phase(OrchestratorPhase.SYNTHESIS)
                synthesis = self._synthesize_results(research_results)
                self.workspace.save_artifact("research_synthesis.txt", synthesis)

            # Phase 2: Implementation
            impl_task = self._create_implementation_task(analysis)
            impl_results = self._run_phase(
                OrchestratorPhase.IMPLEMENTATION,
                [impl_task],
                parallel=False
            )
            all_results.update(impl_results)

            # Check main promise
            promise_result = verify_promise_detailed(self.promise, self.working_dir)

            if promise_result.success:
                # Optional: Run verification phase
                if "verify" in analysis["suggested_phases"]:
                    self._set_phase(OrchestratorPhase.VERIFICATION)
                    # Quick review for quality (non-blocking)
                    verify_task = self._create_verification_task()
                    self._run_phase(
                        OrchestratorPhase.VERIFICATION,
                        [verify_task],
                        parallel=False
                    )

                return True

            # Plan retry
            if self.iteration < max_iterations:
                retry_tasks = self._plan_retry(impl_results, promise_result)
                retry_results = self._run_phase(
                    OrchestratorPhase.IMPLEMENTATION,
                    retry_tasks,
                    parallel=False
                )
                all_results.update(retry_results)

                # Check again
                if verify_promise(self.promise, self.working_dir):
                    return True

        # Max iterations reached
        self.workspace.state.last_error = "Max orchestration iterations reached"
        self.workspace.save()
        return False

    def get_status(self) -> Dict:
        """Get current orchestration status."""
        return {
            "loop_id": self.loop_id,
            "task": self.task,
            "iteration": self.iteration,
            "phase": self.current_phase.value if self.current_phase else None,
            "subagents": {
                k: {
                    "type": v.type,
                    "status": v.status.value,
                    "objective": v.objective[:50]
                }
                for k, v in self.workspace.state.subagents.items()
            },
            "findings_count": len(self.workspace.state.findings),
            "total_subagent_iterations": self.workspace.state.total_subagent_iterations
        }


def run_orchestrated_loop(
    task: str,
    promise: str,
    working_dir: Path,
    max_iterations: int = 3,
    max_subagents: int = 4,
    verbose: bool = False
) -> Tuple[bool, str]:
    """
    Convenience function to run an orchestrated loop.

    Args:
        task: Task description
        promise: Promise command
        working_dir: Working directory
        max_iterations: Max orchestration iterations
        max_subagents: Max concurrent subagents
        verbose: Print progress

    Returns:
        Tuple of (success, loop_id)
    """
    config = OrchestratorConfig(
        max_subagents=max_subagents,
        verbose=verbose
    )

    def on_phase(phase: OrchestratorPhase):
        if verbose:
            print(f"[ORCHESTRATOR] Phase: {phase.value}")

    def on_subagent(subagent_id: str, status: SubagentStatus):
        if verbose:
            print(f"[SUBAGENT] {subagent_id}: {status.value}")

    orchestrator = Orchestrator(
        task=task,
        promise=promise,
        working_dir=working_dir,
        config=config,
        on_phase=on_phase,
        on_subagent=on_subagent
    )

    success = orchestrator.run(max_iterations)
    return success, orchestrator.loop_id
