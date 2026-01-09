"""Server management for LSP Pool."""

import asyncio
import json
import os
import signal
import sys
import time
from dataclasses import dataclass, field
from pathlib import Path
from typing import Any, Dict, List, Optional

from .config import Config, get_config, SOCKET_PATH


@dataclass
class ServerState:
    """State of a running language server."""
    language: str
    project: Path
    process: Optional[asyncio.subprocess.Process] = None
    pid: int = 0
    memory_mb: float = 0.0
    started_at: float = field(default_factory=time.time)
    last_query: float = field(default_factory=time.time)
    status: str = "initializing"  # initializing, warm, busy, error
    request_id: int = 0
    pending_requests: Dict[int, asyncio.Future] = field(default_factory=dict)

    @property
    def key(self) -> str:
        return f"{self.language}:{self.project}"

    def next_request_id(self) -> int:
        self.request_id += 1
        return self.request_id


class LSPProtocol:
    """LSP protocol implementation."""

    @staticmethod
    def encode_message(content: dict) -> bytes:
        """Encode a message with LSP headers."""
        body = json.dumps(content)
        header = f"Content-Length: {len(body)}\r\n\r\n"
        return (header + body).encode("utf-8")

    @staticmethod
    async def read_message(reader: asyncio.StreamReader) -> Optional[dict]:
        """Read a message from the LSP stream."""
        try:
            # Read headers
            content_length = 0
            while True:
                line = await reader.readline()
                if not line:
                    return None

                line = line.decode("utf-8").strip()
                if not line:
                    break

                if line.lower().startswith("content-length:"):
                    content_length = int(line.split(":")[1].strip())

            if content_length == 0:
                return None

            # Read body
            body = await reader.read(content_length)
            return json.loads(body.decode("utf-8"))

        except Exception:
            return None


