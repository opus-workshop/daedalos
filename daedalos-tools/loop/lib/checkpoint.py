"""
Checkpoint backends for loop state preservation.

Every iteration creates a rollback point. This module provides:
- BtrfsCheckpoint: Instant, space-efficient snapshots (recommended)
- GitCheckpoint: Git-based checkpoints for non-Btrfs systems
- NoneCheckpoint: No checkpoints (for low-risk operations)
"""

from abc import ABC, abstractmethod
from pathlib import Path
import subprocess
import json
import shutil
from datetime import datetime
from typing import Optional, List
from dataclasses import dataclass


@dataclass
class Checkpoint:
    """Represents a checkpoint in the system."""
    id: str
    name: str
    created_at: str
    path: str
    backend: str
    metadata: dict


class CheckpointBackend(ABC):
    """Abstract base class for checkpoint backends."""

    @property
    @abstractmethod
    def name(self) -> str:
        """Return the backend name."""
        pass

    @abstractmethod
    def create(self, name: str, path: Path) -> str:
        """Create a checkpoint, return checkpoint ID."""
        pass

    @abstractmethod
    def restore(self, checkpoint_id: str, path: Path) -> bool:
        """Restore to a checkpoint."""
        pass

    @abstractmethod
    def list(self, path: Path) -> List[Checkpoint]:
        """List available checkpoints."""
        pass

    @abstractmethod
    def delete(self, checkpoint_id: str) -> bool:
        """Delete a checkpoint."""
        pass

    @abstractmethod
    def exists(self, checkpoint_id: str) -> bool:
        """Check if a checkpoint exists."""
        pass


class BtrfsCheckpoint(CheckpointBackend):
    """
    Btrfs snapshot-based checkpoints.

    Advantages:
    - Instant snapshots (< 1ms)
    - Copy-on-write (minimal space usage)
    - Full filesystem state captured
    - Works with any file type
    """

    def __init__(self, snapshot_dir: Optional[Path] = None):
        self.snapshot_dir = snapshot_dir or (
            Path.home() / ".local/share/daedalos/loop/snapshots"
        )
        self.snapshot_dir.mkdir(parents=True, exist_ok=True)
        self._metadata_file = self.snapshot_dir / "metadata.json"
        self._metadata = self._load_metadata()

    def _load_metadata(self) -> dict:
        """Load checkpoint metadata from JSON file."""
        if self._metadata_file.exists():
            try:
                with open(self._metadata_file) as f:
                    return json.load(f)
            except (json.JSONDecodeError, IOError):
                return {}
        return {}

    def _save_metadata(self):
        """Save checkpoint metadata to JSON file."""
        with open(self._metadata_file, "w") as f:
            json.dump(self._metadata, f, indent=2)

    @property
    def name(self) -> str:
        return "btrfs"

    def create(self, name: str, path: Path) -> str:
        timestamp = datetime.now().strftime("%Y%m%d_%H%M%S")
        checkpoint_id = f"{name}_{timestamp}"
        snapshot_path = self.snapshot_dir / checkpoint_id

        try:
            result = subprocess.run(
                ["btrfs", "subvolume", "snapshot", "-r", str(path), str(snapshot_path)],
                check=True,
                capture_output=True,
                text=True
            )

            # Store metadata
            self._metadata[checkpoint_id] = {
                "name": name,
                "created_at": datetime.now().isoformat(),
                "source_path": str(path),
                "snapshot_path": str(snapshot_path)
            }
            self._save_metadata()

            return checkpoint_id

        except subprocess.CalledProcessError as e:
            raise RuntimeError(f"Failed to create Btrfs snapshot: {e.stderr}")

    def restore(self, checkpoint_id: str, path: Path) -> bool:
        snapshot_path = self.snapshot_dir / checkpoint_id
        if not snapshot_path.exists():
            return False

        try:
            # For Btrfs restore, we need to:
            # 1. Delete current subvolume (if it is one)
            # 2. Create new snapshot from checkpoint

            # Check if path is a subvolume
            check_result = subprocess.run(
                ["btrfs", "subvolume", "show", str(path)],
                capture_output=True
            )

            if check_result.returncode == 0:
                # It's a subvolume, delete it
                subprocess.run(
                    ["btrfs", "subvolume", "delete", str(path)],
                    check=True,
                    capture_output=True
                )

            # Create writable snapshot from the read-only checkpoint
            subprocess.run(
                ["btrfs", "subvolume", "snapshot", str(snapshot_path), str(path)],
                check=True,
                capture_output=True
            )

            return True

        except subprocess.CalledProcessError:
            return False

    def list(self, path: Path) -> List[Checkpoint]:
        checkpoints = []
        for checkpoint_id, meta in self._metadata.items():
            snapshot_path = self.snapshot_dir / checkpoint_id
            if snapshot_path.exists():
                checkpoints.append(Checkpoint(
                    id=checkpoint_id,
                    name=meta.get("name", checkpoint_id),
                    created_at=meta.get("created_at", ""),
                    path=str(snapshot_path),
                    backend="btrfs",
                    metadata=meta
                ))
        return sorted(checkpoints, key=lambda c: c.created_at, reverse=True)

    def delete(self, checkpoint_id: str) -> bool:
        snapshot_path = self.snapshot_dir / checkpoint_id
        if snapshot_path.exists():
            try:
                subprocess.run(
                    ["btrfs", "subvolume", "delete", str(snapshot_path)],
                    check=True,
                    capture_output=True
                )
                if checkpoint_id in self._metadata:
                    del self._metadata[checkpoint_id]
                    self._save_metadata()
                return True
            except subprocess.CalledProcessError:
                return False
        return False

    def exists(self, checkpoint_id: str) -> bool:
        return (self.snapshot_dir / checkpoint_id).exists()


