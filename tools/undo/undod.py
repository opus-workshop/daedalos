#!/usr/bin/env python3
"""
undod - Undo Daemon for Daedalos

The undod daemon watches for file changes and automatically creates
backup entries in the undo timeline. It provides:

- Real-time file change detection (using watchdog)
- Automatic backup creation with debouncing
- Named checkpoint support
- Unix socket IPC for undo commands
- Web UI at localhost:7778

"Every change is cheap to undo."
"""

import os
import sys
import json
import signal
import socket
import threading
import time
import hashlib
import shutil
import argparse
import sqlite3
from pathlib import Path
from datetime import datetime
from dataclasses import dataclass, asdict
from typing import Optional, Dict, List, Set
from http.server import HTTPServer, BaseHTTPRequestHandler
from collections import defaultdict

# Try to import watchdog, fall back to polling if not available
try:
    from watchdog.observers import Observer
    from watchdog.events import FileSystemEventHandler, FileSystemEvent
    HAS_WATCHDOG = True
except ImportError:
    HAS_WATCHDOG = False
    print("Warning: watchdog not installed, using polling fallback")


@dataclass
class DaemonConfig:
    """Configuration for undod."""
    watch_paths: List[str]
    socket_path: str = "/run/daedalos/undod.sock"
    web_ui_port: int = 7778
    data_dir: str = str(Path.home() / ".local/share/daedalos/undo")
    debounce_ms: int = 500
    max_file_size: int = 10 * 1024 * 1024  # 10MB
    backup_retention_hours: int = 24 * 7  # 1 week
    exclude_patterns: List[str] = None

    def __post_init__(self):
        if self.exclude_patterns is None:
            self.exclude_patterns = [
                '.git', 'node_modules', '__pycache__', '.pytest_cache',
                'target', 'build', 'dist', '.DS_Store', '.undo',
                '*.pyc', '*.pyo', '.env', '.venv', 'venv'
            ]

    @classmethod
    def load(cls, path: Optional[Path] = None) -> "DaemonConfig":
        if path is None:
            path = Path.home() / ".config/daedalos/undo/undod.yaml"

        if path.exists():
            import yaml
            with open(path) as f:
                data = yaml.safe_load(f) or {}
            return cls(**data)
        return cls(watch_paths=[str(Path.cwd())])


@dataclass
class UndoEntry:
    """A single entry in the undo timeline."""
    id: str
    timestamp: str
    change_type: str  # edit, create, delete, rename, checkpoint
    file_path: str
    description: str
    backup_hash: Optional[str]
    file_size: int
    project_path: str


