"""LSP query handling - send queries through the daemon."""

import asyncio
import json
import sys
from pathlib import Path
from typing import Optional

from .config import SOCKET_PATH


async def send_query(
    command: str,
    file_path: Path,
    line: int = 1,
    col: int = 1,
    timeout: float = 30.0
) -> dict:
    """Send an LSP query through the daemon."""
    socket_path = SOCKET_PATH

    if not socket_path.exists():
        return {"success": False, "error": "Daemon is not running"}

    request = {
        "type": "query",
        "command": command,
        "file": str(file_path.resolve()),
        "line": line,
        "col": col,
    }

    try:
        reader, writer = await asyncio.open_unix_connection(str(socket_path))
        writer.write(json.dumps(request).encode())
        await writer.drain()

        data = await asyncio.wait_for(reader.read(65536), timeout=timeout)
        response = json.loads(data.decode())

        writer.close()
        await writer.wait_closed()

        return response

    except asyncio.TimeoutError:
        return {"success": False, "error": "Query timed out"}
    except Exception as e:
        return {"success": False, "error": str(e)}


def format_hover_result(result: dict) -> str:
    """Format hover result for display."""
    if not result:
        return "No information available."

    contents = result.get("contents", result)

    if isinstance(contents, str):
        return contents

    if isinstance(contents, dict):
        return contents.get("value", str(contents))

    if isinstance(contents, list):
        parts = []
        for item in contents:
            if isinstance(item, str):
                parts.append(item)
            elif isinstance(item, dict):
                parts.append(item.get("value", str(item)))
        return "\n".join(parts)

    return str(contents)


def format_location(loc: dict) -> str:
    """Format a location result."""
    if not loc:
        return ""

    uri = loc.get("uri", "")
    if uri.startswith("file://"):
        uri = uri[7:]

    range_info = loc.get("range", {})
    start = range_info.get("start", {})
    line = start.get("line", 0) + 1
    char = start.get("character", 0) + 1

    return f"{uri}:{line}:{char}"


def format_definition_result(result) -> str:
    """Format definition result for display."""
    if not result:
        return "Definition not found."

    if isinstance(result, dict):
        return format_location(result)

    if isinstance(result, list):
        return "\n".join(format_location(loc) for loc in result if loc)

    return str(result)


def format_references_result(result) -> str:
    """Format references result for display."""
    if not result:
        return "No references found."

    if isinstance(result, list):
        lines = []
        for loc in result:
            if loc:
                lines.append(format_location(loc))
        return "\n".join(lines) if lines else "No references found."

    return str(result)


def format_completion_result(result) -> str:
    """Format completion result for display."""
    if not result:
        return "No completions."

    items = result if isinstance(result, list) else result.get("items", [])

    if not items:
        return "No completions."

    lines = []
    for item in items[:20]:  # Limit to 20
        label = item.get("label", "")
        kind = item.get("kind", 0)
        detail = item.get("detail", "")

        kind_names = {
            1: "Text", 2: "Method", 3: "Function", 4: "Constructor",
            5: "Field", 6: "Variable", 7: "Class", 8: "Interface",
            9: "Module", 10: "Property", 11: "Unit", 12: "Value",
            13: "Enum", 14: "Keyword", 15: "Snippet", 16: "Color",
            17: "File", 18: "Reference", 19: "Folder", 20: "EnumMember",
            21: "Constant", 22: "Struct", 23: "Event", 24: "Operator",
            25: "TypeParameter"
        }
        kind_str = kind_names.get(kind, "")

        line = f"  {label}"
        if kind_str:
            line += f" ({kind_str})"
        if detail:
            line += f" - {detail}"
        lines.append(line)

    return "\n".join(lines)


# CLI entry point
def main():
    """CLI for LSP queries."""
    import click

    @click.command()
    @click.argument("command", type=click.Choice([
        "hover", "definition", "references", "completion", "diagnostics"
    ]))
    @click.argument("file_path", type=click.Path(exists=True))
    @click.option("--line", "-l", default=1, help="Line number (1-indexed)")
    @click.option("--col", "-c", default=1, help="Column number (1-indexed)")
    @click.option("--json", "as_json", is_flag=True, help="Output as JSON")
    def cli(command, file_path, line, col, as_json):
        """
        Send LSP query to the pool.

        \b
        Commands:
          hover        Get hover information at position
          definition   Go to definition
          references   Find all references
          completion   Get completions at position
          diagnostics  Get file diagnostics
        """
        async def run():
            result = await send_query(
                command,
                Path(file_path),
                line=line,
                col=col
            )

            if as_json:
                click.echo(json.dumps(result, indent=2))
                return

            if not result.get("success"):
                click.echo(f"Error: {result.get('error', 'Unknown error')}", err=True)
                sys.exit(1)

            data = result.get("result", {})

            if command == "hover":
                click.echo(format_hover_result(data))
            elif command == "definition":
                click.echo(format_definition_result(data))
            elif command == "references":
                click.echo(format_references_result(data))
            elif command == "completion":
                click.echo(format_completion_result(data))
            elif command == "diagnostics":
                diags = data.get("diagnostics", [])
                if not diags:
                    click.echo("No diagnostics.")
                else:
                    for d in diags:
                        click.echo(f"{d.get('severity', 'info')}: {d.get('message', '')}")

        asyncio.run(run())

    cli()


if __name__ == "__main__":
    main()
