"""CLI for context tool."""

import sys
import json
import click

from .tracker import ContextTracker
from .visualizer import (
    format_status,
    format_breakdown,
    format_files,
    format_suggestions,
    format_checkpoints,
)


@click.group()
@click.version_option(version="1.0.0")
@click.option("--no-color", is_flag=True, help="Disable colored output")
@click.pass_context
def cli(ctx, no_color):
    """Context window management for Claude Code.

    Monitor and manage Claude's context usage to optimize long sessions.
    """
    ctx.ensure_object(dict)
    ctx.obj["use_color"] = not no_color


@cli.command()
@click.option("--json", "as_json", is_flag=True, help="Output as JSON")
@click.pass_context
def status(ctx, as_json):
    """Show current context budget.

    Displays a visual progress bar showing how much of Claude's
    context window is being used.
    """
    tracker = ContextTracker()
    status_data = tracker.get_status()

    if as_json:
        click.echo(json.dumps(status_data, indent=2))
    else:
        use_color = ctx.obj.get("use_color", True)
        click.echo(format_status(status_data, use_color=use_color))


@cli.command()
@click.option("--json", "as_json", is_flag=True, help="Output as JSON")
@click.pass_context
def breakdown(ctx, as_json):
    """Show detailed context breakdown.

    Shows how context is distributed across different categories:
    system prompts, user messages, assistant responses, tool results, etc.
    """
    tracker = ContextTracker()
    status_data = tracker.get_status()

    if as_json:
        click.echo(json.dumps(status_data["breakdown"], indent=2))
    else:
        use_color = ctx.obj.get("use_color", True)
        click.echo(format_breakdown(status_data, use_color=use_color))


@cli.command()
@click.option("--json", "as_json", is_flag=True, help="Output as JSON")
@click.option("--limit", "-n", default=10, help="Max files to show")
@click.pass_context
def files(ctx, as_json, limit):
    """Show files currently in context.

    Lists files that have been read during this session,
    sorted by token count.
    """
    tracker = ContextTracker()
    files_data = tracker.get_files_in_context()

    if as_json:
        click.echo(json.dumps(files_data[:limit], indent=2))
    else:
        use_color = ctx.obj.get("use_color", True)
        click.echo(format_files(files_data, use_color=use_color, limit=limit))


@cli.command()
@click.option("--json", "as_json", is_flag=True, help="Output as JSON")
@click.pass_context
def compact(ctx, as_json):
    """Suggest context compaction strategies.

    Analyzes current context usage and suggests ways to reduce
    token consumption for longer sessions.
    """
    tracker = ContextTracker()
    suggestions = tracker.get_compaction_suggestions()

    if as_json:
        click.echo(json.dumps(suggestions, indent=2))
    else:
        use_color = ctx.obj.get("use_color", True)
        click.echo(format_suggestions(suggestions, use_color=use_color))


@cli.command()
@click.argument("name")
def checkpoint(name):
    """Save context checkpoint for later reference.

    Creates a snapshot of the current context state that can be
    used to understand what was in context at a particular point.
    """
    tracker = ContextTracker()
    data = tracker.checkpoint(name)

    click.echo(f"Checkpoint '{name}' created")
    click.echo(f"  Tokens: {data['status']['used']:,}")
    click.echo(f"  Files: {len(data['files'])}")


@cli.command("list-checkpoints")
@click.option("--json", "as_json", is_flag=True, help="Output as JSON")
@click.pass_context
def list_checkpoints(ctx, as_json):
    """List saved checkpoints."""
    tracker = ContextTracker()
    checkpoints = tracker.list_checkpoints()

    if as_json:
        click.echo(json.dumps(checkpoints, indent=2))
    else:
        use_color = ctx.obj.get("use_color", True)
        click.echo(format_checkpoints(checkpoints, use_color=use_color))


@cli.command()
@click.argument("name")
@click.option("--json", "as_json", is_flag=True, help="Output as JSON")
def restore(name, as_json):
    """Show checkpoint contents.

    Displays what was in context at the time the checkpoint was created.
    """
    tracker = ContextTracker()
    data = tracker.restore_checkpoint(name)

    if not data:
        click.echo(f"Checkpoint '{name}' not found", err=True)
        sys.exit(1)

    if as_json:
        click.echo(json.dumps(data, indent=2))
    else:
        click.echo(f"Checkpoint: {data['name']}")
        click.echo(f"Created: {data['created']}")
        click.echo(f"Project: {data.get('project', 'unknown')}")
        click.echo()
        click.echo(f"Status at checkpoint:")
        click.echo(f"  Used: {data['status']['used']:,} tokens")
        click.echo(f"  Percentage: {data['status']['percentage']:.1f}%")
        click.echo()
        if data.get("files"):
            click.echo("Files in context:")
            for f in data["files"][:10]:
                click.echo(f"  {f['path']} ({f['tokens']:,} tokens)")


@cli.command()
@click.pass_context
def full(ctx):
    """Show complete context report.

    Combines status, breakdown, and files into a comprehensive report.
    """
    tracker = ContextTracker()
    status_data = tracker.get_status()
    files_data = tracker.get_files_in_context()
    suggestions = tracker.get_compaction_suggestions()

    use_color = ctx.obj.get("use_color", True)

    click.echo(format_status(status_data, use_color=use_color))
    click.echo()
    click.echo(format_breakdown(status_data, use_color=use_color))
    click.echo()
    click.echo(format_files(files_data, use_color=use_color, limit=5))

    if suggestions:
        click.echo()
        click.echo(format_suggestions(suggestions, use_color=use_color))


def main():
    """Entry point."""
    cli()


if __name__ == "__main__":
    main()