class UndoDatabase:
    """SQLite database for undo timeline."""

    def __init__(self, data_dir: Path):
        self.data_dir = Path(data_dir)
        self.data_dir.mkdir(parents=True, exist_ok=True)
        self.db_path = self.data_dir / "timeline.db"
        self.backup_dir = self.data_dir / "backups"
        self.backup_dir.mkdir(exist_ok=True)
        self._init_db()

    def _init_db(self):
        """Initialize the database schema."""
        with sqlite3.connect(self.db_path) as conn:
            conn.execute("""
                CREATE TABLE IF NOT EXISTS entries (
                    id TEXT PRIMARY KEY,
                    timestamp TEXT NOT NULL,
                    change_type TEXT NOT NULL,
                    file_path TEXT NOT NULL,
                    description TEXT,
                    backup_hash TEXT,
                    file_size INTEGER,
                    project_path TEXT
                )
            """)
            conn.execute("""
                CREATE INDEX IF NOT EXISTS idx_timestamp ON entries(timestamp DESC)
            """)
            conn.execute("""
                CREATE INDEX IF NOT EXISTS idx_file_path ON entries(file_path)
            """)
            conn.execute("""
                CREATE TABLE IF NOT EXISTS checkpoints (
                    id TEXT PRIMARY KEY,
                    name TEXT NOT NULL,
                    timestamp TEXT NOT NULL,
                    description TEXT,
                    entry_ids TEXT
                )
            """)

    def add_entry(self, entry: UndoEntry):
        """Add an entry to the timeline."""
        with sqlite3.connect(self.db_path) as conn:
            conn.execute("""
                INSERT OR REPLACE INTO entries
                (id, timestamp, change_type, file_path, description, backup_hash, file_size, project_path)
                VALUES (?, ?, ?, ?, ?, ?, ?, ?)
            """, (
                entry.id, entry.timestamp, entry.change_type, entry.file_path,
                entry.description, entry.backup_hash, entry.file_size, entry.project_path
            ))

    def get_entries(self, limit: int = 20, file_path: Optional[str] = None) -> List[UndoEntry]:
        """Get timeline entries."""
        with sqlite3.connect(self.db_path) as conn:
            conn.row_factory = sqlite3.Row
            if file_path:
                rows = conn.execute("""
                    SELECT * FROM entries WHERE file_path = ?
                    ORDER BY timestamp DESC LIMIT ?
                """, (file_path, limit)).fetchall()
            else:
                rows = conn.execute("""
                    SELECT * FROM entries ORDER BY timestamp DESC LIMIT ?
                """, (limit,)).fetchall()

            return [UndoEntry(**dict(row)) for row in rows]

    def get_entry(self, entry_id: str) -> Optional[UndoEntry]:
        """Get a specific entry by ID."""
        with sqlite3.connect(self.db_path) as conn:
            conn.row_factory = sqlite3.Row
            row = conn.execute(
                "SELECT * FROM entries WHERE id = ?", (entry_id,)
            ).fetchone()
            return UndoEntry(**dict(row)) if row else None

    def backup_file(self, file_path: Path) -> Optional[str]:
        """Create a backup of a file and return its hash."""
        if not file_path.exists():
            return None

        try:
            content = file_path.read_bytes()
            file_hash = hashlib.sha256(content).hexdigest()[:16]

            backup_path = self.backup_dir / file_hash
            if not backup_path.exists():
                backup_path.write_bytes(content)

            return file_hash
        except (PermissionError, OSError):
            return None

    def restore_file(self, file_path: Path, backup_hash: str) -> bool:
        """Restore a file from backup."""
        backup_path = self.backup_dir / backup_hash
        if not backup_path.exists():
            return False

        try:
            content = backup_path.read_bytes()
            file_path.parent.mkdir(parents=True, exist_ok=True)
            file_path.write_bytes(content)
            return True
        except (PermissionError, OSError):
            return False

    def add_checkpoint(self, name: str, description: str = "") -> str:
        """Create a named checkpoint."""
        checkpoint_id = hashlib.sha256(
            f"{name}{datetime.now().isoformat()}".encode()
        ).hexdigest()[:12]

        # Get recent entry IDs
        entries = self.get_entries(limit=100)
        entry_ids = json.dumps([e.id for e in entries])

        with sqlite3.connect(self.db_path) as conn:
            conn.execute("""
                INSERT INTO checkpoints (id, name, timestamp, description, entry_ids)
                VALUES (?, ?, ?, ?, ?)
            """, (checkpoint_id, name, datetime.now().isoformat(), description, entry_ids))

        # Also add as timeline entry
        self.add_entry(UndoEntry(
            id=checkpoint_id,
            timestamp=datetime.now().isoformat(),
            change_type="checkpoint",
            file_path="",
            description=f"Checkpoint: {name}",
            backup_hash=None,
            file_size=0,
            project_path=""
        ))

        return checkpoint_id

    def get_checkpoint(self, name_or_id: str) -> Optional[dict]:
        """Get a checkpoint by name or ID."""
        with sqlite3.connect(self.db_path) as conn:
            conn.row_factory = sqlite3.Row
            row = conn.execute("""
                SELECT * FROM checkpoints
                WHERE id = ? OR name = ?
                ORDER BY timestamp DESC LIMIT 1
            """, (name_or_id, name_or_id)).fetchone()
            return dict(row) if row else None


