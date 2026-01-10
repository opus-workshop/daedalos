"""Configuration management for LSP Pool."""

import os
from pathlib import Path
from typing import Any, Dict, Optional
import yaml


# Default paths
CONFIG_DIR = Path(os.environ.get("XDG_CONFIG_HOME", Path.home() / ".config")) / "daedalos" / "lsp-pool"
DATA_DIR = Path(os.environ.get("XDG_DATA_HOME", Path.home() / ".local" / "share")) / "daedalos" / "lsp-pool"
SOCKET_PATH = Path(os.environ.get("LSPPOOL_SOCKET", "/run/daedalos/lsp-pool.sock"))

# Default configuration
DEFAULT_CONFIG = {
    "max_servers": 5,
    "memory_limit_mb": 2048,
    "idle_timeout_minutes": 30,
    "warmup_on_start": True,
    "health_check_interval": 60,
    "servers": {
        "typescript": {
            "command": ["typescript-language-server", "--stdio"],
            "extensions": [".ts", ".tsx", ".js", ".jsx", ".mjs", ".cjs"],
            "memory_estimate_mb": 400,
        },
        "python": {
            "command": ["pyright-langserver", "--stdio"],
            "extensions": [".py", ".pyi"],
            "memory_estimate_mb": 300,
            "alternatives": ["pylsp", "jedi-language-server"],
        },
        "rust": {
            "command": ["rust-analyzer"],
            "extensions": [".rs"],
            "memory_estimate_mb": 500,
        },
        "go": {
            "command": ["gopls", "serve"],
            "extensions": [".go"],
            "memory_estimate_mb": 200,
        },
        "c": {
            "command": ["clangd"],
            "extensions": [".c", ".h"],
            "memory_estimate_mb": 400,
        },
        "cpp": {
            "command": ["clangd"],
            "extensions": [".cpp", ".hpp", ".cc", ".hh", ".cxx"],
            "memory_estimate_mb": 500,
        },
        "java": {
            "command": ["jdtls"],
            "extensions": [".java"],
            "memory_estimate_mb": 800,
        },
        "kotlin": {
            "command": ["kotlin-language-server"],
            "extensions": [".kt", ".kts"],
            "memory_estimate_mb": 600,
        },
        "swift": {
            "command": ["sourcekit-lsp"],
            "extensions": [".swift"],
            "memory_estimate_mb": 400,
        },
        "lua": {
            "command": ["lua-language-server"],
            "extensions": [".lua"],
            "memory_estimate_mb": 150,
        },
        "ruby": {
            "command": ["solargraph", "stdio"],
            "extensions": [".rb", ".rake"],
            "memory_estimate_mb": 300,
        },
        "elixir": {
            "command": ["elixir-ls"],
            "extensions": [".ex", ".exs"],
            "memory_estimate_mb": 400,
        },
        "zig": {
            "command": ["zls"],
            "extensions": [".zig"],
            "memory_estimate_mb": 200,
        },
        "ocaml": {
            "command": ["ocamllsp"],
            "extensions": [".ml", ".mli"],
            "memory_estimate_mb": 300,
        },
        "haskell": {
            "command": ["haskell-language-server-wrapper", "--lsp"],
            "extensions": [".hs"],
            "memory_estimate_mb": 600,
        },
        "bash": {
            "command": ["bash-language-server", "start"],
            "extensions": [".sh", ".bash"],
            "memory_estimate_mb": 100,
        },
        "yaml": {
            "command": ["yaml-language-server", "--stdio"],
            "extensions": [".yaml", ".yml"],
            "memory_estimate_mb": 100,
        },
        "json": {
            "command": ["vscode-json-language-server", "--stdio"],
            "extensions": [".json", ".jsonc"],
            "memory_estimate_mb": 100,
        },
        "html": {
            "command": ["vscode-html-language-server", "--stdio"],
            "extensions": [".html", ".htm"],
            "memory_estimate_mb": 100,
        },
        "css": {
            "command": ["vscode-css-language-server", "--stdio"],
            "extensions": [".css", ".scss", ".less"],
            "memory_estimate_mb": 100,
        },
        "dockerfile": {
            "command": ["docker-langserver", "--stdio"],
            "extensions": ["Dockerfile"],
            "memory_estimate_mb": 100,
        },
        "nix": {
            "command": ["nil"],
            "extensions": [".nix"],
            "memory_estimate_mb": 200,
        },
    },
}


class Config:
    """Configuration for LSP Pool."""

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
    def max_servers(self) -> int:
        return self.get("max_servers", 5)

    @property
    def memory_limit_mb(self) -> int:
        return self.get("memory_limit_mb", 2048)

    @property
    def idle_timeout_minutes(self) -> int:
        return self.get("idle_timeout_minutes", 30)

    @property
    def servers(self) -> Dict[str, Any]:
        return self.get("servers", {})

    def get_server_config(self, language: str) -> Optional[Dict[str, Any]]:
        """Get configuration for a specific language server."""
        return self.servers.get(language)


# Singleton config
_config: Optional[Config] = None


def get_config() -> Config:
    """Get the global configuration instance."""
    global _config
    if _config is None:
        _config = Config()
    return _config
