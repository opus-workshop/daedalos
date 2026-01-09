"""LSP Pool Daemon - manages warm language servers."""

import asyncio
import json
import os
import signal
import sys
from pathlib import Path
from typing import Optional

from .config import Config, get_config, DATA_DIR, SOCKET_PATH
from .predictor import Predictor
from .servers import ServerManager


class LSPPoolDaemon:
    """Daemon managing pool of language servers."""

    def __init__(self, config: Optional[Config] = None, socket_path: Optional[Path] = None):
        self.config = config or get_config()
        self.socket_path = socket_path or SOCKET_PATH
        self.server_manager = ServerManager(self.config)
        self.predictor = Predictor()
        self._server: Optional[asyncio.Server] = None
        self._running = False
        self._pid_file = DATA_DIR / "daemon.pid"

    async def start(self):
        """Start the pool daemon."""
        if self._is_running():
            print("Daemon is already running.")
            return

        self._running = True
        self._write_pid()

        # Setup signal handlers
        loop = asyncio.get_event_loop()
        for sig in (signal.SIGTERM, signal.SIGINT):
            loop.add_signal_handler(sig, lambda: asyncio.create_task(self.stop()))

        # Pre-warm predicted servers
        if self.config.get("warmup_on_start", True):
            predictions = self.predictor.predict(self.config.max_servers)
            for pred in predictions:
                try:
                    await self.server_manager.warm(pred["language"], Path(pred["project"]))
                except Exception as e:
                    print(f"Failed to warm {pred['language']}: {e}", file=sys.stderr)

        # Ensure socket directory exists
        self.socket_path.parent.mkdir(parents=True, exist_ok=True)

        # Remove stale socket
        if self.socket_path.exists():
            self.socket_path.unlink()

        # Start listening
        print(f"LSP Pool daemon starting on {self.socket_path}")
        self._server = await asyncio.start_unix_server(
            self._handle_client,
            path=str(self.socket_path)
        )

        # Start background tasks
        idle_task = asyncio.create_task(self._idle_cleanup_loop())

        async with self._server:
            await self._server.serve_forever()

        idle_task.cancel()

    async def stop(self):
        """Stop the daemon."""
        print("Stopping LSP Pool daemon...")
        self._running = False

        # Stop all servers
        await self.server_manager.stop_all()

        # Close socket server
        if self._server:
            self._server.close()
            await self._server.wait_closed()

        # Cleanup
        if self.socket_path.exists():
            self.socket_path.unlink()
        if self._pid_file.exists():
            self._pid_file.unlink()

        print("Daemon stopped.")

    async def _handle_client(self, reader: asyncio.StreamReader, writer: asyncio.StreamWriter):
        """Handle incoming client request."""
        try:
            data = await reader.read(65536)
            if not data:
                return

            request = json.loads(data.decode())
            response = await self._process_request(request)

            writer.write(json.dumps(response).encode())
            await writer.drain()

        except json.JSONDecodeError as e:
            response = {"success": False, "error": f"Invalid JSON: {e}"}
            writer.write(json.dumps(response).encode())
            await writer.drain()

        except Exception as e:
            response = {"success": False, "error": str(e)}
            writer.write(json.dumps(response).encode())
            await writer.drain()

        finally:
            writer.close()
            await writer.wait_closed()

    async def _process_request(self, request: dict) -> dict:
        """Process a client request."""
        req_type = request.get("type")

        if req_type == "status":
            servers = self.server_manager.list_servers()
            return {
                "success": True,
                "servers": servers,
                "config": {
                    "max_servers": self.config.max_servers,
                    "memory_limit_mb": self.config.memory_limit_mb,
                },
            }

        elif req_type == "warm":
            language = request.get("language")
            project = request.get("project", ".")
            success = await self.server_manager.warm(language, Path(project))
            return {"success": success}

        elif req_type == "cool":
            language = request.get("language")
            project = request.get("project")
            await self.server_manager.cool(language, Path(project) if project else None)
            return {"success": True}

        elif req_type == "query":
            return await self._handle_query(request)

        elif req_type == "predict":
            predictions = self.predictor.predict(request.get("n", 5))
            return {"success": True, "predictions": predictions}

        elif req_type == "stop":
            asyncio.create_task(self.stop())
            return {"success": True, "message": "Daemon stopping"}

        else:
            return {"success": False, "error": f"Unknown request type: {req_type}"}

    async def _handle_query(self, request: dict) -> dict:
        """Handle an LSP query request."""
        command = request.get("command")
        file_path = Path(request.get("file", ""))
        line = request.get("line", 1)
        col = request.get("col", 1)

        # Detect language from file extension
        language = self._detect_language(file_path)
        if not language:
            return {"success": False, "error": f"Unknown language for: {file_path}"}

        # Find project root
        project = self._find_project_root(file_path)

        # Get or warm server
        server = await self.server_manager.get_server(language, project)
        if not server:
            return {"success": False, "error": f"Could not start server for: {language}"}

        # Record activity for prediction
        self.predictor.record_activity(language, project, command)

        # Send LSP request
        try:
            if command == "hover":
                result = await self._request_hover(server, file_path, line, col)
            elif command == "definition":
                result = await self._request_definition(server, file_path, line, col)
            elif command == "references":
                result = await self._request_references(server, file_path, line, col)
            elif command == "completion":
                result = await self._request_completion(server, file_path, line, col)
            elif command == "diagnostics":
                result = await self._request_diagnostics(server, file_path)
            else:
                return {"success": False, "error": f"Unknown command: {command}"}

            return {"success": True, "result": result}

        except Exception as e:
            return {"success": False, "error": str(e)}

    async def _request_hover(self, server, file_path: Path, line: int, col: int) -> dict:
        """Request hover information."""
        from .servers import LSPProtocol

        request = {
            "jsonrpc": "2.0",
            "id": server.next_request_id(),
            "method": "textDocument/hover",
            "params": {
                "textDocument": {"uri": f"file://{file_path}"},
                "position": {"line": line - 1, "character": col - 1}
            }
        }
        return await self._send_lsp_request(server, request)

    async def _request_definition(self, server, file_path: Path, line: int, col: int) -> dict:
        """Request go-to-definition."""
        request = {
            "jsonrpc": "2.0",
            "id": server.next_request_id(),
            "method": "textDocument/definition",
            "params": {
                "textDocument": {"uri": f"file://{file_path}"},
                "position": {"line": line - 1, "character": col - 1}
            }
        }
        return await self._send_lsp_request(server, request)

    async def _request_references(self, server, file_path: Path, line: int, col: int) -> dict:
        """Request find references."""
        request = {
            "jsonrpc": "2.0",
            "id": server.next_request_id(),
            "method": "textDocument/references",
            "params": {
                "textDocument": {"uri": f"file://{file_path}"},
                "position": {"line": line - 1, "character": col - 1},
                "context": {"includeDeclaration": True}
            }
        }
        return await self._send_lsp_request(server, request)

    async def _request_completion(self, server, file_path: Path, line: int, col: int) -> dict:
        """Request completions."""
        request = {
            "jsonrpc": "2.0",
            "id": server.next_request_id(),
            "method": "textDocument/completion",
            "params": {
                "textDocument": {"uri": f"file://{file_path}"},
                "position": {"line": line - 1, "character": col - 1}
            }
        }
        return await self._send_lsp_request(server, request)

    async def _request_diagnostics(self, server, file_path: Path) -> dict:
        """Request diagnostics for a file."""
        # Open the document first
        content = file_path.read_text() if file_path.exists() else ""

        open_notification = {
            "jsonrpc": "2.0",
            "method": "textDocument/didOpen",
            "params": {
                "textDocument": {
                    "uri": f"file://{file_path}",
                    "languageId": self._detect_language(file_path),
                    "version": 1,
                    "text": content
                }
            }
        }

        from .servers import LSPProtocol
        if server.process and server.process.stdin:
            message = LSPProtocol.encode_message(open_notification)
            server.process.stdin.write(message)
            await server.process.stdin.drain()

        # Diagnostics are usually sent asynchronously
        # For now, return empty - a full implementation would wait for publishDiagnostics
        return {"diagnostics": []}

    async def _send_lsp_request(self, server, request: dict) -> dict:
        """Send LSP request and get response."""
        from .servers import LSPProtocol

        if not server.process or not server.process.stdin or not server.process.stdout:
            return {"error": "Server not running"}

        try:
            message = LSPProtocol.encode_message(request)
            server.process.stdin.write(message)
            await server.process.stdin.drain()

            response = await asyncio.wait_for(
                LSPProtocol.read_message(server.process.stdout),
                timeout=30.0
            )
            return response.get("result", {}) if response else {}

        except asyncio.TimeoutError:
            return {"error": "Request timed out"}
        except Exception as e:
            return {"error": str(e)}

    def _detect_language(self, file_path: Path) -> str:
        """Detect language from file extension."""
        ext_map = {
            ".ts": "typescript",
            ".tsx": "typescript",
            ".js": "typescript",
            ".jsx": "typescript",
            ".mjs": "typescript",
            ".cjs": "typescript",
            ".py": "python",
            ".pyi": "python",
            ".rs": "rust",
            ".go": "go",
            ".java": "java",
            ".kt": "kotlin",
            ".swift": "swift",
            ".c": "c",
            ".cpp": "cpp",
            ".h": "c",
            ".hpp": "cpp",
        }
        return ext_map.get(file_path.suffix.lower(), "")

    def _find_project_root(self, file_path: Path) -> Path:
        """Find project root by looking for markers."""
        markers = [
            "package.json", "Cargo.toml", "pyproject.toml", "go.mod",
            "Package.swift", "build.gradle", "pom.xml", ".git"
        ]

        current = file_path.parent if file_path.is_file() else file_path
        while current != current.parent:
            for marker in markers:
                if (current / marker).exists():
                    return current
            current = current.parent

        return file_path.parent

    async def _idle_cleanup_loop(self):
        """Periodically check for idle servers to evict."""
        import time

        while self._running:
            await asyncio.sleep(60)  # Check every minute

            timeout = self.config.idle_timeout_minutes * 60
            now = time.time()

            idle_servers = [
                key for key, server in self.server_manager.servers.items()
                if (now - server.last_query) > timeout
            ]

            for key in idle_servers:
                print(f"Evicting idle server: {key}")
                await self.server_manager._stop_server(key)

    def _is_running(self) -> bool:
        """Check if daemon is already running."""
        if not self._pid_file.exists():
            return False

        try:
            pid = int(self._pid_file.read_text().strip())
            os.kill(pid, 0)  # Check if process exists
            return True
        except (ValueError, ProcessLookupError, PermissionError):
            self._pid_file.unlink(missing_ok=True)
            return False

    def _write_pid(self):
        """Write PID file."""
        self._pid_file.parent.mkdir(parents=True, exist_ok=True)
        self._pid_file.write_text(str(os.getpid()))


