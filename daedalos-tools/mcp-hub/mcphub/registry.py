"""Server registry - catalog of known MCP servers."""

import json
import subprocess
import sys
from dataclasses import dataclass, field
from pathlib import Path
from typing import Any, Dict, List, Optional
import yaml

from .config import CONFIG_DIR, DATA_DIR


@dataclass
class ServerInfo:
    """Information about an MCP server."""
    name: str
    description: str
    command: List[str]
    args: List[str] = field(default_factory=list)
    env: Dict[str, str] = field(default_factory=dict)
    category: str = "general"
    tools: List[str] = field(default_factory=list)
    resources: List[str] = field(default_factory=list)
    requires_auth: bool = False
    auth_env_vars: List[str] = field(default_factory=list)
    source: str = "builtin"  # builtin, npm, github, local
    enabled: bool = True

    def to_dict(self) -> dict:
        return {
            "name": self.name,
            "description": self.description,
            "command": self.command,
            "args": self.args,
            "env": self.env,
            "category": self.category,
            "tools": self.tools,
            "resources": self.resources,
            "requires_auth": self.requires_auth,
            "auth_env_vars": self.auth_env_vars,
            "source": self.source,
            "enabled": self.enabled,
        }


# Built-in server definitions
BUILTIN_SERVERS = [
    ServerInfo(
        name="filesystem",
        description="File system operations (read, write, list, search)",
        command=["npx", "-y", "@modelcontextprotocol/server-filesystem"],
        args=["/"],
        category="core",
        tools=["read_file", "write_file", "list_directory", "create_directory",
               "move_file", "search_files", "get_file_info", "read_multiple_files"],
        source="npm",
    ),
    ServerInfo(
        name="github",
        description="GitHub operations (issues, PRs, code search)",
        command=["npx", "-y", "@modelcontextprotocol/server-github"],
        category="integrations",
        tools=["create_issue", "create_pull_request", "search_code",
               "list_commits", "get_file_contents", "push_files"],
        resources=["repo", "issue", "pull_request"],
        requires_auth=True,
        auth_env_vars=["GITHUB_TOKEN"],
        source="npm",
    ),
    ServerInfo(
        name="memory",
        description="Persistent memory for conversations",
        command=["npx", "-y", "@modelcontextprotocol/server-memory"],
        category="core",
        tools=["store", "retrieve", "list", "delete"],
        source="npm",
    ),
    ServerInfo(
        name="sqlite",
        description="SQLite database operations",
        command=["npx", "-y", "@modelcontextprotocol/server-sqlite"],
        category="data",
        tools=["read_query", "write_query", "create_table", "list_tables", "describe_table"],
        source="npm",
    ),
    ServerInfo(
        name="fetch",
        description="HTTP fetch operations",
        command=["npx", "-y", "@modelcontextprotocol/server-fetch"],
        category="network",
        tools=["fetch"],
        source="npm",
    ),
    ServerInfo(
        name="brave-search",
        description="Brave Search API",
        command=["npx", "-y", "@modelcontextprotocol/server-brave-search"],
        category="search",
        tools=["brave_web_search", "brave_local_search"],
        requires_auth=True,
        auth_env_vars=["BRAVE_API_KEY"],
        source="npm",
    ),
]


