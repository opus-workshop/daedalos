"""
Shared workspace for multi-agent orchestration.

The workspace provides:
1. Shared state between orchestrator and subagents
2. Findings storage and retrieval
3. Handoff message passing
4. Artifact management

Workspace structure:
    ~/.local/share/daedalos/loops/<loop-id>/
    ├── workspace.json      # Main state file
    ├── findings/           # Subagent outputs
    │   ├── explorer-1.txt
    │   └── explorer-2.txt
    ├── handoffs/           # Structured handoff messages
    │   └── orchestrator-to-implementer.json
    └── artifacts/          # Files produced by subagents
"""

import json
import os
from pathlib import Path
from datetime import datetime
from dataclasses import dataclass, asdict, field
from typing import Dict, List, Optional, Any
from enum import Enum
import shutil


class SubagentStatus(Enum):
    """Status of a subagent."""
    PENDING = "pending"
    RUNNING = "running"
    COMPLETED = "completed"
    FAILED = "failed"
    CANCELLED = "cancelled"


@dataclass
class Finding:
    """A finding from a subagent."""
    id: str
    from_agent: str
    type: str  # research, implementation, review, debug
    content: str
    files: List[str]
    timestamp: str

    def to_dict(self) -> dict:
        return asdict(self)

    @classmethod
    def from_dict(cls, d: dict) -> "Finding":
        return cls(**d)


@dataclass
class Handoff:
    """A handoff message between agents."""
    id: str
    from_agent: str
    to_agent: str
    message: str
    context_files: List[str]
    timestamp: str
    acknowledged: bool = False

    def to_dict(self) -> dict:
        return asdict(self)

    @classmethod
    def from_dict(cls, d: dict) -> "Handoff":
        return cls(**d)


@dataclass
class SubagentRecord:
    """Record of a subagent's execution."""
    id: str
    type: str  # explorer, implementer, reviewer, debugger, tester
    objective: str
    status: SubagentStatus
    loop_id: Optional[str]  # ID of the subagent's loop
    started_at: Optional[str]
    finished_at: Optional[str]
    promise_result: Optional[bool]
    output_summary: str
    error: Optional[str]

    def to_dict(self) -> dict:
        d = asdict(self)
        d["status"] = self.status.value
        return d

    @classmethod
    def from_dict(cls, d: dict) -> "SubagentRecord":
        d["status"] = SubagentStatus(d["status"])
        return cls(**d)


@dataclass
class OrchestratorPlan:
    """The orchestrator's plan for the current iteration."""
    phases: List[str]  # e.g., ["research", "implement", "verify"]
    current_phase: str
    current_phase_index: int
    strategy: str

    def to_dict(self) -> dict:
        return asdict(self)

    @classmethod
    def from_dict(cls, d: dict) -> "OrchestratorPlan":
        return cls(**d)


@dataclass
class WorkspaceState:
    """
    Complete workspace state.

    Persisted to workspace.json and used for coordination
    between orchestrator and subagents.
    """
    loop_id: str
    created_at: str
    updated_at: str
    iteration: int
    main_task: str
    main_promise: str

    # Orchestrator state
    plan: Optional[OrchestratorPlan]

    # Subagent tracking
    subagents: Dict[str, SubagentRecord]

    # Shared data
    findings: List[Finding]
    handoffs: List[Handoff]

    # Metadata
    total_subagent_iterations: int
    last_error: Optional[str]

    def to_dict(self) -> dict:
        d = {
            "loop_id": self.loop_id,
            "created_at": self.created_at,
            "updated_at": self.updated_at,
            "iteration": self.iteration,
            "main_task": self.main_task,
            "main_promise": self.main_promise,
            "plan": self.plan.to_dict() if self.plan else None,
            "subagents": {k: v.to_dict() for k, v in self.subagents.items()},
            "findings": [f.to_dict() for f in self.findings],
            "handoffs": [h.to_dict() for h in self.handoffs],
            "total_subagent_iterations": self.total_subagent_iterations,
            "last_error": self.last_error
        }
        return d

    @classmethod
    def from_dict(cls, d: dict) -> "WorkspaceState":
        return cls(
            loop_id=d["loop_id"],
            created_at=d["created_at"],
            updated_at=d["updated_at"],
            iteration=d["iteration"],
            main_task=d["main_task"],
            main_promise=d["main_promise"],
            plan=OrchestratorPlan.from_dict(d["plan"]) if d.get("plan") else None,
            subagents={k: SubagentRecord.from_dict(v) for k, v in d.get("subagents", {}).items()},
            findings=[Finding.from_dict(f) for f in d.get("findings", [])],
            handoffs=[Handoff.from_dict(h) for h in d.get("handoffs", [])],
            total_subagent_iterations=d.get("total_subagent_iterations", 0),
            last_error=d.get("last_error")
        )


