"""MCP Hub Daemon - manages server pool and routes requests."""

import asyncio
import json
import os
import signal
import sys
import time
from dataclasses import dataclass, field
from pathlib import Path
from typing import Any, Dict, List, Optional

from .config import Config, get_config, DATA_DIR, SOCKET_PATH
from .registry import ServerRegistry, ServerInfo


@dataclass
class ServerProcess:
    """Wrapper for a managed MCP server process."""
    name: str
    info: ServerInfo
    process: Optional[asyncio.subprocess.Process] = None
    pid: int = 0
    status: str = "stopped"  # stopped, starting, running, error, unhealthy
    started_at: float = 0
    tools: List[dict] = field(default_factory=list)
    resources: List[dict] = field(default_factory=list)
    prompts: List[dict] = field(default_factory=list)
    request_id: int = 0
    last_health_check: float = 0
    health_failures: int = 0
    restart_count: int = 0
    stderr_log: List[str] = field(default_factory=list)

    def next_request_id(self) -> int:
        self.request_id += 1
        return self.request_id

    def add_log(self, line: str):
        """Add a log line, keeping last 100 lines."""
        self.stderr_log.append(line)
        if len(self.stderr_log) > 100:
            self.stderr_log = self.stderr_log[-100:]


class MCPHubDaemon:
    """Central hub daemon for MCP servers."""

    HEALTH_CHECK_INTERVAL = 30  # seconds
    MAX_HEALTH_FAILURES = 3
    MAX_RESTART_ATTEMPTS = 3

    def __init__(self, config: Optional[Config] = None, socket_path: Optional[Path] = None):
        self.config = config or get_config()
        self.socket_path = socket_path or SOCKET_PATH
        self.registry = ServerRegistry()
        self.servers: Dict[str, ServerProcess] = {}
        self._server: Optional[asyncio.Server] = None
        self._running = False
        self._pid_file = DATA_DIR / "daemon.pid"
        self._health_task: Optional[asyncio.Task] = None
        self._stderr_tasks: Dict[str, asyncio.Task] = {}

    async def start(self):
        """Start the hub daemon."""
        if self._is_running():
            print("Daemon is already running.")
            return

        self._running = True
        self._write_pid()

        # Setup signal handlers
        loop = asyncio.get_event_loop()
        for sig in (signal.SIGTERM, signal.SIGINT):
            loop.add_signal_handler(sig, lambda: asyncio.create_task(self.stop()))

        # Start auto-start servers
        for server_name in self.config.auto_start_servers:
            server_info = self.registry.get(server_name)
            if server_info and server_info.enabled:
                try:
                    await self._start_server(server_info)
                except Exception as e:
                    print(f"Failed to start {server_name}: {e}", file=sys.stderr)

        # Ensure socket directory exists
        self.socket_path.parent.mkdir(parents=True, exist_ok=True)

        # Remove stale socket
        if self.socket_path.exists():
            self.socket_path.unlink()

        # Start health check loop
        self._health_task = asyncio.create_task(self._health_check_loop())

        # Start listening
        print(f"MCP Hub daemon starting on {self.socket_path}")
        self._server = await asyncio.start_unix_server(
            self._handle_client,
            path=str(self.socket_path)
        )

        async with self._server:
            await self._server.serve_forever()

    async def stop(self):
        """Stop the daemon and all managed servers."""
        print("Stopping MCP Hub daemon...")
        self._running = False

        # Cancel health check
        if self._health_task:
            self._health_task.cancel()
            try:
                await self._health_task
            except asyncio.CancelledError:
                pass

        # Cancel stderr collectors
        for task in self._stderr_tasks.values():
            task.cancel()

        # Stop all servers
        for name in list(self.servers.keys()):
            await self._stop_server(name)

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

    async def _health_check_loop(self):
        """Periodically check server health."""
        while self._running:
            try:
                await asyncio.sleep(self.HEALTH_CHECK_INTERVAL)
                await self._check_all_servers()
            except asyncio.CancelledError:
                break
            except Exception as e:
                print(f"Health check error: {e}", file=sys.stderr)

    async def _check_all_servers(self):
        """Check health of all running servers."""
        for name, server in list(self.servers.items()):
            if server.status not in ("running", "unhealthy"):
                continue

            healthy = await self._health_check(server)
            server.last_health_check = time.time()

            if healthy:
                server.health_failures = 0
                if server.status == "unhealthy":
                    server.status = "running"
                    print(f"Server {name} recovered")
            else:
                server.health_failures += 1
                if server.health_failures >= self.MAX_HEALTH_FAILURES:
                    server.status = "unhealthy"
                    print(f"Server {name} unhealthy ({server.health_failures} failures)")

                    # Attempt restart if under limit
                    if server.restart_count < self.MAX_RESTART_ATTEMPTS:
                        print(f"Attempting restart of {name}...")
                        await self._restart_server(name)

    async def _health_check(self, server: ServerProcess) -> bool:
        """Check if a server is healthy by sending a ping."""
        if not server.process or server.process.returncode is not None:
            return False

        try:
            # Send a simple tools/list as health check
            request = {
                "jsonrpc": "2.0",
                "id": server.next_request_id(),
                "method": "tools/list"
            }
            response = await self._send_mcp_request(server, request, timeout=5.0)
            return response is not None and "error" not in response
        except Exception:
            return False

    async def _restart_server(self, name: str):
        """Restart a server."""
        if name not in self.servers:
            return False

        server = self.servers[name]
        info = server.info
        restart_count = server.restart_count + 1

        await self._stop_server(name)
        success = await self._start_server(info)

        if success and name in self.servers:
            self.servers[name].restart_count = restart_count
            print(f"Server {name} restarted (attempt {restart_count})")

        return success

    async def _collect_stderr(self, server: ServerProcess):
        """Collect stderr output from server process."""
        if not server.process or not server.process.stderr:
            return

        try:
            while True:
                line = await server.process.stderr.readline()
                if not line:
                    break
                server.add_log(line.decode().rstrip())
        except asyncio.CancelledError:
            pass
        except Exception:
            pass

    async def _start_server(self, info: ServerInfo) -> bool:
        """Start an MCP server."""
        if info.name in self.servers and self.servers[info.name].status == "running":
            return True

        # Check auth requirements
        if info.requires_auth:
            for var in info.auth_env_vars:
                if var not in os.environ:
                    print(f"Warning: {info.name} requires {var} environment variable", file=sys.stderr)

        try:
            # Build command
            command = info.command + info.args
            env = {**os.environ, **info.env}

            # Start server process
            process = await asyncio.create_subprocess_exec(
                *command,
                stdin=asyncio.subprocess.PIPE,
                stdout=asyncio.subprocess.PIPE,
                stderr=asyncio.subprocess.PIPE,
                env=env,
            )

            server = ServerProcess(
                name=info.name,
                info=info,
                process=process,
                pid=process.pid,
                status="starting",
                started_at=time.time(),
            )
            self.servers[info.name] = server

            # Start stderr collector
            self._stderr_tasks[info.name] = asyncio.create_task(
                self._collect_stderr(server)
            )

            # Initialize MCP connection
            success = await self._initialize_server(server)
            if success:
                server.status = "running"
                print(f"Server started: {info.name}")
                return True
            else:
                await self._stop_server(info.name)
                return False

        except Exception as e:
            print(f"Failed to start server {info.name}: {e}", file=sys.stderr)
            return False

    async def _stop_server(self, name: str):
        """Stop an MCP server."""
        if name not in self.servers:
            return

        # Cancel stderr collector
        if name in self._stderr_tasks:
            self._stderr_tasks[name].cancel()
            del self._stderr_tasks[name]

        server = self.servers[name]
        if server.process:
            try:
                server.process.terminate()
                await asyncio.wait_for(server.process.wait(), timeout=5.0)
            except asyncio.TimeoutError:
                server.process.kill()
                await server.process.wait()
            except Exception:
                pass

        del self.servers[name]
        print(f"Server stopped: {name}")

    async def warm_servers(self, server_names: List[str]) -> Dict[str, bool]:
        """Pre-start specified servers."""
        results = {}
        for name in server_names:
            info = self.registry.get(name)
            if not info:
                results[name] = False
                continue
            if name in self.servers and self.servers[name].status == "running":
                results[name] = True
                continue
            results[name] = await self._start_server(info)
        return results

    def get_server_logs(self, name: str, lines: int = 50) -> List[str]:
        """Get recent log lines for a server."""
        if name not in self.servers:
            return []
        return self.servers[name].stderr_log[-lines:]

    async def _initialize_server(self, server: ServerProcess) -> bool:
        """Initialize MCP connection with server."""
        try:
            # Send initialize request
            request = {
                "jsonrpc": "2.0",
                "id": server.next_request_id(),
                "method": "initialize",
                "params": {
                    "protocolVersion": "0.1.0",
                    "capabilities": {},
                    "clientInfo": {
                        "name": "mcp-hub",
                        "version": "1.0.0"
                    }
                }
            }

            response = await self._send_mcp_request(server, request)
            if not response or "error" in response:
                return False

            # Store capabilities
            result = response.get("result", {})
            capabilities = result.get("capabilities", {})

            # Send initialized notification
            notification = {
                "jsonrpc": "2.0",
                "method": "notifications/initialized"
            }
            await self._send_mcp_notification(server, notification)

            # Get tools if server supports them
            if "tools" in capabilities:
                tools_response = await self._send_mcp_request(server, {
                    "jsonrpc": "2.0",
                    "id": server.next_request_id(),
                    "method": "tools/list"
                })
                if tools_response and "result" in tools_response:
                    server.tools = tools_response["result"].get("tools", [])

            # Get resources if server supports them
            if "resources" in capabilities:
                resources_response = await self._send_mcp_request(server, {
                    "jsonrpc": "2.0",
                    "id": server.next_request_id(),
                    "method": "resources/list"
                })
                if resources_response and "result" in resources_response:
                    server.resources = resources_response["result"].get("resources", [])

            return True

        except Exception as e:
            print(f"Initialize failed for {server.name}: {e}", file=sys.stderr)
            return False

    async def _send_mcp_request(self, server: ServerProcess, request: dict, timeout: float = 30.0) -> Optional[dict]:
        """Send MCP request to server and wait for response."""
        if not server.process or not server.process.stdin or not server.process.stdout:
            return None

        try:
            # Write request (line-delimited JSON)
            message = json.dumps(request) + "\n"
            server.process.stdin.write(message.encode())
            await server.process.stdin.drain()

            # Read response
            response_line = await asyncio.wait_for(
                server.process.stdout.readline(),
                timeout=timeout
            )

            if not response_line:
                return None

            return json.loads(response_line.decode())

        except asyncio.TimeoutError:
            return None
        except Exception as e:
            print(f"Request failed: {e}", file=sys.stderr)
            return None

    async def _send_mcp_notification(self, server: ServerProcess, notification: dict):
        """Send MCP notification to server (no response expected)."""
        if not server.process or not server.process.stdin:
            return

        try:
            message = json.dumps(notification) + "\n"
            server.process.stdin.write(message.encode())
            await server.process.stdin.drain()
        except Exception:
            pass

    async def _handle_client(self, reader: asyncio.StreamReader, writer: asyncio.StreamWriter):
        """Handle incoming client connection."""
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
            return self._get_status()

        elif req_type == "call_tool":
            return await self._route_tool_call(request)

        elif req_type == "list_tools":
            return {"success": True, "tools": self._get_all_tools()}

        elif req_type == "list_resources":
            return {"success": True, "resources": self._get_all_resources()}

        elif req_type == "start_server":
            server_name = request.get("server")
            info = self.registry.get(server_name)
            if not info:
                return {"success": False, "error": f"Unknown server: {server_name}"}
            success = await self._start_server(info)
            return {"success": success}

        elif req_type == "stop_server":
            server_name = request.get("server")
            await self._stop_server(server_name)
            return {"success": True}

        elif req_type == "restart_server":
            server_name = request.get("server")
            success = await self._restart_server(server_name)
            return {"success": success}

        elif req_type == "warm":
            server_names = request.get("servers", [])
            results = await self.warm_servers(server_names)
            return {"success": True, "results": results}

        elif req_type == "logs":
            server_name = request.get("server")
            lines = request.get("lines", 50)
            logs = self.get_server_logs(server_name, lines)
            return {"success": True, "logs": logs}

        elif req_type == "reload":
            # Reload config and registry
            self.config = get_config()
            self.registry = ServerRegistry()
            return {"success": True, "message": "Configuration reloaded"}

        elif req_type == "stop":
            asyncio.create_task(self.stop())
            return {"success": True, "message": "Daemon stopping"}

        else:
            return {"success": False, "error": f"Unknown request type: {req_type}"}

    async def _route_tool_call(self, request: dict) -> dict:
        """Route a tool call to the appropriate server."""
        tool_name = request.get("tool")
        arguments = request.get("arguments", {})
        target_server = request.get("server")

        # Find server that provides this tool
        server = None
        if target_server:
            server = self.servers.get(target_server)
        else:
            for s in self.servers.values():
                for tool in s.tools:
                    if tool.get("name") == tool_name:
                        server = s
                        break
                if server:
                    break

        if not server:
            return {"success": False, "error": f"No server provides tool: {tool_name}"}

        # Call the tool
        mcp_request = {
            "jsonrpc": "2.0",
            "id": server.next_request_id(),
            "method": "tools/call",
            "params": {
                "name": tool_name,
                "arguments": arguments
            }
        }

        response = await self._send_mcp_request(server, mcp_request)
        if not response:
            return {"success": False, "error": "Tool call failed"}

        if "error" in response:
            return {"success": False, "error": response["error"]}

        return {"success": True, "result": response.get("result", {})}

    def _get_status(self) -> dict:
        """Get hub status."""
        return {
            "success": True,
            "servers": [
                {
                    "name": s.name,
                    "status": s.status,
                    "pid": s.pid,
                    "uptime": int(time.time() - s.started_at) if s.started_at else 0,
                    "tools": len(s.tools),
                    "resources": len(s.resources),
                    "prompts": len(s.prompts),
                    "health_failures": s.health_failures,
                    "restart_count": s.restart_count,
                    "last_health_check": int(time.time() - s.last_health_check) if s.last_health_check else None,
                }
                for s in self.servers.values()
            ],
            "tools": self._get_all_tools(),
            "config": {
                "max_servers": self.config.max_servers,
                "auto_start": self.config.auto_start_servers,
            },
        }

    def _get_all_tools(self) -> List[dict]:
        """Get all tools from all running servers."""
        tools = []
        for server in self.servers.values():
            for tool in server.tools:
                tools.append({
                    "server": server.name,
                    "name": tool.get("name"),
                    "description": tool.get("description", ""),
                })
        return tools

    def _get_all_resources(self) -> List[dict]:
        """Get all resources from all running servers."""
        resources = []
        for server in self.servers.values():
            for resource in server.resources:
                resources.append({
                    "server": server.name,
                    "uri": resource.get("uri"),
                    "name": resource.get("name", ""),
                })
        return resources

    def _is_running(self) -> bool:
        """Check if daemon is already running."""
        if not self._pid_file.exists():
            return False

        try:
            pid = int(self._pid_file.read_text().strip())
            os.kill(pid, 0)
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
        """MCP Hub daemon management."""
        pass

    @cli.command()
    @click.option("--foreground", "-f", is_flag=True, help="Run in foreground")
    def start(foreground):
        """Start the daemon."""
        daemon = MCPHubDaemon()

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
                    click.echo("MCP Hub Daemon Status")
                    click.echo("=" * 40)

                    servers = response.get("servers", [])
                    if servers:
                        click.echo("Running servers:")
                        for s in servers:
                            health = ""
                            if s.get('health_failures', 0) > 0:
                                health = f" (health: {s['health_failures']} failures)"
                            click.echo(f"  {s['name']:15} {s['status']:10} tools:{s['tools']}{health}")
                    else:
                        click.echo("No servers running.")

                    tools = response.get("tools", [])
                    if tools:
                        click.echo(f"\nAvailable tools: {len(tools)}")

            except Exception as e:
                if as_json:
                    click.echo(json.dumps({"running": False, "error": str(e)}))
                else:
                    click.echo(f"Could not connect to daemon: {e}")

        asyncio.run(get_status())

    @cli.command()
    @click.argument("servers", nargs=-1, required=True)
    def warm(servers):
        """Pre-start servers for fast response."""
        socket_path = SOCKET_PATH

        if not socket_path.exists():
            click.echo("Daemon is not running. Start it first with: mcp-hub start")
            sys.exit(1)

        async def do_warm():
            try:
                reader, writer = await asyncio.open_unix_connection(str(socket_path))
                request = {"type": "warm", "servers": list(servers)}
                writer.write(json.dumps(request).encode())
                await writer.drain()

                data = await reader.read(65536)
                response = json.loads(data.decode())

                writer.close()
                await writer.wait_closed()

                results = response.get("results", {})
                for name, success in results.items():
                    status = "started" if success else "failed"
                    click.echo(f"  {name}: {status}")

            except Exception as e:
                click.echo(f"Error: {e}", err=True)
                sys.exit(1)

        asyncio.run(do_warm())

    @cli.command()
    @click.argument("server")
    @click.option("--lines", "-n", default=50, help="Number of lines to show")
    @click.option("--follow", "-f", is_flag=True, help="Follow log output")
    def logs(server, lines, follow):
        """View server logs."""
        socket_path = SOCKET_PATH

        if not socket_path.exists():
            click.echo("Daemon is not running.")
            sys.exit(1)

        async def get_logs():
            try:
                reader, writer = await asyncio.open_unix_connection(str(socket_path))
                request = {"type": "logs", "server": server, "lines": lines}
                writer.write(json.dumps(request).encode())
                await writer.drain()

                data = await reader.read(65536)
                response = json.loads(data.decode())

                writer.close()
                await writer.wait_closed()

                logs = response.get("logs", [])
                if not logs:
                    click.echo(f"No logs for {server}")
                else:
                    for line in logs:
                        click.echo(line)

            except Exception as e:
                click.echo(f"Error: {e}", err=True)
                sys.exit(1)

        asyncio.run(get_logs())

        if follow:
            click.echo("(follow mode not yet implemented)")

    @cli.command()
    @click.argument("server")
    def restart(server):
        """Restart a server."""
        socket_path = SOCKET_PATH

        if not socket_path.exists():
            click.echo("Daemon is not running.")
            sys.exit(1)

        async def do_restart():
            try:
                reader, writer = await asyncio.open_unix_connection(str(socket_path))
                request = {"type": "restart_server", "server": server}
                writer.write(json.dumps(request).encode())
                await writer.drain()

                data = await reader.read(65536)
                response = json.loads(data.decode())

                writer.close()
                await writer.wait_closed()

                if response.get("success"):
                    click.echo(f"Restarted: {server}")
                else:
                    click.echo(f"Failed to restart: {server}", err=True)

            except Exception as e:
                click.echo(f"Error: {e}", err=True)
                sys.exit(1)

        asyncio.run(do_restart())

    @cli.command()
    def reload():
        """Reload configuration without restarting."""
        socket_path = SOCKET_PATH

        if not socket_path.exists():
            click.echo("Daemon is not running.")
            sys.exit(1)

        async def do_reload():
            try:
                reader, writer = await asyncio.open_unix_connection(str(socket_path))
                request = {"type": "reload"}
                writer.write(json.dumps(request).encode())
                await writer.drain()

                data = await reader.read(65536)
                response = json.loads(data.decode())

                writer.close()
                await writer.wait_closed()

                if response.get("success"):
                    click.echo("Configuration reloaded")
                else:
                    click.echo(f"Failed: {response.get('error')}", err=True)

            except Exception as e:
                click.echo(f"Error: {e}", err=True)
                sys.exit(1)

        asyncio.run(do_reload())

    cli()


if __name__ == "__main__":
    main()