class ServerRegistry:
    """Registry of known MCP servers."""

    def __init__(self, registry_path: Optional[Path] = None):
        self.registry_path = registry_path or DATA_DIR / "registry"
        self.registry_path.mkdir(parents=True, exist_ok=True)
        self.servers: Dict[str, ServerInfo] = {}
        self._load_builtin()
        self._load_installed()

    def _load_builtin(self):
        """Load built-in server definitions."""
        for server in BUILTIN_SERVERS:
            self.servers[server.name] = server

    def _load_installed(self):
        """Load user-installed servers from registry."""
        installed_file = self.registry_path / "installed.yaml"
        if installed_file.exists():
            try:
                data = yaml.safe_load(installed_file.read_text()) or {}
                for name, info in data.items():
                    self.servers[name] = ServerInfo(**info)
            except Exception:
                pass

        # Load enabled/disabled state
        state_file = self.registry_path / "state.yaml"
        if state_file.exists():
            try:
                state = yaml.safe_load(state_file.read_text()) or {}
                for name, enabled in state.get("enabled", {}).items():
                    if name in self.servers:
                        self.servers[name].enabled = enabled
            except Exception:
                pass

    def _save_installed(self):
        """Save installed servers to registry."""
        installed = {
            name: server.to_dict()
            for name, server in self.servers.items()
            if server.source != "builtin"
        }
        installed_file = self.registry_path / "installed.yaml"
        installed_file.write_text(yaml.dump(installed, default_flow_style=False))

    def _save_state(self):
        """Save enabled/disabled state."""
        state = {
            "enabled": {
                name: server.enabled
                for name, server in self.servers.items()
            }
        }
        state_file = self.registry_path / "state.yaml"
        state_file.write_text(yaml.dump(state, default_flow_style=False))

    def list(self, category: Optional[str] = None, enabled_only: bool = False) -> List[ServerInfo]:
        """List servers, optionally filtered."""
        servers = list(self.servers.values())
        if category:
            servers = [s for s in servers if s.category == category]
        if enabled_only:
            servers = [s for s in servers if s.enabled]
        return servers

    def search(self, query: str) -> List[ServerInfo]:
        """Search servers by name, tool, or description."""
        query = query.lower()
        results = []
        for server in self.servers.values():
            if query in server.name.lower():
                results.append(server)
            elif any(query in tool.lower() for tool in server.tools):
                results.append(server)
            elif query in server.description.lower():
                results.append(server)
        return results

    def get(self, name: str) -> Optional[ServerInfo]:
        """Get server by name."""
        return self.servers.get(name)

    def enable(self, name: str) -> bool:
        """Enable a server."""
        if name in self.servers:
            self.servers[name].enabled = True
            self._save_state()
            return True
        return False

    def disable(self, name: str) -> bool:
        """Disable a server."""
        if name in self.servers:
            self.servers[name].enabled = False
            self._save_state()
            return True
        return False

    def install(self, source: str) -> Optional[ServerInfo]:
        """Install a server from source."""
        if source.startswith("npm:"):
            return self._install_npm(source[4:])
        elif source.startswith("github:"):
            return self._install_github(source[7:])
        elif source in self.servers:
            # Enable a built-in server
            self.enable(source)
            return self.servers[source]
        else:
            # Try as npm package
            return self._install_npm(source)

    def _install_npm(self, package: str) -> Optional[ServerInfo]:
        """Install from npm."""
        try:
            # Install globally
            subprocess.run(
                ["npm", "install", "-g", package],
                check=True,
                capture_output=True
            )

            # Create server info (user will need to configure command)
            name = package.split("/")[-1].replace("server-", "")
            server = ServerInfo(
                name=name,
                description=f"Installed from npm: {package}",
                command=["npx", "-y", package],
                source="npm",
            )
            self.servers[name] = server
            self._save_installed()
            return server

        except subprocess.CalledProcessError as e:
            print(f"Failed to install {package}: {e.stderr.decode()}", file=sys.stderr)
            return None

    def _install_github(self, repo: str) -> Optional[ServerInfo]:
        """Install from GitHub."""
        # Clone and setup - simplified implementation
        name = repo.split("/")[-1]
        clone_path = DATA_DIR / "servers" / name

        try:
            subprocess.run(
                ["git", "clone", f"https://github.com/{repo}.git", str(clone_path)],
                check=True,
                capture_output=True
            )

            server = ServerInfo(
                name=name,
                description=f"Installed from GitHub: {repo}",
                command=["node", str(clone_path / "index.js")],
                source="github",
            )
            self.servers[name] = server
            self._save_installed()
            return server

        except subprocess.CalledProcessError as e:
            print(f"Failed to clone {repo}: {e.stderr.decode()}", file=sys.stderr)
            return None

    def uninstall(self, name: str) -> bool:
        """Uninstall a server."""
        if name not in self.servers:
            return False

        server = self.servers[name]
        if server.source == "builtin":
            # Just disable built-in servers
            self.disable(name)
            return True

        del self.servers[name]
        self._save_installed()
        return True

    def get_tools(self) -> List[Dict[str, Any]]:
        """Get all tools from all enabled servers."""
        tools = []
        for server in self.servers.values():
            if server.enabled:
                for tool_name in server.tools:
                    tools.append({
                        "server": server.name,
                        "name": tool_name,
                        "description": f"{tool_name} from {server.name}",
                    })
        return tools


