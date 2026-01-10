"""Daedalos Observe - Real-time TUI dashboard."""

import asyncio
import json
import os
import subprocess
from datetime import datetime
from pathlib import Path
from typing import Optional

from textual.app import App, ComposeResult
from textual.containers import Container, Horizontal, Vertical, ScrollableContainer
from textual.widgets import Header, Footer, Static, Label, DataTable, Log, TabbedContent, TabPane
from textual.reactive import reactive
from textual.timer import Timer


# Data directories
DATA_DIR = Path(os.environ.get("XDG_DATA_HOME", Path.home() / ".local" / "share")) / "daedalos"
TOOLS_DIR = Path.home() / ".local" / "bin"


class StatusIndicator(Static):
    """A status indicator with color."""

    status = reactive("unknown")

    def __init__(self, label: str, **kwargs):
        super().__init__(**kwargs)
        self.label = label

    def render(self) -> str:
        colors = {
            "running": "[green]●[/]",
            "stopped": "[dim]○[/]",
            "error": "[red]●[/]",
            "busy": "[yellow]●[/]",
            "unknown": "[dim]?[/]",
        }
        indicator = colors.get(self.status, colors["unknown"])
        return f"{indicator} {self.label}: [bold]{self.status}[/]"


class DaemonPanel(Static):
    """Panel showing daemon status."""

    def compose(self) -> ComposeResult:
        yield Static("[bold]Daemons[/]", classes="panel-title")
        yield StatusIndicator("Loop Daemon", id="loop-status")
        yield StatusIndicator("MCP Hub", id="mcp-status")
        yield StatusIndicator("LSP Pool", id="lsp-status")
        yield StatusIndicator("Undo Daemon", id="undo-status")


class LoopPanel(Static):
    """Panel showing active loops."""

    def compose(self) -> ComposeResult:
        yield Static("[bold]Active Loops[/]", classes="panel-title")
        yield DataTable(id="loop-table")

    def on_mount(self) -> None:
        table = self.query_one("#loop-table", DataTable)
        table.add_columns("ID", "Task", "Status", "Iteration", "Duration")


class AgentPanel(Static):
    """Panel showing active agents."""

    def compose(self) -> ComposeResult:
        yield Static("[bold]Active Agents[/]", classes="panel-title")
        yield DataTable(id="agent-table")

    def on_mount(self) -> None:
        table = self.query_one("#agent-table", DataTable)
        table.add_columns("Slot", "Name", "Template", "Status", "Uptime")


class EventLog(Static):
    """Panel showing recent events."""

    def compose(self) -> ComposeResult:
        yield Static("[bold]Event Log[/]", classes="panel-title")
        yield Log(id="event-log", max_lines=100)


class FileChangesPanel(Static):
    """Panel showing recent file changes."""

    def compose(self) -> ComposeResult:
        yield Static("[bold]Recent Changes[/]", classes="panel-title")
        yield DataTable(id="changes-table")

    def on_mount(self) -> None:
        table = self.query_one("#changes-table", DataTable)
        table.add_columns("Time", "File", "Action", "Source")


class ResourcePanel(Static):
    """Panel showing resource usage."""

    def compose(self) -> ComposeResult:
        yield Static("[bold]Resources[/]", classes="panel-title")
        yield Static("", id="resource-stats")


