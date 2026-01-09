"""
CePO-style best-of-N parallel exploration.

This module implements the CePO methodology:
1. Plan: Create N divergent branches from current state
2. Execute: Run loops in parallel on each branch
3. Analyze: Score each result
4. Select: Pick the best and apply to main state

This allows exploring multiple solution paths simultaneously
and selecting the best outcome.
"""

import subprocess
import shutil
import tempfile
from pathlib import Path
from dataclasses import dataclass, field
from typing import Optional, Callable, List
from concurrent.futures import ThreadPoolExecutor, as_completed, Future
import threading

from .state import Loop, LoopState, LoopStatus
from .checkpoint import get_backend
from .agent import get_agent, AgentAdapter


@dataclass
class BranchResult:
    """Result of a single branch execution."""
    branch_id: int
    success: bool
    iterations_used: int
    final_state: LoopState
    working_dir: Path
    score: float = 0.0
    metrics: dict = field(default_factory=dict)


@dataclass
class BestOfNResult:
    """Result of a best-of-N execution."""
    winner: Optional[BranchResult]
    all_results: List[BranchResult]
    total_iterations: int
    selection_reason: str


def score_result(result: BranchResult, original_dir: Path) -> float:
    """
    Score a branch result for selection.
    Higher scores are better.

    Scoring factors:
    - Promise satisfied: +100
    - Fewer iterations used: +10 per iteration saved
    - Smaller diff (fewer changes): +0.1 per line not changed
    - Test coverage maintained/improved: +coverage%
    """
    score = 0.0
    metrics = {}

    # Promise satisfaction is the primary factor
    if result.success:
        score += 100
        metrics["promise_met"] = True
    else:
        metrics["promise_met"] = False

    # Fewer iterations is better (more efficient solution)
    max_iter = result.final_state.max_iterations
    saved_iterations = max_iter - result.iterations_used
    iteration_bonus = saved_iterations * 10
    score += iteration_bonus
    metrics["iterations_saved"] = saved_iterations
    metrics["iteration_bonus"] = iteration_bonus

    # Measure diff size (fewer changes = cleaner solution)
    try:
        diff_result = subprocess.run(
            ["git", "diff", "--stat", "--cached"],
            cwd=result.working_dir,
            capture_output=True,
            text=True,
            timeout=10
        )
        lines = diff_result.stdout.strip().split('\n')
        lines_changed = len([l for l in lines if l.strip()])
        # Negative score for more changes (cleaner is better)
        change_penalty = lines_changed * 0.5
        score -= change_penalty
        metrics["lines_changed"] = lines_changed
        metrics["change_penalty"] = change_penalty
    except (subprocess.TimeoutExpired, FileNotFoundError):
        metrics["lines_changed"] = "unknown"

    # Try to measure test coverage if available
    try:
        # Check for coverage report
        coverage_file = result.working_dir / "coverage" / "coverage-summary.json"
        if coverage_file.exists():
            import json
            with open(coverage_file) as f:
                coverage = json.load(f)
                if "total" in coverage and "lines" in coverage["total"]:
                    pct = coverage["total"]["lines"].get("pct", 0)
                    score += pct * 0.5
                    metrics["coverage_pct"] = pct
    except:
        pass

    result.score = score
    result.metrics = metrics
    return score


