"""Request router - sends requests to appropriate servers."""

import asyncio
import json
import sys
from pathlib import Path
from typing import Any, Dict, List, Optional

from .config import SOCKET_PATH


class Router:
    """Route MCP requests to the hub daemon."""

    def __init__(self, socket_path: Optional[Path] = None):
        self.socket_path = socket_path or SOCKET_PATH

    async def _send_request(self, request: dict, timeout: float = 30.0) -> dict:
        """Send request to hub daemon."""
        if not self.socket_path.exists():
            return {"success": False, "error": "Hub daemon not running"}

        try:
            reader, writer = await asyncio.open_unix_connection(str(self.socket_path))

            writer.write(json.dumps(request).encode())
            await writer.drain()

            data = await asyncio.wait_for(reader.read(65536), timeout=timeout)
            response = json.loads(data.decode())

            writer.close()
            await writer.wait_closed()

            return response

        except asyncio.TimeoutError:
            return {"success": False, "error": "Request timed out"}
        except Exception as e:
            return {"success": False, "error": str(e)}

    def _send_request_sync(self, request: dict, timeout: float = 30.0) -> dict:
        """Synchronous wrapper for _send_request."""
        return asyncio.run(self._send_request(request, timeout))

    async def call_tool(
        self,
        tool_name: str,
        arguments: Dict[str, Any],
        server: Optional[str] = None
    ) -> dict:
        """Call a tool, optionally on a specific server."""
        request = {
            "type": "call_tool",
            "tool": tool_name,
            "arguments": arguments
        }
        if server:
            request["server"] = server

        return await self._send_request(request)

    async def list_tools(self) -> List[dict]:
        """List all available tools from running servers."""
        request = {"type": "list_tools"}
        response = await self._send_request(request)
        return response.get("tools", [])

    async def list_resources(self) -> List[dict]:
        """List all available resources from running servers."""
        request = {"type": "list_resources"}
        response = await self._send_request(request)
        return response.get("resources", [])

    async def get_resource(self, uri: str, server: Optional[str] = None) -> dict:
        """Get a resource by URI."""
        request = {
            "type": "get_resource",
            "uri": uri
        }
        if server:
            request["server"] = server

        return await self._send_request(request)

    async def status(self) -> dict:
        """Get hub status."""
        request = {"type": "status"}
        return await self._send_request(request)


def parse_arguments(args: List[str]) -> Dict[str, Any]:
    """Parse command line arguments into a dictionary."""
    result = {}
    i = 0
    while i < len(args):
        arg = args[i]
        if arg.startswith("--"):
            key = arg[2:]
            if "=" in key:
                key, value = key.split("=", 1)
            elif i + 1 < len(args) and not args[i + 1].startswith("--"):
                i += 1
                value = args[i]
            else:
                value = "true"

            # Try to parse as JSON for complex values
            try:
                value = json.loads(value)
            except (json.JSONDecodeError, TypeError):
                pass

            result[key] = value
        i += 1

    return result


# CLI entry point
def main():
    """CLI for routing requests."""
    import click

    @click.group()
    def cli():
        """MCP request routing."""
        pass

    @cli.command("call")
    @click.argument("tool_name")
    @click.option("--server", "-s", help="Specific server to use")
    @click.option("--json", "as_json", is_flag=True, help="Output as JSON")
    @click.argument("args", nargs=-1)
    def call_tool(tool_name, server, as_json, args):
        """
        Call an MCP tool.

        \b
        Examples:
          mcp-hub call read_file --path /etc/hosts
          mcp-hub call search_files --path . --regex "TODO"
        """
        arguments = parse_arguments(list(args))

        async def run():
            router = Router()
            result = await router.call_tool(tool_name, arguments, server=server)

            if as_json:
                click.echo(json.dumps(result, indent=2))
            elif result.get("success"):
                data = result.get("result", {})
                if isinstance(data, dict):
                    content = data.get("content", [])
                    for item in content:
                        if isinstance(item, dict):
                            text = item.get("text", str(item))
                        else:
                            text = str(item)
                        click.echo(text)
                else:
                    click.echo(str(data))
            else:
                click.echo(f"Error: {result.get('error', 'Unknown error')}", err=True)
                sys.exit(1)

        asyncio.run(run())

    @cli.command("tools")
    @click.option("--json", "as_json", is_flag=True, help="Output as JSON")
    def list_tools(as_json):
        """List available tools from running servers."""
        async def run():
            router = Router()
            result = await router.status()

            if not result.get("success"):
                click.echo(f"Error: {result.get('error', 'Hub not running')}", err=True)
                sys.exit(1)

            tools = result.get("tools", [])
            if as_json:
                click.echo(json.dumps(tools, indent=2))
            elif not tools:
                click.echo("No tools available. Start the hub first.")
            else:
                for t in tools:
                    click.echo(f"{t.get('server', '?'):15} {t.get('name', '?')}")

        asyncio.run(run())

    @cli.command("resources")
    @click.option("--json", "as_json", is_flag=True, help="Output as JSON")
    def list_resources(as_json):
        """List available resources from running servers."""
        async def run():
            router = Router()
            resources = await router.list_resources()

            if as_json:
                click.echo(json.dumps(resources, indent=2))
            elif not resources:
                click.echo("No resources available.")
            else:
                for r in resources:
                    click.echo(f"{r.get('uri', '?')}")

        asyncio.run(run())

    cli()


if __name__ == "__main__":
    main()