class GitCheckpoint(CheckpointBackend):
    """
    Git-based checkpoints for non-Btrfs systems.

    Advantages:
    - Works everywhere git works
    - Familiar model for developers
    - Easy to inspect and compare

    Limitations:
    - Only tracks git-tracked files
    - Slower for large repos
    """

    def __init__(self, repo_path: Optional[Path] = None):
        self.repo_path = repo_path
        self._branch_prefix = "loop-checkpoint"

    def _git(self, *args, cwd: Optional[Path] = None) -> subprocess.CompletedProcess:
        """Run a git command."""
        working_dir = cwd or self.repo_path
        return subprocess.run(
            ["git", "-C", str(working_dir)] + list(args),
            capture_output=True,
            text=True
        )

    def _ensure_repo(self, path: Path):
        """Ensure we're working with a git repo."""
        if self.repo_path is None:
            self.repo_path = path

    @property
    def name(self) -> str:
        return "git"

    def create(self, name: str, path: Path) -> str:
        self._ensure_repo(path)
        timestamp = datetime.now().strftime("%Y%m%d_%H%M%S")
        checkpoint_id = f"{self._branch_prefix}/{name}_{timestamp}"

        # First, stash any uncommitted changes
        stash_result = self._git("stash", "push", "-m", f"loop-auto-stash-{checkpoint_id}")
        had_changes = "No local changes" not in stash_result.stdout

        # Get current HEAD
        head_result = self._git("rev-parse", "HEAD")
        if head_result.returncode != 0:
            # No commits yet, create initial commit
            self._git("add", "-A")
            self._git("commit", "-m", "Initial commit for loop checkpoint", "--allow-empty")

        # Create checkpoint branch at current HEAD
        result = self._git("branch", checkpoint_id)
        if result.returncode != 0:
            raise RuntimeError(f"Failed to create checkpoint branch: {result.stderr}")

        # Restore stashed changes if any
        if had_changes:
            self._git("stash", "pop")

        return checkpoint_id

    def restore(self, checkpoint_id: str, path: Path) -> bool:
        self._ensure_repo(path)

        # Stash current changes
        self._git("stash", "push", "-m", "loop-pre-restore")

        # Get current branch name to return to after restore
        current_branch = self._git("rev-parse", "--abbrev-ref", "HEAD").stdout.strip()

        # Hard reset to checkpoint
        # First checkout the checkpoint
        result = self._git("checkout", checkpoint_id)
        if result.returncode != 0:
            self._git("stash", "pop")
            return False

        # If we were on a different branch, delete it and recreate at this point
        if current_branch != checkpoint_id and not current_branch.startswith(self._branch_prefix):
            self._git("branch", "-D", current_branch)
            self._git("checkout", "-b", current_branch)

        return True

    def list(self, path: Path) -> List[Checkpoint]:
        self._ensure_repo(path)
        result = self._git(
            "branch", "--list", f"{self._branch_prefix}/*",
            "--format=%(refname:short)|%(creatordate:iso-strict)"
        )

        checkpoints = []
        for line in result.stdout.strip().split("\n"):
            if "|" in line:
                branch, date = line.split("|", 1)
                # Extract name from branch
                name = branch.replace(f"{self._branch_prefix}/", "")
                checkpoints.append(Checkpoint(
                    id=branch,
                    name=name,
                    created_at=date,
                    path=str(self.repo_path),
                    backend="git",
                    metadata={"branch": branch}
                ))

        return sorted(checkpoints, key=lambda c: c.created_at, reverse=True)

    def delete(self, checkpoint_id: str) -> bool:
        result = self._git("branch", "-D", checkpoint_id)
        return result.returncode == 0

    def exists(self, checkpoint_id: str) -> bool:
        result = self._git("rev-parse", "--verify", checkpoint_id)
        return result.returncode == 0


