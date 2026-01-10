"""CLI for semantic code search."""

import os
import sys
from pathlib import Path

import click

from .indexer import CodeIndex
from .searcher import CodeSearcher, format_results


def find_project_root(start_path: str = ".") -> Path:
    """Find the project root by looking for common markers."""
    current = Path(start_path).resolve()

    markers = [".git", "package.json", "Cargo.toml", "pyproject.toml", "go.mod", "Package.swift"]

    while current != current.parent:
        for marker in markers:
            if (current / marker).exists():
                return current
        current = current.parent

    # No marker found, use the starting directory
    return Path(start_path).resolve()


@click.group()
@click.option("--project", "-p", default=".", help="Project path")
@click.pass_context
def main(ctx, project):
    """
    Semantic code search - ask natural language questions about your codebase.

    \b
    Examples:
      codex search "where is authentication handled?"
      codex search "what functions call the database?"
      codex search -f auth.py "login logic"

    \b
    Commands:
      search   Search the codebase (main command)
      index    Build/update the search index
      status   Show index statistics
      clear    Clear the index
      similar  Find code similar to a location
      explain  Search within a specific file
    """
    ctx.ensure_object(dict)

    # Find project root
    if project == ".":
        project_path = find_project_root()
    else:
        project_path = Path(project).resolve()

    ctx.obj["project_path"] = project_path


@main.command("search")
@click.argument("query")
@click.option("--limit", "-n", default=5, help="Number of results")
@click.option("--file", "-f", "file_filter", help="Filter by file path pattern")
@click.option("--type", "-t", "type_filter", help="Filter by chunk type")
@click.option("--show-content", "-c", is_flag=True, help="Show code content")
@click.option("--reindex", is_flag=True, help="Force reindex")
@click.pass_context
def search(ctx, query, limit, file_filter, type_filter, show_content, reindex):
    """
    Search the codebase with a natural language query.

    \b
    Examples:
      codex search "where is authentication handled?"
      codex search "what functions call the database?"
      codex search -c "error handling patterns"
      codex search -f auth.py "login logic"
    """
    project_path = ctx.obj.get("project_path") or find_project_root()
    index = CodeIndex(str(project_path))

    # Check if index exists or reindex requested
    if reindex or not index.is_indexed():
        click.echo(f"Indexing {project_path}...")
        index.index_project(force=reindex)
        click.echo()

    searcher = CodeSearcher(index)
    results = searcher.search(
        query,
        limit=limit,
        file_filter=file_filter,
        type_filter=type_filter,
    )

    output = format_results(results, show_content=show_content)
    click.echo(output)


@main.command("index")
@click.option("--force", is_flag=True, help="Force full reindex")
@click.pass_context
def index(ctx, force):
    """Build or update the search index."""
    project_path = ctx.obj.get("project_path") or find_project_root()
    click.echo(f"Indexing {project_path}...")

    idx = CodeIndex(str(project_path))
    idx.index_project(force=force)


@main.command("status")
@click.pass_context
def status(ctx):
    """Show index status and statistics."""
    project_path = ctx.obj.get("project_path") or find_project_root()
    idx = CodeIndex(str(project_path))

    if not idx.is_indexed():
        click.echo("Not indexed. Run 'codex index' first.")
        return

    stats = idx.get_stats()
    click.echo(f"Project:  {stats['project_path']}")
    click.echo(f"Database: {stats['db_path']}")
    click.echo(f"Backend:  {stats['backend']}")
    click.echo(f"Files:    {stats['files']}")
    click.echo(f"Chunks:   {stats['chunks']}")


@main.command("clear")
@click.pass_context
def clear(ctx):
    """Clear the search index."""
    project_path = ctx.obj.get("project_path") or find_project_root()
    idx = CodeIndex(str(project_path))

    if click.confirm("Clear the index?"):
        idx.clear()


@main.command("similar")
@click.argument("file_path")
@click.argument("line", type=int)
@click.option("--limit", "-n", default=5, help="Number of results")
@click.pass_context
def similar(ctx, file_path, line, limit):
    """
    Find code similar to a specific location.

    \b
    Example:
      codex similar src/auth.py 42
    """
    project_path = ctx.obj.get("project_path") or find_project_root()
    idx = CodeIndex(str(project_path))

    if not idx.is_indexed():
        click.echo("Not indexed. Run 'codex index' first.")
        return

    searcher = CodeSearcher(idx)

    # Make path relative to project
    try:
        rel_path = str(Path(file_path).relative_to(project_path))
    except ValueError:
        rel_path = file_path

    results = searcher.find_similar(rel_path, line, limit=limit)

    if not results:
        click.echo("No similar code found.")
        return

    output = format_results(results, show_content=True)
    click.echo(output)


@main.command("explain")
@click.argument("file_path")
@click.argument("query")
@click.option("--limit", "-n", default=5, help="Number of results")
@click.pass_context
def explain(ctx, file_path, query, limit):
    """
    Search within a specific file.

    \b
    Example:
      codex explain src/auth.py "how does login work?"
    """
    project_path = ctx.obj.get("project_path") or find_project_root()
    idx = CodeIndex(str(project_path))

    if not idx.is_indexed():
        click.echo("Not indexed. Run 'codex index' first.")
        return

    searcher = CodeSearcher(idx)

    # Make path relative to project
    try:
        rel_path = str(Path(file_path).relative_to(project_path))
    except ValueError:
        rel_path = file_path

    results = searcher.search_file(query, rel_path, limit=limit)
    output = format_results(results, show_content=True)
    click.echo(output)


if __name__ == "__main__":
    main(obj={})
