"""
Agent adapters for different AI coding tools.

Loop is agent-agnostic. This module provides adapters for:
- OpenCode (FOSS default)
- Claude Code CLI
- Aider
- Custom agents via command

Priority order for auto-detection prefers FOSS options.
"""

from abc import ABC, abstractmethod
from pathlib import Path
import subprocess
import tempfile
import os
import signal
from typing import Optional
from dataclasses import dataclass


@dataclass
class AgentResult:
    """Result of an agent execution."""
    success: bool
    output: str
    error: str
    exit_code: int
    timed_out: bool


class AgentAdapter(ABC):
    """Abstract base class for agent adapters."""

    @property
    @abstractmethod
    def name(self) -> str:
        """Return the agent name."""
        pass

    @abstractmethod
    def run(
        self,
        prompt: str,
        working_dir: Path,
        context: Optional[str] = None,
        timeout: int = 300
    ) -> AgentResult:
        """
        Run the agent with the given prompt.

        Args:
            prompt: Task description for the agent
            working_dir: Directory for the agent to work in
            context: Additional context to prepend to prompt
            timeout: Maximum seconds to wait (default: 5 minutes)

        Returns:
            AgentResult with execution details
        """
        pass

    @abstractmethod
    def is_available(self) -> bool:
        """Check if this agent is available on the system."""
        pass

    def _build_full_prompt(self, prompt: str, context: Optional[str] = None) -> str:
        """Build the full prompt with optional context."""
        if context:
            return f"{context}\n\n---\n\n{prompt}"
        return prompt


class OpenCodeAgent(AgentAdapter):
    """
    Adapter for OpenCode - the FOSS default agent.

    OpenCode is the recommended agent for Daedalos as it's fully
    open source and works with local models via Ollama.
    """

    @property
    def name(self) -> str:
        return "opencode"

    def is_available(self) -> bool:
        try:
            result = subprocess.run(
                ["opencode", "--version"],
                capture_output=True,
                timeout=5
            )
            return result.returncode == 0
        except (FileNotFoundError, subprocess.TimeoutExpired):
            return False

    def run(
        self,
        prompt: str,
        working_dir: Path,
        context: Optional[str] = None,
        timeout: int = 300
    ) -> AgentResult:
        full_prompt = self._build_full_prompt(prompt, context)

        # Write prompt to temp file for opencode
        with tempfile.NamedTemporaryFile(
            mode='w',
            suffix='.txt',
            delete=False
        ) as f:
            f.write(full_prompt)
            prompt_file = f.name

        try:
            result = subprocess.run(
                ["opencode", "--prompt-file", prompt_file, "--non-interactive"],
                cwd=working_dir,
                capture_output=True,
                text=True,
                timeout=timeout
            )

            return AgentResult(
                success=result.returncode == 0,
                output=result.stdout,
                error=result.stderr,
                exit_code=result.returncode,
                timed_out=False
            )
        except subprocess.TimeoutExpired:
            return AgentResult(
                success=False,
                output="",
                error=f"Agent timed out after {timeout} seconds",
                exit_code=-1,
                timed_out=True
            )
        finally:
            # Clean up temp file
            try:
                os.unlink(prompt_file)
            except OSError:
                pass


class ClaudeAgent(AgentAdapter):
    """
    Adapter for Claude Code CLI.

    Claude Code is Anthropic's official CLI. It requires an API key
    but provides excellent code generation capabilities.
    """

    @property
    def name(self) -> str:
        return "claude"

    def is_available(self) -> bool:
        try:
            result = subprocess.run(
                ["claude", "--version"],
                capture_output=True,
                timeout=5
            )
            return result.returncode == 0
        except (FileNotFoundError, subprocess.TimeoutExpired):
            return False

    def run(
        self,
        prompt: str,
        working_dir: Path,
        context: Optional[str] = None,
        timeout: int = 300
    ) -> AgentResult:
        full_prompt = self._build_full_prompt(prompt, context)

        try:
            # Claude Code uses --print for non-interactive output mode
            # --permission-mode acceptEdits allows file modifications without prompts
            # Prompt is passed via stdin to avoid shell escaping and length issues
            result = subprocess.run(
                [
                    "claude",
                    "--print",
                    "--permission-mode", "acceptEdits",
                ],
                cwd=working_dir,
                input=full_prompt,
                capture_output=True,
                text=True,
                timeout=timeout
            )

            return AgentResult(
                success=result.returncode == 0,
                output=result.stdout,
                error=result.stderr,
                exit_code=result.returncode,
                timed_out=False
            )
        except subprocess.TimeoutExpired:
            return AgentResult(
                success=False,
                output="",
                error=f"Agent timed out after {timeout} seconds",
                exit_code=-1,
                timed_out=True
            )