# CLI entry point
def main():
    """CLI for registry management."""
    import click

    @click.group()
    def cli():
        """MCP server registry."""
        pass

    @cli.command("list")
    @click.option("--category", "-c", help="Filter by category")
    @click.option("--enabled", is_flag=True, help="Show only enabled servers")
    @click.option("--json", "as_json", is_flag=True, help="Output as JSON")
    def list_servers(category, enabled, as_json):
        """List available servers."""
        registry = ServerRegistry()
        servers = registry.list(category=category, enabled_only=enabled)

        if as_json:
            click.echo(json.dumps([s.to_dict() for s in servers], indent=2))
        else:
            for s in servers:
                status = "[ON]" if s.enabled else "[--]"
                click.echo(f"{status} {s.name:15} {s.category:12} {s.description}")

    @cli.command("search")
    @click.argument("query")
    def search_servers(query):
        """Search for servers."""
        registry = ServerRegistry()
        servers = registry.search(query)

        if not servers:
            click.echo("No servers found.")
        else:
            for s in servers:
                click.echo(f"{s.name:15} {s.description}")

    @cli.command("install")
    @click.argument("source")
    def install_server(source):
        """Install a server."""
        registry = ServerRegistry()
        server = registry.install(source)

        if server:
            click.echo(f"Installed: {server.name}")
        else:
            click.echo("Installation failed.", err=True)
            sys.exit(1)

    @cli.command("uninstall")
    @click.argument("name")
    def uninstall_server(name):
        """Uninstall a server."""
        registry = ServerRegistry()
        if registry.uninstall(name):
            click.echo(f"Uninstalled: {name}")
        else:
            click.echo(f"Server not found: {name}", err=True)
            sys.exit(1)

    @cli.command("enable")
    @click.argument("name")
    def enable_server(name):
        """Enable a server."""
        registry = ServerRegistry()
        if registry.enable(name):
            click.echo(f"Enabled: {name}")
        else:
            click.echo(f"Server not found: {name}", err=True)
            sys.exit(1)

    @cli.command("disable")
    @click.argument("name")
    def disable_server(name):
        """Disable a server."""
        registry = ServerRegistry()
        if registry.disable(name):
            click.echo(f"Disabled: {name}")
        else:
            click.echo(f"Server not found: {name}", err=True)
            sys.exit(1)

    @cli.command("tools")
    @click.option("--json", "as_json", is_flag=True, help="Output as JSON")
    def list_tools(as_json):
        """List all available tools."""
        registry = ServerRegistry()
        tools = registry.get_tools()

        if as_json:
            click.echo(json.dumps(tools, indent=2))
        else:
            for t in tools:
                click.echo(f"{t['server']:15} {t['name']}")

    @cli.command("info")
    @click.argument("name")
    def server_info(name):
        """Show server information."""
        registry = ServerRegistry()
        server = registry.get(name)

        if not server:
            click.echo(f"Server not found: {name}", err=True)
            sys.exit(1)

        click.echo(f"Name:        {server.name}")
        click.echo(f"Description: {server.description}")
        click.echo(f"Category:    {server.category}")
        click.echo(f"Source:      {server.source}")
        click.echo(f"Enabled:     {server.enabled}")
        click.echo(f"Command:     {' '.join(server.command)}")
        click.echo(f"Tools:       {', '.join(server.tools)}")
        if server.requires_auth:
            click.echo(f"Auth vars:   {', '.join(server.auth_env_vars)}")

    cli()


if __name__ == "__main__":
    main()