class NoneCheckpoint(CheckpointBackend):
    """
    No-op checkpoint backend.

    Use for:
    - Low-risk operations
    - When speed is critical
    - Testing without side effects
    """

    @property
    def name(self) -> str:
        return "none"

    def create(self, name: str, path: Path) -> str:
        timestamp = datetime.now().strftime("%Y%m%d_%H%M%S")
        return f"none_{name}_{timestamp}"

    def restore(self, checkpoint_id: str, path: Path) -> bool:
        # No-op: can't restore without checkpoints
        return False

    def list(self, path: Path) -> List[Checkpoint]:
        return []

    def delete(self, checkpoint_id: str) -> bool:
        return True

    def exists(self, checkpoint_id: str) -> bool:
        return False


def detect_backend(path: Path) -> str:
    """
    Auto-detect the best checkpoint backend for a path.

    Priority:
    1. Btrfs (if available and path is on Btrfs)
    2. Git (if path is in a git repo)
    3. None (fallback)
    """
    # Check for Btrfs
    try:
        result = subprocess.run(
            ["btrfs", "subvolume", "show", str(path)],
            capture_output=True
        )
        if result.returncode == 0:
            return "btrfs"
    except FileNotFoundError:
        pass  # btrfs command not available

    # Check for Git
    try:
        result = subprocess.run(
            ["git", "-C", str(path), "rev-parse", "--git-dir"],
            capture_output=True
        )
        if result.returncode == 0:
            return "git"
    except FileNotFoundError:
        pass  # git command not available

    return "none"


def get_backend(path: Path, strategy: str = "auto") -> CheckpointBackend:
    """
    Factory function to get appropriate checkpoint backend.

    Args:
        path: Working directory for the loop
        strategy: "auto", "btrfs", "git", or "none"

    Returns:
        Appropriate CheckpointBackend instance
    """
    if strategy == "auto":
        strategy = detect_backend(path)

    if strategy == "btrfs":
        return BtrfsCheckpoint()
    elif strategy == "git":
        return GitCheckpoint(path)
    elif strategy == "none":
        return NoneCheckpoint()
    else:
        raise ValueError(f"Unknown checkpoint strategy: {strategy}")