class AiderAgent(AgentAdapter):
    """
    Adapter for Aider.

    Aider is a popular open-source AI coding assistant that
    works with various models.
    """

    @property
    def name(self) -> str:
        return "aider"

    def is_available(self) -> bool:
        try:
            result = subprocess.run(
                ["aider", "--version"],
                capture_output=True,
                timeout=5
            )
            return result.returncode == 0
        except (FileNotFoundError, subprocess.TimeoutExpired):
            return False

    def run(
        self,
        prompt: str,
        working_dir: Path,
        context: Optional[str] = None,
        timeout: int = 300
    ) -> AgentResult:
        full_prompt = self._build_full_prompt(prompt, context)

        try:
            # Aider uses --message for prompts and --yes for auto-confirm
            result = subprocess.run(
                ["aider", "--message", full_prompt, "--yes", "--no-auto-commits"],
                cwd=working_dir,
                capture_output=True,
                text=True,
                timeout=timeout
            )

            return AgentResult(
                success=result.returncode == 0,
                output=result.stdout,
                error=result.stderr,
                exit_code=result.returncode,
                timed_out=False
            )
        except subprocess.TimeoutExpired:
            return AgentResult(
                success=False,
                output="",
                error=f"Agent timed out after {timeout} seconds",
                exit_code=-1,
                timed_out=True
            )


class CursorAgent(AgentAdapter):
    """
    Adapter for Cursor's CLI mode.

    Cursor is a VS Code fork with AI capabilities.
    This adapter uses its CLI interface if available.
    """

    @property
    def name(self) -> str:
        return "cursor"

    def is_available(self) -> bool:
        try:
            result = subprocess.run(
                ["cursor", "--version"],
                capture_output=True,
                timeout=5
            )
            return result.returncode == 0
        except (FileNotFoundError, subprocess.TimeoutExpired):
            return False

    def run(
        self,
        prompt: str,
        working_dir: Path,
        context: Optional[str] = None,
        timeout: int = 300
    ) -> AgentResult:
        # Cursor CLI support is limited; this is a placeholder
        # for when/if they add proper CLI support
        return AgentResult(
            success=False,
            output="",
            error="Cursor CLI agent support not yet implemented",
            exit_code=1,
            timed_out=False
        )


class CustomAgent(AgentAdapter):
    """
    Adapter for custom agent commands.

    Allows using any command-line tool as an agent.
    The prompt is passed via stdin.
    """

    def __init__(self, command: str):
        self.command = command

    @property
    def name(self) -> str:
        return "custom"

    def is_available(self) -> bool:
        # Assume custom commands are available
        return True

    def run(
        self,
        prompt: str,
        working_dir: Path,
        context: Optional[str] = None,
        timeout: int = 300
    ) -> AgentResult:
        full_prompt = self._build_full_prompt(prompt, context)

        try:
            # Custom command receives prompt via stdin
            result = subprocess.run(
                self.command,
                shell=True,
                cwd=working_dir,
                input=full_prompt,
                capture_output=True,
                text=True,
                timeout=timeout
            )

            return AgentResult(
                success=result.returncode == 0,
                output=result.stdout,
                error=result.stderr,
                exit_code=result.returncode,
                timed_out=False
            )
        except subprocess.TimeoutExpired:
            return AgentResult(
                success=False,
                output="",
                error=f"Agent timed out after {timeout} seconds",
                exit_code=-1,
                timed_out=True
            )


# Agent registry for factory function
_AGENTS = {
    "opencode": OpenCodeAgent,
    "claude": ClaudeAgent,
    "aider": AiderAgent,
    "cursor": CursorAgent,
}


def get_agent(name: str, custom_cmd: Optional[str] = None) -> AgentAdapter:
    """
    Factory function to get appropriate agent adapter.

    Args:
        name: Agent name ("opencode", "claude", "aider", "cursor", "custom")
        custom_cmd: Command string for custom agent (required if name="custom")

    Returns:
        AgentAdapter instance

    Raises:
        ValueError: If agent name is unknown or custom_cmd missing for custom agent
    """
    if name == "custom":
        if not custom_cmd:
            raise ValueError("Custom agent requires --agent-cmd")
        return CustomAgent(custom_cmd)

    if name not in _AGENTS:
        raise ValueError(f"Unknown agent: {name}. Available: {', '.join(_AGENTS.keys())}")

    return _AGENTS[name]()


def detect_agent() -> Optional[AgentAdapter]:
    """
    Auto-detect available agent, preferring FOSS options.

    Priority order:
    1. OpenCode (FOSS, works with local models)
    2. Aider (FOSS, popular)
    3. Claude (proprietary but excellent)
    4. Cursor (proprietary)

    Returns:
        First available AgentAdapter, or None if none available
    """
    priority = [OpenCodeAgent, AiderAgent, ClaudeAgent, CursorAgent]

    for agent_class in priority:
        agent = agent_class()
        if agent.is_available():
            return agent

    return None


def list_available_agents() -> list[str]:
    """
    List all agents that are available on the system.

    Returns:
        List of available agent names
    """
    available = []
    for name, agent_class in _AGENTS.items():
        agent = agent_class()
        if agent.is_available():
            available.append(name)
    return available
