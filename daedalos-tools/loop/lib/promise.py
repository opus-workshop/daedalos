"""
Promise verification for loops.

A promise is a verifiable condition that must be true for the loop to complete.
Promises are shell commands that return exit code 0 on success.

Examples:
  - "make test"           -> All tests pass
  - "cargo clippy"        -> No linter warnings
  - "npm run build"       -> Build succeeds
  - "./verify.sh"         -> Custom verification script
"""

import subprocess
import shlex
from pathlib import Path
from typing import Tuple
from dataclasses import dataclass


@dataclass
class PromiseResult:
    """Result of a promise verification."""
    success: bool
    exit_code: int
    stdout: str
    stderr: str
    timed_out: bool
    duration_ms: int


def verify_promise(command: str, working_dir: Path, timeout: int = 120) -> bool:
    """
    Run the promise command and return True if it succeeds (exit code 0).

    Args:
        command: Shell command that defines "done"
        working_dir: Directory to run the command in
        timeout: Maximum seconds to wait (default: 120)

    Returns:
        True if command exits with code 0, False otherwise
    """
    try:
        result = subprocess.run(
            command,
            shell=True,
            cwd=working_dir,
            capture_output=True,
            text=True,
            timeout=timeout
        )
        return result.returncode == 0
    except subprocess.TimeoutExpired:
        return False
    except Exception:
        return False


def verify_promise_with_output(
    command: str,
    working_dir: Path,
    timeout: int = 120
) -> Tuple[bool, str, str]:
    """
    Run promise command and return (success, stdout, stderr).

    Useful for debugging why a promise failed.

    Args:
        command: Shell command to run
        working_dir: Directory to run the command in
        timeout: Maximum seconds to wait

    Returns:
        Tuple of (success: bool, stdout: str, stderr: str)
    """
    try:
        result = subprocess.run(
            command,
            shell=True,
            cwd=working_dir,
            capture_output=True,
            text=True,
            timeout=timeout
        )
        return (result.returncode == 0, result.stdout, result.stderr)
    except subprocess.TimeoutExpired:
        return (False, "", f"Promise command timed out after {timeout} seconds")
    except Exception as e:
        return (False, "", str(e))


def verify_promise_detailed(
    command: str,
    working_dir: Path,
    timeout: int = 120
) -> PromiseResult:
    """
    Run promise command and return detailed result.

    Args:
        command: Shell command to run
        working_dir: Directory to run the command in
        timeout: Maximum seconds to wait

    Returns:
        PromiseResult with full details
    """
    import time
    start_time = time.monotonic()

    try:
        result = subprocess.run(
            command,
            shell=True,
            cwd=working_dir,
            capture_output=True,
            text=True,
            timeout=timeout
        )
        duration_ms = int((time.monotonic() - start_time) * 1000)

        return PromiseResult(
            success=result.returncode == 0,
            exit_code=result.returncode,
            stdout=result.stdout,
            stderr=result.stderr,
            timed_out=False,
            duration_ms=duration_ms
        )
    except subprocess.TimeoutExpired as e:
        duration_ms = int((time.monotonic() - start_time) * 1000)
        return PromiseResult(
            success=False,
            exit_code=-1,
            stdout=e.stdout.decode() if e.stdout else "",
            stderr=e.stderr.decode() if e.stderr else "",
            timed_out=True,
            duration_ms=duration_ms
        )
    except Exception as e:
        duration_ms = int((time.monotonic() - start_time) * 1000)
        return PromiseResult(
            success=False,
            exit_code=-1,
            stdout="",
            stderr=str(e),
            timed_out=False,
            duration_ms=duration_ms
        )


def parse_promise_command(promise: str) -> dict:
    """
    Parse a promise command and return metadata about it.

    Identifies common promise patterns for better UX:
    - Test commands (npm test, pytest, cargo test, etc.)
    - Build commands (make, npm run build, cargo build, etc.)
    - Lint commands (eslint, clippy, ruff, etc.)
    - Custom commands

    Args:
        promise: The promise command string

    Returns:
        Dict with type, description, and parsed command
    """
    promise_lower = promise.lower()

    # Test commands
    test_patterns = ["test", "pytest", "jest", "mocha", "cargo test", "go test"]
    for pattern in test_patterns:
        if pattern in promise_lower:
            return {
                "type": "test",
                "description": "Tests must pass",
                "command": promise
            }

    # Build commands
    build_patterns = ["build", "compile", "make"]
    for pattern in build_patterns:
        if pattern in promise_lower:
            return {
                "type": "build",
                "description": "Build must succeed",
                "command": promise
            }

    # Lint commands
    lint_patterns = ["lint", "clippy", "eslint", "ruff", "pylint", "flake8"]
    for pattern in lint_patterns:
        if pattern in promise_lower:
            return {
                "type": "lint",
                "description": "Linting must pass",
                "command": promise
            }

    # Type check commands
    typecheck_patterns = ["tsc", "mypy", "pyright", "typecheck"]
    for pattern in typecheck_patterns:
        if pattern in promise_lower:
            return {
                "type": "typecheck",
                "description": "Type checking must pass",
                "command": promise
            }

    # Default: custom command
    return {
        "type": "custom",
        "description": "Command must exit with code 0",
        "command": promise
    }