class ObserveApp(App):
    """Daedalos Observe - Real-time dashboard."""

    CSS = """
    Screen {
        layout: grid;
        grid-size: 2 2;
        grid-gutter: 1;
    }

    .panel-title {
        background: $primary-background;
        padding: 0 1;
        text-style: bold;
    }

    DaemonPanel {
        row-span: 1;
        border: solid $primary;
        padding: 1;
    }

    LoopPanel {
        border: solid $secondary;
        padding: 1;
    }

    AgentPanel {
        border: solid $success;
        padding: 1;
    }

    EventLog {
        border: solid $warning;
        padding: 1;
    }

    DataTable {
        height: auto;
        max-height: 100%;
    }

    StatusIndicator {
        height: 1;
        padding: 0 1;
    }

    #resource-stats {
        padding: 1;
    }
    """

    BINDINGS = [
        ("q", "quit", "Quit"),
        ("r", "refresh", "Refresh"),
        ("l", "focus_loops", "Loops"),
        ("a", "focus_agents", "Agents"),
        ("d", "focus_daemons", "Daemons"),
        ("e", "focus_events", "Events"),
        ("p", "pause", "Pause"),
        ("?", "help", "Help"),
    ]

    paused = reactive(False)

    def compose(self) -> ComposeResult:
        yield Header(show_clock=True)
        yield DaemonPanel()
        yield LoopPanel()
        yield AgentPanel()
        yield EventLog()
        yield Footer()

    def on_mount(self) -> None:
        """Start update timers."""
        self.update_timer = self.set_interval(2.0, self.refresh_data)
        self.log_event("Observe started")
        self.refresh_data()

    async def refresh_data(self) -> None:
        """Refresh all data panels."""
        if self.paused:
            return

        await self.update_daemons()
        await self.update_loops()
        await self.update_agents()

    async def update_daemons(self) -> None:
        """Update daemon status indicators."""
        # Loop daemon
        loop_status = self.query_one("#loop-status", StatusIndicator)
        loop_status.status = "running" if self.check_process("loopd") else "stopped"

        # MCP Hub
        mcp_status = self.query_one("#mcp-status", StatusIndicator)
        mcp_sock = DATA_DIR / "mcp-hub" / "mcp-hub.sock"
        mcp_status.status = "running" if mcp_sock.exists() else "stopped"

        # LSP Pool
        lsp_status = self.query_one("#lsp-status", StatusIndicator)
        lsp_sock = Path("/run/daedalos/lsp-pool.sock")
        if not lsp_sock.exists():
            lsp_sock = DATA_DIR / "lsp-pool" / "lsp-pool.sock"
        lsp_status.status = "running" if lsp_sock.exists() else "stopped"

        # Undo daemon
        undo_status = self.query_one("#undo-status", StatusIndicator)
        undo_status.status = "running" if self.check_process("undod") else "stopped"

    async def update_loops(self) -> None:
        """Update loops table."""
        table = self.query_one("#loop-table", DataTable)
        table.clear()

        loop_tool = TOOLS_DIR / "loop"
        if not loop_tool.exists():
            return

        try:
            result = subprocess.run(
                [str(loop_tool), "list", "--json"],
                capture_output=True,
                text=True,
                timeout=5
            )
            if result.returncode == 0 and result.stdout.strip():
                loops = json.loads(result.stdout)
                for loop in loops:
                    table.add_row(
                        loop.get("id", "")[:8],
                        loop.get("task", "")[:30],
                        loop.get("status", ""),
                        str(loop.get("iteration", 0)),
                        self.format_duration(loop.get("duration", 0))
                    )
        except (subprocess.TimeoutExpired, json.JSONDecodeError, FileNotFoundError):
            pass

    async def update_agents(self) -> None:
        """Update agents table."""
        table = self.query_one("#agent-table", DataTable)
        table.clear()

        agent_tool = TOOLS_DIR / "agent"
        if not agent_tool.exists():
            return

        try:
            result = subprocess.run(
                [str(agent_tool), "list", "--json"],
                capture_output=True,
                text=True,
                timeout=5
            )
            if result.returncode == 0 and result.stdout.strip():
                agents = json.loads(result.stdout)
                for agent in agents:
                    table.add_row(
                        str(agent.get("slot", "")),
                        agent.get("name", ""),
                        agent.get("template", ""),
                        agent.get("status", ""),
                        self.format_duration(agent.get("uptime", 0))
                    )
        except (subprocess.TimeoutExpired, json.JSONDecodeError, FileNotFoundError):
            pass

    def log_event(self, message: str) -> None:
        """Add event to the log."""
        log = self.query_one("#event-log", Log)
        timestamp = datetime.now().strftime("%H:%M:%S")
        log.write_line(f"[dim]{timestamp}[/] {message}")

    def check_process(self, name: str) -> bool:
        """Check if a process is running."""
        try:
            result = subprocess.run(
                ["pgrep", "-f", name],
                capture_output=True,
                timeout=2
            )
            return result.returncode == 0
        except (subprocess.TimeoutExpired, FileNotFoundError):
            return False

    def format_duration(self, seconds: float) -> str:
        """Format duration in human-readable form."""
        if seconds < 60:
            return f"{int(seconds)}s"
        elif seconds < 3600:
            return f"{int(seconds // 60)}m {int(seconds % 60)}s"
        else:
            hours = int(seconds // 3600)
            minutes = int((seconds % 3600) // 60)
            return f"{hours}h {minutes}m"

    def action_refresh(self) -> None:
        """Manual refresh."""
        self.log_event("Manual refresh")
        asyncio.create_task(self.refresh_data())

    def action_pause(self) -> None:
        """Toggle pause."""
        self.paused = not self.paused
        status = "paused" if self.paused else "resumed"
        self.log_event(f"Updates {status}")
        self.sub_title = "PAUSED" if self.paused else ""

    def action_focus_loops(self) -> None:
        """Focus loops panel."""
        self.query_one("#loop-table").focus()

    def action_focus_agents(self) -> None:
        """Focus agents panel."""
        self.query_one("#agent-table").focus()

    def action_focus_daemons(self) -> None:
        """Focus daemons panel."""
        self.query_one(DaemonPanel).focus()

    def action_focus_events(self) -> None:
        """Focus event log."""
        self.query_one("#event-log").focus()


def main():
    """Run the observe app."""
    app = ObserveApp()
    app.title = "Daedalos Observe"
    app.sub_title = "Real-time Dashboard"
    app.run()


if __name__ == "__main__":
    main()
