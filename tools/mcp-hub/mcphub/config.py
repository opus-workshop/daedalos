"""Configuration management for MCP Hub."""

import os
from pathlib import Path
from typing import Any, Dict, Optional
import yaml


# Default paths
CONFIG_DIR = Path(os.environ.get("XDG_CONFIG_HOME", Path.home() / ".config")) / "daedalos" / "mcp-hub"
DATA_DIR = Path(os.environ.get("XDG_DATA_HOME", Path.home() / ".local" / "share")) / "daedalos" / "mcp-hub"
SOCKET_PATH = Path(os.environ.get("MCPHUB_SOCKET", "/run/daedalos/mcp-hub.sock"))

# Fallback socket path for non-root users
if not SOCKET_PATH.parent.exists():
    SOCKET_PATH = Path.home() / ".local" / "run" / "daedalos" / "mcp-hub.sock"

# Default configuration
DEFAULT_CONFIG = {
    "auto_start_servers": ["filesystem"],
    "max_servers": 10,
    "request_timeout": 30,
    "servers": {},
}


class Config:
    """Configuration for MCP Hub."""

    def __init__(self, config_path: Optional[Path] = None):
        self.config_path = config_path or CONFIG_DIR / "config.yaml"
        self._config: Dict[str, Any] = {}
        self._load()

    def _load(self):
        """Load configuration from file, falling back to defaults."""
        self._config = DEFAULT_CONFIG.copy()

        if self.config_path.exists():
            try:
                with open(self.config_path) as f:
                    user_config = yaml.safe_load(f) or {}
                self._merge(self._config, user_config)
            except Exception:
                pass

    def _merge(self, base: dict, override: dict):
        """Recursively merge override into base."""
        for key, value in override.items():
            if key in base and isinstance(base[key], dict) and isinstance(value, dict):
                self._merge(base[key], value)
            else:
                base[key] = value

    def save(self):
        """Save configuration to file."""
        self.config_path.parent.mkdir(parents=True, exist_ok=True)
        with open(self.config_path, "w") as f:
            yaml.dump(self._config, f, default_flow_style=False)

    def get(self, key: str, default: Any = None) -> Any:
        """Get a configuration value."""
        parts = key.split(".")
        value = self._config
        for part in parts:
            if isinstance(value, dict):
                value = value.get(part)
            else:
                return default
        return value if value is not None else default

    def set(self, key: str, value: Any):
        """Set a configuration value."""
        parts = key.split(".")
        config = self._config
        for part in parts[:-1]:
            config = config.setdefault(part, {})
        config[parts[-1]] = value

    @property
    def auto_start_servers(self) -> list:
        return self.get("auto_start_servers", [])

    @property
    def servers(self) -> Dict[str, Any]:
        return self.get("servers", {})

    @property
    def max_servers(self) -> int:
        return self.get("max_servers", 10)


# Singleton config
_config: Optional[Config] = None


def get_config() -> Config:
    """Get the global configuration instance."""
    global _config
    if _config is None:
        _config = Config()
    return _config


# CLI entry point
def main():
    """CLI for configuration management."""
    import click
    import json

    @click.group()
    def cli():
        """MCP Hub configuration."""
        pass

    @cli.command("get")
    @click.argument("key")
    def config_get(key):
        """Get a configuration value."""
        config = get_config()
        value = config.get(key)
        if value is None:
            click.echo(f"Key not found: {key}")
        elif isinstance(value, (dict, list)):
            click.echo(json.dumps(value, indent=2))
        else:
            click.echo(value)

    @cli.command("set")
    @click.argument("key")
    @click.argument("value")
    def config_set(key, value):
        """Set a configuration value."""
        config = get_config()
        # Try to parse as JSON, otherwise use as string
        try:
            value = json.loads(value)
        except json.JSONDecodeError:
            pass
        config.set(key, value)
        config.save()
        click.echo(f"Set {key} = {value}")

    @cli.command("show")
    def config_show():
        """Show all configuration."""
        config = get_config()
        click.echo(yaml.dump(config._config, default_flow_style=False))

    @cli.command("path")
    def config_path():
        """Show configuration file path."""
        config = get_config()
        click.echo(config.config_path)

    cli()


if __name__ == "__main__":
    main()