# CLI entry points
def main():
    """CLI for daemon management."""
    import click

    @click.group()
    def cli():
        """LSP Pool daemon management."""
        pass

    @cli.command()
    @click.option("--foreground", "-f", is_flag=True, help="Run in foreground")
    def start(foreground):
        """Start the daemon."""
        daemon = LSPPoolDaemon()

        if foreground:
            asyncio.run(daemon.start())
        else:
            # Fork to background
            pid = os.fork()
            if pid > 0:
                print(f"Daemon started (PID: {pid})")
                sys.exit(0)

            os.setsid()
            asyncio.run(daemon.start())

    @cli.command()
    def stop():
        """Stop the daemon."""
        socket_path = SOCKET_PATH

        if not socket_path.exists():
            print("Daemon is not running.")
            return

        async def send_stop():
            try:
                reader, writer = await asyncio.open_unix_connection(str(socket_path))
                writer.write(json.dumps({"type": "stop"}).encode())
                await writer.drain()
                writer.close()
                await writer.wait_closed()
                print("Stop signal sent.")
            except Exception as e:
                print(f"Could not connect to daemon: {e}")

        asyncio.run(send_stop())

    @cli.command()
    @click.option("--json", "as_json", is_flag=True, help="Output as JSON")
    def status(as_json):
        """Show daemon status."""
        socket_path = SOCKET_PATH

        if not socket_path.exists():
            if as_json:
                click.echo(json.dumps({"running": False}))
            else:
                click.echo("Daemon is not running.")
            return

        async def get_status():
            try:
                reader, writer = await asyncio.open_unix_connection(str(socket_path))
                writer.write(json.dumps({"type": "status"}).encode())
                await writer.drain()

                data = await reader.read(65536)
                response = json.loads(data.decode())

                writer.close()
                await writer.wait_closed()

                if as_json:
                    click.echo(json.dumps(response, indent=2))
                else:
                    click.echo("LSP Pool Daemon Status")
                    click.echo("=" * 40)
                    click.echo(f"Max servers: {response['config']['max_servers']}")
                    click.echo(f"Memory limit: {response['config']['memory_limit_mb']}MB")
                    click.echo()

                    servers = response.get("servers", [])
                    if servers:
                        click.echo("Running servers:")
                        for s in servers:
                            click.echo(f"  {s['language']:12} {s['status']:12} {s['memory_mb']:>6.1f}MB")
                    else:
                        click.echo("No servers running.")

            except Exception as e:
                if as_json:
                    click.echo(json.dumps({"running": False, "error": str(e)}))
                else:
                    click.echo(f"Could not connect to daemon: {e}")

        asyncio.run(get_status())

    cli()


if __name__ == "__main__":
    main()