class ServerManager:
    """Manages the pool of language servers."""

    def __init__(self, config: Optional[Config] = None):
        self.config = config or get_config()
        self.servers: Dict[str, ServerState] = {}
        self._lock = asyncio.Lock()

    async def warm(self, language: str, project: Path, priority: str = "normal") -> bool:
        """
        Warm a server for a language/project.

        Returns True if server is warm, False if failed.
        """
        key = f"{language}:{project}"

        async with self._lock:
            if key in self.servers and self.servers[key].status == "warm":
                return True

            # Check resource limits
            if len(self.servers) >= self.config.max_servers:
                await self._evict_lowest_priority()

            current_mem = self._current_memory()
            estimated = self._estimate_memory(language)
            if current_mem + estimated > self.config.memory_limit_mb:
                await self._evict_for_memory(estimated)

        # Get server configuration
        server_config = self.config.get_server_config(language)
        if not server_config:
            print(f"No configuration for language: {language}", file=sys.stderr)
            return False

        command = server_config.get("command", [])
        if isinstance(command, str):
            command = command.split()

        if not command:
            print(f"No command configured for: {language}", file=sys.stderr)
            return False

        try:
            # Start the server process
            process = await asyncio.create_subprocess_exec(
                *command,
                stdin=asyncio.subprocess.PIPE,
                stdout=asyncio.subprocess.PIPE,
                stderr=asyncio.subprocess.PIPE,
                cwd=str(project),
                env={**os.environ, "PWD": str(project)},
            )

            server = ServerState(
                language=language,
                project=project,
                process=process,
                pid=process.pid,
                status="initializing",
            )
            self.servers[key] = server

            # Initialize LSP connection
            success = await self._initialize_server(server)
            if success:
                server.status = "warm"
                print(f"Server warm: {key}")
                return True
            else:
                await self._stop_server(key)
                return False

        except Exception as e:
            print(f"Failed to start server for {language}: {e}", file=sys.stderr)
            return False

    async def cool(self, language: str, project: Optional[Path] = None):
        """Stop a server or all servers for a language."""
        async with self._lock:
            if project:
                key = f"{language}:{project}"
                if key in self.servers:
                    await self._stop_server(key)
            else:
                # Cool all servers for this language
                keys = [k for k in self.servers if k.startswith(f"{language}:")]
                for key in keys:
                    await self._stop_server(key)

    async def get_server(self, language: str, project: Path) -> Optional[ServerState]:
        """Get a warm server for the language/project."""
        key = f"{language}:{project}"

        if key not in self.servers:
            success = await self.warm(language, project, priority="high")
            if not success:
                return None

        server = self.servers.get(key)
        if server:
            server.last_query = time.time()

        return server

    def list_servers(self) -> List[Dict[str, Any]]:
        """List all running servers."""
        self._update_memory()
        return [
            {
                "language": s.language,
                "project": str(s.project),
                "pid": s.pid,
                "memory_mb": round(s.memory_mb, 1),
                "status": s.status,
                "uptime_seconds": int(time.time() - s.started_at),
                "idle_seconds": int(time.time() - s.last_query),
            }
            for s in self.servers.values()
        ]

    async def _initialize_server(self, server: ServerState) -> bool:
        """Send LSP initialize request to server."""
        try:
            request = {
                "jsonrpc": "2.0",
                "id": server.next_request_id(),
                "method": "initialize",
                "params": {
                    "processId": os.getpid(),
                    "rootUri": f"file://{server.project}",
                    "capabilities": {
                        "textDocument": {
                            "hover": {"contentFormat": ["markdown", "plaintext"]},
                            "completion": {
                                "completionItem": {"snippetSupport": True}
                            },
                            "definition": {},
                            "references": {},
                            "diagnostics": {},
                        }
                    },
                },
            }

            response = await self._send_request(server, request)
            if not response or "error" in response:
                return False

            # Send initialized notification
            notification = {
                "jsonrpc": "2.0",
                "method": "initialized",
                "params": {},
            }
            await self._send_notification(server, notification)

            return True

        except Exception as e:
            print(f"Initialize failed: {e}", file=sys.stderr)
            return False

    async def _send_request(self, server: ServerState, request: dict, timeout: float = 30.0) -> Optional[dict]:
        """Send a request to the server and wait for response."""
        if not server.process or not server.process.stdin or not server.process.stdout:
            return None

        try:
            message = LSPProtocol.encode_message(request)
            server.process.stdin.write(message)
            await server.process.stdin.drain()

            # Read response with timeout
            response = await asyncio.wait_for(
                LSPProtocol.read_message(server.process.stdout),
                timeout=timeout
            )
            return response

        except asyncio.TimeoutError:
            print(f"Request timed out: {request.get('method')}", file=sys.stderr)
            return None
        except Exception as e:
            print(f"Request failed: {e}", file=sys.stderr)
            return None

    async def _send_notification(self, server: ServerState, notification: dict):
        """Send a notification to the server (no response expected)."""
        if not server.process or not server.process.stdin:
            return

        try:
            message = LSPProtocol.encode_message(notification)
            server.process.stdin.write(message)
            await server.process.stdin.drain()
        except Exception:
            pass

    async def _stop_server(self, key: str):
        """Stop a server and remove from pool."""
        if key not in self.servers:
            return

        server = self.servers[key]
        if server.process:
            try:
                # Try graceful shutdown first
                shutdown_request = {
                    "jsonrpc": "2.0",
                    "id": server.next_request_id(),
                    "method": "shutdown",
                }
                await asyncio.wait_for(
                    self._send_request(server, shutdown_request),
                    timeout=5.0
                )

                # Send exit notification
                exit_notification = {
                    "jsonrpc": "2.0",
                    "method": "exit",
                }
                await self._send_notification(server, exit_notification)

                # Wait for process to exit
                await asyncio.wait_for(server.process.wait(), timeout=5.0)

            except asyncio.TimeoutError:
                # Force kill if graceful shutdown failed
                server.process.kill()
                await server.process.wait()

            except Exception:
                try:
                    server.process.kill()
                except Exception:
                    pass

        del self.servers[key]
        print(f"Server cooled: {key}")

    async def _evict_lowest_priority(self):
        """Evict the lowest priority server to make room."""
        if not self.servers:
            return

        # Sort by last_query (oldest first)
        sorted_servers = sorted(
            self.servers.items(),
            key=lambda x: x[1].last_query
        )
        if sorted_servers:
            await self._stop_server(sorted_servers[0][0])

    async def _evict_for_memory(self, needed_mb: float):
        """Evict servers until enough memory is available."""
        while self._current_memory() + needed_mb > self.config.memory_limit_mb:
            if not self.servers:
                break
            await self._evict_lowest_priority()

    def _current_memory(self) -> float:
        """Get current total memory usage of pool."""
        self._update_memory()
        return sum(s.memory_mb for s in self.servers.values())

    def _update_memory(self):
        """Update memory usage for all servers."""
        try:
            import psutil

            for server in self.servers.values():
                try:
                    process = psutil.Process(server.pid)
                    server.memory_mb = process.memory_info().rss / 1024 / 1024
                except (psutil.NoSuchProcess, psutil.AccessDenied):
                    pass
        except ImportError:
            # psutil not available, skip memory tracking
            pass

    def _estimate_memory(self, language: str) -> float:
        """Estimate memory for a language server."""
        config = self.config.get_server_config(language)
        if config:
            return config.get("memory_estimate_mb", 300)
        return 300

    async def stop_all(self):
        """Stop all servers."""
        keys = list(self.servers.keys())
        for key in keys:
            await self._stop_server(key)


# CLI entry point
def main():
    """CLI for server management."""
    import click

    @click.group()
    def cli():
        """LSP server management."""
        pass

    @cli.command()
    @click.option("--language", "-l", required=True, help="Language")
    @click.option("--path", "-p", default=".", help="Project path")
    def warm(language, path):
        """Warm a server for a language/project."""
        async def run():
            manager = ServerManager()
            success = await manager.warm(language, Path(path).resolve())
            if success:
                click.echo(f"Server warm: {language}")
            else:
                click.echo(f"Failed to warm: {language}")
                sys.exit(1)

        asyncio.run(run())

    @cli.command()
    @click.option("--language", "-l", required=True, help="Language")
    def cool(language):
        """Cool (stop) servers for a language."""
        async def run():
            manager = ServerManager()
            await manager.cool(language)
            click.echo(f"Servers cooled: {language}")

        asyncio.run(run())

    @cli.command("list")
    @click.option("--json", "as_json", is_flag=True, help="Output as JSON")
    def list_servers(as_json):
        """List running servers."""
        manager = ServerManager()
        servers = manager.list_servers()

        if as_json:
            click.echo(json.dumps(servers, indent=2))
        elif not servers:
            click.echo("No servers running.")
        else:
            for s in servers:
                click.echo(f"{s['language']:12} {s['status']:12} {s['memory_mb']:>6.1f}MB  {s['project']}")

    cli()


if __name__ == "__main__":
    main()