class FileChangeHandler(FileSystemEventHandler if HAS_WATCHDOG else object):
    """Handler for file system events."""

    def __init__(self, daemon: "UndoDaemon"):
        self.daemon = daemon
        self.pending_changes: Dict[str, float] = {}
        self.lock = threading.Lock()

    def _should_ignore(self, path: str) -> bool:
        """Check if path should be ignored."""
        for pattern in self.daemon.config.exclude_patterns:
            if pattern in path:
                return True
            if pattern.startswith('*') and path.endswith(pattern[1:]):
                return True
        return False

    def _queue_change(self, path: str, event_type: str):
        """Queue a change with debouncing."""
        if self._should_ignore(path):
            return

        with self.lock:
            self.pending_changes[path] = time.time()

        # Schedule processing after debounce
        threading.Timer(
            self.daemon.config.debounce_ms / 1000,
            self._process_change,
            args=(path, event_type)
        ).start()

    def _process_change(self, path: str, event_type: str):
        """Process a file change after debouncing."""
        with self.lock:
            if path not in self.pending_changes:
                return

            # Check if this is still the latest change
            if time.time() - self.pending_changes[path] < self.daemon.config.debounce_ms / 1000:
                return

            del self.pending_changes[path]

        self.daemon.record_change(path, event_type)

    def on_modified(self, event):
        if not event.is_directory:
            self._queue_change(event.src_path, "edit")

    def on_created(self, event):
        if not event.is_directory:
            self._queue_change(event.src_path, "create")

    def on_deleted(self, event):
        if not event.is_directory:
            self._queue_change(event.src_path, "delete")

    def on_moved(self, event):
        if not event.is_directory:
            self._queue_change(event.dest_path, "rename")