class Workspace:
    """
    Manages shared workspace for orchestrated loops.

    Usage:
        workspace = Workspace.create(loop_id, task, promise, working_dir)
        workspace.add_finding(Finding(...))
        workspace.add_handoff(Handoff(...))
        workspace.save()

        # Later, from subagent:
        workspace = Workspace.load(loop_id, working_dir)
        findings = workspace.get_findings_for(subagent_id)
    """

    def __init__(self, path: Path, state: WorkspaceState):
        """Initialize workspace with path and state."""
        self.path = path
        self.state = state

        # Ensure directories exist
        (self.path / "findings").mkdir(parents=True, exist_ok=True)
        (self.path / "handoffs").mkdir(parents=True, exist_ok=True)
        (self.path / "artifacts").mkdir(parents=True, exist_ok=True)

    @classmethod
    def create(
        cls,
        loop_id: str,
        task: str,
        promise: str,
        base_dir: Optional[Path] = None
    ) -> "Workspace":
        """
        Create a new workspace for an orchestrated loop.

        Args:
            loop_id: Unique loop identifier
            task: Main task description
            promise: Main promise command
            base_dir: Base directory for workspaces
        """
        base_dir = base_dir or Path.home() / ".local/share/daedalos/loops"
        path = base_dir / loop_id
        path.mkdir(parents=True, exist_ok=True)

        state = WorkspaceState(
            loop_id=loop_id,
            created_at=datetime.now().isoformat(),
            updated_at=datetime.now().isoformat(),
            iteration=0,
            main_task=task,
            main_promise=promise,
            plan=None,
            subagents={},
            findings=[],
            handoffs=[],
            total_subagent_iterations=0,
            last_error=None
        )

        workspace = cls(path, state)
        workspace.save()
        return workspace

    @classmethod
    def load(cls, loop_id: str, base_dir: Optional[Path] = None) -> "Workspace":
        """
        Load an existing workspace.

        Args:
            loop_id: Loop identifier
            base_dir: Base directory for workspaces

        Returns:
            Workspace instance

        Raises:
            FileNotFoundError: If workspace doesn't exist
        """
        base_dir = base_dir or Path.home() / ".local/share/daedalos/loops"
        path = base_dir / loop_id
        state_file = path / "workspace.json"

        if not state_file.exists():
            raise FileNotFoundError(f"Workspace not found: {loop_id}")

        with open(state_file) as f:
            state = WorkspaceState.from_dict(json.load(f))

        return cls(path, state)

    @classmethod
    def exists(cls, loop_id: str, base_dir: Optional[Path] = None) -> bool:
        """Check if a workspace exists."""
        base_dir = base_dir or Path.home() / ".local/share/daedalos/loops"
        return (base_dir / loop_id / "workspace.json").exists()

    def save(self):
        """Save workspace state to disk."""
        self.state.updated_at = datetime.now().isoformat()
        state_file = self.path / "workspace.json"
        with open(state_file, "w") as f:
            json.dump(self.state.to_dict(), f, indent=2)

    def destroy(self):
        """Remove the workspace directory."""
        if self.path.exists():
            shutil.rmtree(self.path)

    # Plan management

    def set_plan(self, phases: List[str], strategy: str):
        """Set the orchestrator's plan."""
        self.state.plan = OrchestratorPlan(
            phases=phases,
            current_phase=phases[0] if phases else "",
            current_phase_index=0,
            strategy=strategy
        )
        self.save()

    def advance_phase(self) -> bool:
        """
        Advance to the next phase.

        Returns:
            True if advanced, False if no more phases
        """
        if not self.state.plan:
            return False

        plan = self.state.plan
        if plan.current_phase_index < len(plan.phases) - 1:
            plan.current_phase_index += 1
            plan.current_phase = plan.phases[plan.current_phase_index]
            self.save()
            return True
        return False

    # Subagent management

    def register_subagent(
        self,
        subagent_id: str,
        subagent_type: str,
        objective: str
    ) -> SubagentRecord:
        """Register a new subagent."""
        record = SubagentRecord(
            id=subagent_id,
            type=subagent_type,
            objective=objective,
            status=SubagentStatus.PENDING,
            loop_id=None,
            started_at=None,
            finished_at=None,
            promise_result=None,
            output_summary="",
            error=None
        )
        self.state.subagents[subagent_id] = record
        self.save()
        return record

    def update_subagent(
        self,
        subagent_id: str,
        status: Optional[SubagentStatus] = None,
        loop_id: Optional[str] = None,
        promise_result: Optional[bool] = None,
        output_summary: Optional[str] = None,
        error: Optional[str] = None
    ):
        """Update a subagent's record."""
        if subagent_id not in self.state.subagents:
            return

        record = self.state.subagents[subagent_id]

        if status is not None:
            record.status = status
            if status == SubagentStatus.RUNNING:
                record.started_at = datetime.now().isoformat()
            elif status in (SubagentStatus.COMPLETED, SubagentStatus.FAILED):
                record.finished_at = datetime.now().isoformat()

        if loop_id is not None:
            record.loop_id = loop_id

        if promise_result is not None:
            record.promise_result = promise_result

        if output_summary is not None:
            record.output_summary = output_summary

        if error is not None:
            record.error = error

        self.save()

    def get_subagent(self, subagent_id: str) -> Optional[SubagentRecord]:
        """Get a subagent record."""
        return self.state.subagents.get(subagent_id)

    def get_active_subagents(self) -> List[SubagentRecord]:
        """Get all running subagents."""
        return [
            s for s in self.state.subagents.values()
            if s.status == SubagentStatus.RUNNING
        ]

    def get_completed_subagents(self) -> List[SubagentRecord]:
        """Get all completed subagents."""
        return [
            s for s in self.state.subagents.values()
            if s.status == SubagentStatus.COMPLETED
        ]

    # Findings management

    def add_finding(self, finding: Finding):
        """Add a finding from a subagent."""
        self.state.findings.append(finding)

        # Also write to findings directory
        findings_file = self.path / "findings" / f"{finding.from_agent}.txt"
        with open(findings_file, "a") as f:
            f.write(f"\n{'='*60}\n")
            f.write(f"Type: {finding.type}\n")
            f.write(f"Time: {finding.timestamp}\n")
            f.write(f"Files: {', '.join(finding.files)}\n")
            f.write(f"{'='*60}\n")
            f.write(finding.content)
            f.write("\n")

        self.save()

    def get_findings(self, finding_type: Optional[str] = None) -> List[Finding]:
        """Get findings, optionally filtered by type."""
        if finding_type:
            return [f for f in self.state.findings if f.type == finding_type]
        return self.state.findings

    def get_findings_from(self, agent_id: str) -> List[Finding]:
        """Get findings from a specific agent."""
        return [f for f in self.state.findings if f.from_agent == agent_id]

    def get_findings_summary(self) -> str:
        """Get a summary of all findings for context."""
        if not self.state.findings:
            return "No findings yet."

        lines = ["FINDINGS SUMMARY:", "=" * 40]
        for finding in self.state.findings:
            lines.append(f"\n[{finding.from_agent}] ({finding.type}):")
            # Truncate long content
            content = finding.content[:500]
            if len(finding.content) > 500:
                content += "..."
            lines.append(content)
            if finding.files:
                lines.append(f"  Files: {', '.join(finding.files[:5])}")

        return "\n".join(lines)

    # Handoff management

    def add_handoff(self, handoff: Handoff):
        """Add a handoff message."""
        self.state.handoffs.append(handoff)

        # Also write to handoffs directory
        handoff_file = self.path / "handoffs" / f"{handoff.id}.json"
        with open(handoff_file, "w") as f:
            json.dump(handoff.to_dict(), f, indent=2)

        self.save()

    def get_handoffs_for(self, agent_id: str) -> List[Handoff]:
        """Get handoffs targeted at a specific agent."""
        return [h for h in self.state.handoffs if h.to_agent == agent_id]

    def acknowledge_handoff(self, handoff_id: str):
        """Mark a handoff as acknowledged."""
        for handoff in self.state.handoffs:
            if handoff.id == handoff_id:
                handoff.acknowledged = True
                self.save()
                return

    # Artifact management

    def save_artifact(self, name: str, content: str) -> Path:
        """Save an artifact file."""
        artifact_path = self.path / "artifacts" / name
        artifact_path.parent.mkdir(parents=True, exist_ok=True)
        with open(artifact_path, "w") as f:
            f.write(content)
        return artifact_path

    def get_artifact(self, name: str) -> Optional[str]:
        """Get an artifact's content."""
        artifact_path = self.path / "artifacts" / name
        if artifact_path.exists():
            return artifact_path.read_text()
        return None

    def list_artifacts(self) -> List[str]:
        """List all artifacts."""
        artifacts_dir = self.path / "artifacts"
        if not artifacts_dir.exists():
            return []
        return [f.name for f in artifacts_dir.iterdir() if f.is_file()]

    # Iteration management

    def start_iteration(self):
        """Start a new orchestration iteration."""
        self.state.iteration += 1
        self.save()

    def record_subagent_iteration(self):
        """Record that a subagent completed an iteration."""
        self.state.total_subagent_iterations += 1
        self.save()

    # Context building

    def build_context_for_subagent(self, subagent_id: str) -> str:
        """
        Build context string for a subagent.

        Includes relevant findings and handoffs.
        """
        lines = []

        # Add handoffs
        handoffs = self.get_handoffs_for(subagent_id)
        if handoffs:
            lines.append("HANDOFFS TO YOU:")
            lines.append("=" * 40)
            for h in handoffs:
                if not h.acknowledged:
                    lines.append(f"From {h.from_agent}:")
                    lines.append(h.message)
                    if h.context_files:
                        lines.append(f"Reference files: {', '.join(h.context_files)}")
                    lines.append("")

        # Add relevant findings summary
        findings_summary = self.get_findings_summary()
        if findings_summary != "No findings yet.":
            lines.append("\n" + findings_summary)

        return "\n".join(lines) if lines else ""