def run_best_of_n(
    prompt: str,
    promise_cmd: str,
    working_dir: Path,
    n: int = 3,
    agent_name: str = "opencode",
    agent_cmd: Optional[str] = None,
    max_iterations: int = 10,
    timeout: int = 300,
    on_branch_start: Optional[Callable[[int], None]] = None,
    on_branch_complete: Optional[Callable[[BranchResult], None]] = None,
    selection_mode: str = "auto"
) -> BestOfNResult:
    """
    Run N parallel loop attempts and return the best result.

    Args:
        prompt: Task description
        promise_cmd: Success condition command
        working_dir: Base working directory
        n: Number of parallel branches
        agent_name: Agent to use ("opencode", "claude", etc.)
        agent_cmd: Custom agent command (if agent_name="custom")
        max_iterations: Max iterations per branch
        timeout: Per-iteration timeout
        on_branch_start: Callback when branch starts
        on_branch_complete: Callback when branch completes
        selection_mode: "auto", "manual", or custom metric name

    Returns:
        BestOfNResult with winner and all results
    """
    results: List[BranchResult] = []
    branches: List[tuple[int, Path]] = []

    # Create temporary directories for each branch
    base_temp = Path(tempfile.mkdtemp(prefix="loop_bestofn_"))

    for i in range(n):
        branch_dir = base_temp / f"branch_{i}"
        branch_dir.mkdir(parents=True)

        # Copy working directory to branch
        shutil.copytree(
            working_dir,
            branch_dir / "work",
            dirs_exist_ok=True,
            ignore=shutil.ignore_patterns('.git', '__pycache__', 'node_modules', '.venv')
        )

        # Initialize git in branch if not present
        work_dir = branch_dir / "work"
        if not (work_dir / ".git").exists():
            subprocess.run(
                ["git", "init"],
                cwd=work_dir,
                capture_output=True
            )
            subprocess.run(
                ["git", "add", "-A"],
                cwd=work_dir,
                capture_output=True
            )
            subprocess.run(
                ["git", "commit", "-m", "Initial state for best-of-N branch"],
                cwd=work_dir,
                capture_output=True
            )

        branches.append((i, work_dir))

    def run_branch(branch_id: int, branch_dir: Path) -> BranchResult:
        """Execute a single branch."""
        if on_branch_start:
            on_branch_start(branch_id)

        # Get agent and checkpoint for this branch
        agent = get_agent(agent_name, agent_cmd)
        checkpoint = get_backend(branch_dir)

        # Create and run loop with branch-specific prompt
        branch_prompt = f"[Branch {branch_id + 1}/{n}] {prompt}"

        loop = Loop(
            prompt=branch_prompt,
            promise_cmd=promise_cmd,
            working_dir=branch_dir,
            agent=agent,
            checkpoint=checkpoint,
            max_iterations=max_iterations,
            timeout=timeout
        )

        success = loop.run()

        return BranchResult(
            branch_id=branch_id,
            success=success,
            iterations_used=loop.state.current_iteration,
            final_state=loop.state,
            working_dir=branch_dir,
            score=0.0
        )

    # Run branches in parallel
    with ThreadPoolExecutor(max_workers=n) as executor:
        futures: dict[Future, int] = {
            executor.submit(run_branch, branch_id, branch_dir): branch_id
            for branch_id, branch_dir in branches
        }

        for future in as_completed(futures):
            try:
                result = future.result()
                # Score the result
                score_result(result, working_dir)
                results.append(result)

                if on_branch_complete:
                    on_branch_complete(result)
            except Exception as e:
                branch_id = futures[future]
                # Create failed result
                results.append(BranchResult(
                    branch_id=branch_id,
                    success=False,
                    iterations_used=0,
                    final_state=None,
                    working_dir=branches[branch_id][1],
                    score=-1000,
                    metrics={"error": str(e)}
                ))

    if not results:
        return BestOfNResult(
            winner=None,
            all_results=[],
            total_iterations=0,
            selection_reason="No branches completed"
        )

    # Sort by score (highest first)
    results.sort(key=lambda r: r.score, reverse=True)

    # Select winner
    winner = results[0]
    selection_reason = f"Highest score: {winner.score:.1f}"

    if selection_mode == "manual":
        # In manual mode, just return results without auto-selecting
        selection_reason = "Manual selection required"
    else:
        # Apply winning branch to original working directory
        if winner.success and winner.working_dir.exists():
            # Copy winning state back (excluding .git to preserve original history)
            for item in winner.working_dir.iterdir():
                if item.name == '.git':
                    continue
                dest = working_dir / item.name
                if item.is_dir():
                    if dest.exists():
                        shutil.rmtree(dest)
                    shutil.copytree(item, dest)
                else:
                    shutil.copy2(item, dest)

            selection_reason = (
                f"Branch {winner.branch_id + 1} selected: "
                f"score={winner.score:.1f}, "
                f"iterations={winner.iterations_used}"
            )

    # Calculate total iterations
    total_iterations = sum(r.iterations_used for r in results)

    # Cleanup branch directories (keep winner for inspection if needed)
    for branch_id, branch_dir in branches:
        if branch_id != winner.branch_id:
            try:
                shutil.rmtree(branch_dir.parent)
            except:
                pass

    return BestOfNResult(
        winner=winner,
        all_results=results,
        total_iterations=total_iterations,
        selection_reason=selection_reason
    )


def compare_branches(results: List[BranchResult]) -> str:
    """
    Generate a comparison report of branch results.

    Args:
        results: List of branch results to compare

    Returns:
        Formatted comparison string
    """
    lines = []
    lines.append("=" * 60)
    lines.append("BEST-OF-N BRANCH COMPARISON")
    lines.append("=" * 60)

    for i, result in enumerate(sorted(results, key=lambda r: r.score, reverse=True)):
        rank = i + 1
        status = "PASS" if result.success else "FAIL"

        lines.append(f"\n#{rank} Branch {result.branch_id + 1}")
        lines.append(f"  Status: {status}")
        lines.append(f"  Score: {result.score:.1f}")
        lines.append(f"  Iterations: {result.iterations_used}")

        if result.metrics:
            lines.append("  Metrics:")
            for key, value in result.metrics.items():
                lines.append(f"    {key}: {value}")

    lines.append("\n" + "=" * 60)

    return "\n".join(lines)