class UndoDaemon:
    """Main undo daemon."""

    def __init__(self, config: DaemonConfig):
        self.config = config
        self.running = False
        self.db = UndoDatabase(Path(config.data_dir))
        self.observer = None
        self.socket: Optional[socket.socket] = None
        self.web_server: Optional[HTTPServer] = None
        self.lock = threading.Lock()
        self.stats = {
            "changes_recorded": 0,
            "files_backed_up": 0,
            "start_time": None
        }

        # Ensure directories exist
        Path(config.socket_path).parent.mkdir(parents=True, exist_ok=True)

    def start(self):
        """Start the daemon."""
        print(f"Starting undod v1.0.0")
        print(f"Socket: {self.config.socket_path}")
        print(f"Web UI: http://localhost:{self.config.web_ui_port}")
        print(f"Watch paths: {', '.join(self.config.watch_paths)}")
        print()

        self.running = True
        self.stats["start_time"] = datetime.now().isoformat()

        # Set up signal handlers
        signal.signal(signal.SIGTERM, self._handle_shutdown)
        signal.signal(signal.SIGINT, self._handle_shutdown)

        # Start file watcher
        self._start_watcher()

        # Start IPC socket
        self._start_socket()

        # Start web UI
        self._start_web_ui()

        # Create session checkpoint
        self.db.add_checkpoint("session-start", "Watch session started")

        # Main loop
        try:
            while self.running:
                time.sleep(1)
        except KeyboardInterrupt:
            pass
        finally:
            self.stop()

    def stop(self):
        """Stop the daemon."""
        print("Stopping undod...")
        self.running = False

        if self.observer:
            self.observer.stop()
            self.observer.join()

        if self.socket:
            self.socket.close()
            try:
                os.unlink(self.config.socket_path)
            except OSError:
                pass

        if self.web_server:
            self.web_server.shutdown()

        print("undod stopped")

    def _handle_shutdown(self, signum, frame):
        self.running = False

    def _start_watcher(self):
        """Start the file system watcher."""
        if not HAS_WATCHDOG:
            print("Warning: watchdog not available, file watching disabled")
            return

        handler = FileChangeHandler(self)
        self.observer = Observer()

        for path in self.config.watch_paths:
            if Path(path).exists():
                self.observer.schedule(handler, path, recursive=True)
                print(f"Watching: {path}")

        self.observer.start()

    def _start_socket(self):
        """Start the IPC socket."""
        try:
            os.unlink(self.config.socket_path)
        except OSError:
            pass

        self.socket = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
        self.socket.bind(self.config.socket_path)
        self.socket.listen(5)
        self.socket.setblocking(False)

        thread = threading.Thread(target=self._socket_handler, daemon=True)
        thread.start()

    def _socket_handler(self):
        """Handle IPC socket connections."""
        while self.running:
            try:
                conn, _ = self.socket.accept()
                data = conn.recv(4096).decode()
                response = self._handle_command(json.loads(data))
                conn.send(json.dumps(response).encode())
                conn.close()
            except BlockingIOError:
                time.sleep(0.1)
            except Exception as e:
                print(f"Socket error: {e}")

    def _handle_command(self, cmd: dict) -> dict:
        """Handle a command from the IPC socket."""
        action = cmd.get("action")

        if action == "status":
            return self._cmd_status()
        elif action == "timeline":
            return self._cmd_timeline(cmd.get("limit", 20), cmd.get("file"))
        elif action == "undo_last":
            return self._cmd_undo_last(cmd.get("count", 1))
        elif action == "restore":
            return self._cmd_restore(cmd.get("entry_id"))
        elif action == "checkpoint":
            return self._cmd_checkpoint(cmd.get("name"), cmd.get("description", ""))
        else:
            return {"error": f"Unknown action: {action}"}

    def _cmd_status(self) -> dict:
        return {
            "running": self.running,
            "watch_paths": self.config.watch_paths,
            "changes_recorded": self.stats["changes_recorded"],
            "files_backed_up": self.stats["files_backed_up"],
            "start_time": self.stats["start_time"]
        }

    def _cmd_timeline(self, limit: int, file_path: Optional[str]) -> dict:
        entries = self.db.get_entries(limit=limit, file_path=file_path)
        return {
            "entries": [asdict(e) for e in entries]
        }

    def _cmd_undo_last(self, count: int) -> dict:
        entries = self.db.get_entries(limit=count)
        restored = 0

        for entry in entries:
            if entry.change_type == "checkpoint":
                continue

            if entry.backup_hash:
                file_path = Path(entry.file_path)
                if self.db.restore_file(file_path, entry.backup_hash):
                    restored += 1

        return {"restored": restored, "requested": count}

    def _cmd_restore(self, entry_id: str) -> dict:
        entry = self.db.get_entry(entry_id)
        if not entry:
            return {"error": "Entry not found"}

        if not entry.backup_hash:
            return {"error": "No backup available for this entry"}

        file_path = Path(entry.file_path)
        if self.db.restore_file(file_path, entry.backup_hash):
            return {"success": True, "file": entry.file_path}
        return {"error": "Failed to restore file"}

    def _cmd_checkpoint(self, name: str, description: str) -> dict:
        checkpoint_id = self.db.add_checkpoint(name, description)
        return {"checkpoint_id": checkpoint_id}

    def record_change(self, path: str, change_type: str):
        """Record a file change."""
        file_path = Path(path)

        # Skip if file too large
        try:
            if file_path.exists() and file_path.stat().st_size > self.config.max_file_size:
                return
        except OSError:
            return

        # Create backup
        backup_hash = None
        if change_type != "delete" and file_path.exists():
            backup_hash = self.db.backup_file(file_path)
            if backup_hash:
                self.stats["files_backed_up"] += 1

        # Create entry
        entry_id = hashlib.sha256(
            f"{path}{datetime.now().isoformat()}".encode()
        ).hexdigest()[:12]

        try:
            file_size = file_path.stat().st_size if file_path.exists() else 0
        except OSError:
            file_size = 0

        entry = UndoEntry(
            id=entry_id,
            timestamp=datetime.now().isoformat(),
            change_type=change_type,
            file_path=str(file_path),
            description=f"File {change_type}d",
            backup_hash=backup_hash,
            file_size=file_size,
            project_path=str(file_path.parent)
        )

        self.db.add_entry(entry)
        self.stats["changes_recorded"] += 1

    def _start_web_ui(self):
        """Start the web UI server."""
        daemon = self

        class WebHandler(BaseHTTPRequestHandler):
            def log_message(self, format, *args):
                pass

            def do_GET(self):
                if self.path == "/":
                    self.send_response(200)
                    self.send_header("Content-type", "text/html")
                    self.end_headers()
                    self.wfile.write(daemon._generate_web_ui().encode())
                elif self.path == "/api/status":
                    self.send_response(200)
                    self.send_header("Content-type", "application/json")
                    self.end_headers()
                    self.wfile.write(json.dumps(daemon._cmd_status()).encode())
                elif self.path == "/api/timeline":
                    self.send_response(200)
                    self.send_header("Content-type", "application/json")
                    self.end_headers()
                    self.wfile.write(json.dumps(daemon._cmd_timeline(50, None)).encode())
                else:
                    self.send_response(404)
                    self.end_headers()

        self.web_server = HTTPServer(("localhost", self.config.web_ui_port), WebHandler)
        thread = threading.Thread(target=self.web_server.serve_forever, daemon=True)
        thread.start()

    def _generate_web_ui(self) -> str:
        """Generate the web UI HTML."""
        entries = self.db.get_entries(limit=30)

        entries_html = ""
        for entry in entries:
            type_class = {
                "edit": "type-edit",
                "create": "type-create",
                "delete": "type-delete",
                "rename": "type-rename",
                "checkpoint": "type-checkpoint"
            }.get(entry.change_type, "")

            time_str = entry.timestamp.split("T")[1][:8] if "T" in entry.timestamp else entry.timestamp
            file_name = Path(entry.file_path).name if entry.file_path else "-"

            entries_html += f"""
            <tr class="{type_class}">
                <td><code>{time_str}</code></td>
                <td><span class="type">{entry.change_type}</span></td>
                <td>{file_name}</td>
                <td>{entry.description}</td>
            </tr>
            """

        if not entries_html:
            entries_html = "<tr><td colspan='4'>No entries yet</td></tr>"

        return f"""
<!DOCTYPE html>
<html>
<head>
    <title>undod - Daedalos Undo Daemon</title>
    <style>
        body {{
            font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif;
            background: #1a1a2e;
            color: #eee;
            padding: 20px;
            max-width: 1000px;
            margin: 0 auto;
        }}
        h1 {{
            color: #a6e3a1;
            border-bottom: 2px solid #a6e3a1;
            padding-bottom: 10px;
        }}
        .stats {{
            display: flex;
            gap: 20px;
            margin-bottom: 20px;
        }}
        .stat {{
            background: #16213e;
            padding: 15px 25px;
            border-radius: 8px;
        }}
        .stat-value {{
            font-size: 2em;
            font-weight: bold;
            color: #a6e3a1;
        }}
        .stat-label {{
            color: #888;
            font-size: 0.9em;
        }}
        table {{
            width: 100%;
            border-collapse: collapse;
        }}
        th, td {{
            padding: 10px;
            text-align: left;
            border-bottom: 1px solid #333;
        }}
        th {{
            background: #16213e;
            color: #a6e3a1;
        }}
        tr:hover {{
            background: #16213e;
        }}
        .type {{
            padding: 2px 6px;
            border-radius: 4px;
            font-size: 0.85em;
        }}
        .type-edit .type {{ background: #0d7377; }}
        .type-create .type {{ background: #1d8348; }}
        .type-delete .type {{ background: #922b21; }}
        .type-rename .type {{ background: #7d6608; }}
        .type-checkpoint .type {{ background: #6c3483; }}
        code {{
            background: #0f0f23;
            padding: 2px 6px;
            border-radius: 3px;
            font-family: 'Monaco', 'Menlo', monospace;
        }}
        .refresh {{ color: #888; font-size: 0.9em; }}
    </style>
    <script>
        setTimeout(() => location.reload(), 5000);
    </script>
</head>
<body>
    <div style="display: flex; justify-content: space-between; align-items: center;">
        <h1>undod - Undo Timeline</h1>
        <span class="refresh">Auto-refreshes every 5s</span>
    </div>

    <div class="stats">
        <div class="stat">
            <div class="stat-value">{self.stats['changes_recorded']}</div>
            <div class="stat-label">Changes Recorded</div>
        </div>
        <div class="stat">
            <div class="stat-value">{self.stats['files_backed_up']}</div>
            <div class="stat-label">Files Backed Up</div>
        </div>
        <div class="stat">
            <div class="stat-value">{len(self.config.watch_paths)}</div>
            <div class="stat-label">Watch Paths</div>
        </div>
    </div>

    <table>
        <thead>
            <tr>
                <th>Time</th>
                <th>Type</th>
                <th>File</th>
                <th>Description</th>
            </tr>
        </thead>
        <tbody>
            {entries_html}
        </tbody>
    </table>

    <p style="color: #666; margin-top: 40px; text-align: center;">
        Daedalos - "Every change is cheap to undo"
    </p>
</body>
</html>
        """


def main():
    parser = argparse.ArgumentParser(description="Undo Daemon for Daedalos")
    subparsers = parser.add_subparsers(dest="command", help="Command")

    # start command
    start_parser = subparsers.add_parser("start", help="Start the daemon")
    start_parser.add_argument("paths", nargs="*", default=["."], help="Paths to watch")
    start_parser.add_argument("--port", type=int, default=7778, help="Web UI port")

    # stop command
    subparsers.add_parser("stop", help="Stop the daemon")

    # status command
    subparsers.add_parser("status", help="Show daemon status")

    args = parser.parse_args()

    if args.command == "start" or args.command is None:
        watch_paths = args.paths if hasattr(args, 'paths') else ["."]
        watch_paths = [str(Path(p).resolve()) for p in watch_paths]

        config = DaemonConfig(
            watch_paths=watch_paths,
            web_ui_port=args.port if hasattr(args, 'port') else 7778
        )

        daemon = UndoDaemon(config)
        daemon.start()

    elif args.command == "stop":
        config = DaemonConfig(watch_paths=[])
        try:
            sock = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
            sock.connect(config.socket_path)
            sock.send(json.dumps({"action": "stop"}).encode())
            sock.close()
            print("Stop signal sent")
        except Exception as e:
            print(f"Could not connect to daemon: {e}")
            sys.exit(1)

    elif args.command == "status":
        config = DaemonConfig(watch_paths=[])
        try:
            sock = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
            sock.connect(config.socket_path)
            sock.send(json.dumps({"action": "status"}).encode())
            response = json.loads(sock.recv(4096).decode())
            sock.close()

            print("undod status:")
            print(f"  Running: {response.get('running', False)}")
            print(f"  Changes recorded: {response.get('changes_recorded', 0)}")
            print(f"  Files backed up: {response.get('files_backed_up', 0)}")
            print(f"  Watch paths: {', '.join(response.get('watch_paths', []))}")
        except ConnectionRefusedError:
            print("undod is not running")
            sys.exit(1)
        except FileNotFoundError:
            print("undod is not running (socket not found)")
            sys.exit(1)


if __name__ == "__main__":
    main()
