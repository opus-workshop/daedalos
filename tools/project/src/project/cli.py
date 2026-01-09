"""CLI for project tool."""

import sys
from pathlib import Path
from typing import Optional

import click

from .index import ProjectIndex
from .formatters import SummaryFormatter, TreeFormatter, MapFormatter, StatsFormatter


@click.group()
@click.version_option(version="0.1.0")
def cli():
    """Pre-computed codebase intelligence.

    Index and query project structure, dependencies, and conventions.
    """
    pass


@cli.command()
@click.argument("path", default=".", type=click.Path(exists=True))
@click.option("--json", "as_json", is_flag=True, help="Output as JSON")
@click.option("--brief", is_flag=True, help="One-line summary")
@click.option("--no-color", is_flag=True, help="Disable colors")
@click.option("--refresh", is_flag=True, help="Force re-index")
def summary(path: str, as_json: bool, brief: bool, no_color: bool, refresh: bool):
    """Show project summary.

    Displays project type, architecture, key modules, dependencies,
    and detected conventions.
    """
    try:
        idx = ProjectIndex(path, refresh=refresh)
        data = idx.get_summary()
        idx.close()

        formatter = SummaryFormatter(as_json=as_json, brief=brief, no_color=no_color)
        click.echo(formatter.format(data))
    except Exception as e:
        click.echo(f"Error: {e}", err=True)
        sys.exit(1)


@cli.command()
@click.argument("path", default=".", type=click.Path(exists=True))
@click.option("--format", "fmt", type=click.Choice(["tree", "json", "dot"]),
              default="tree", help="Output format")
@click.option("--no-color", is_flag=True, help="Disable colors")
@click.option("--refresh", is_flag=True, help="Force re-index")
def map(path: str, fmt: str, no_color: bool, refresh: bool):
    """Show dependency map.

    Displays project dependency graph in various formats.
    """
    try:
        idx = ProjectIndex(path, refresh=refresh)

        # Get all dependencies
        deps = []
        for row in idx.db.conn.execute("""
            SELECT f.path as source, d.target_path as target
            FROM dependencies d
            JOIN files f ON d.source_file_id = f.id
        """):
            deps.append({"source": row["source"], "target": row["target"]})

        idx.close()

        formatter = MapFormatter(format_type=fmt, no_color=no_color)
        click.echo(formatter.format(deps))
    except Exception as e:
        click.echo(f"Error: {e}", err=True)
        sys.exit(1)


@cli.command()
@click.argument("file_path", type=str)
@click.option("--project", "-p", default=".", type=click.Path(exists=True),
              help="Project root")
@click.option("--json", "as_json", is_flag=True, help="Output as JSON")
@click.option("--refresh", is_flag=True, help="Force re-index")
def deps(file_path: str, project: str, as_json: bool, refresh: bool):
    """Show dependencies of a file.

    Lists all files/modules that the specified file imports.
    """
    import json

    try:
        idx = ProjectIndex(project, refresh=refresh)
        result = idx.get_file_deps(file_path)
        idx.close()

        if "error" in result:
            click.echo(f"Error: {result['error']}", err=True)
            sys.exit(1)

        if as_json:
            click.echo(json.dumps(result, indent=2))
        else:
            click.echo(f"Dependencies of {file_path}:")
            for imp in result.get("imports", []):
                click.echo(f"  -> {imp}")
            if not result.get("imports"):
                click.echo("  (none)")
    except Exception as e:
        click.echo(f"Error: {e}", err=True)
        sys.exit(1)


@cli.command()
@click.argument("file_path", type=str)
@click.option("--project", "-p", default=".", type=click.Path(exists=True),
              help="Project root")
@click.option("--json", "as_json", is_flag=True, help="Output as JSON")
@click.option("--refresh", is_flag=True, help="Force re-index")
def dependents(file_path: str, project: str, as_json: bool, refresh: bool):
    """Show files that depend on a file.

    Lists all files that import the specified file.
    """
    import json

    try:
        idx = ProjectIndex(project, refresh=refresh)
        result = idx.get_file_dependents(file_path)
        idx.close()

        if "error" in result:
            click.echo(f"Error: {result['error']}", err=True)
            sys.exit(1)

        if as_json:
            click.echo(json.dumps(result, indent=2))
        else:
            click.echo(f"Files that import {file_path}:")
            for dep in result.get("imported_by", []):
                click.echo(f"  <- {dep}")
            if not result.get("imported_by"):
                click.echo("  (none)")
    except Exception as e:
        click.echo(f"Error: {e}", err=True)
        sys.exit(1)


@cli.command()
@click.argument("query", type=str)
@click.option("--project", "-p", default=".", type=click.Path(exists=True),
              help="Project root")
@click.option("--type", "-t", "symbol_type", help="Filter by symbol type")
@click.option("--json", "as_json", is_flag=True, help="Output as JSON")
@click.option("--limit", "-n", default=20, help="Max results")
@click.option("--refresh", is_flag=True, help="Force re-index")
def search(query: str, project: str, symbol_type: Optional[str], as_json: bool,
           limit: int, refresh: bool):
    """Search for symbols.

    Searches for functions, classes, types, etc. by name pattern.
    """
    import json

    try:
        idx = ProjectIndex(project, refresh=refresh)
        results = idx.search_symbols(query)
        idx.close()

        # Filter by type if specified
        if symbol_type:
            results = [r for r in results if r["type"] == symbol_type]

        # Limit results
        results = results[:limit]

        if as_json:
            click.echo(json.dumps(results, indent=2))
        else:
            if not results:
                click.echo("No symbols found")
            else:
                for r in results:
                    file_path = r.get('file_path', r.get('file', 'unknown'))
                    loc = f"{file_path}:{r['line_start']}"
                    sig = r.get('signature', '') or r['name']
                    click.echo(f"  [{r['type']}] {sig}")
                    click.echo(f"         {loc}")
    except Exception as e:
        click.echo(f"Error: {e}", err=True)
        sys.exit(1)


@cli.command()
@click.argument("path", default=".", type=click.Path(exists=True))
@click.option("--json", "as_json", is_flag=True, help="Output as JSON")
@click.option("--refresh", is_flag=True, help="Force re-index")
def stats(path: str, as_json: bool, refresh: bool):
    """Show project statistics.

    Displays file counts, symbol counts, and line counts by type.
    """
    try:
        idx = ProjectIndex(path, refresh=refresh)
        data = idx.db.get_stats()
        idx.close()

        formatter = StatsFormatter(as_json=as_json)
        click.echo(formatter.format(data))
    except Exception as e:
        click.echo(f"Error: {e}", err=True)
        sys.exit(1)


@cli.command()
@click.argument("path", default=".", type=click.Path(exists=True))
@click.option("--full", is_flag=True, help="Full re-index (clear cache first)")
def index(path: str, full: bool):
    """Index or re-index the project.

    Scans all files and builds the symbol database.
    """
    try:
        click.echo(f"Indexing {path}...")
        idx = ProjectIndex(path, refresh=True)

        if full:
            idx.reindex(full=True)

        stats = idx.db.get_stats()
        idx.close()

        click.echo(f"Indexed {stats.get('files', 0)} files, "
                   f"{stats.get('symbols', 0)} symbols")
    except Exception as e:
        click.echo(f"Error: {e}", err=True)
        sys.exit(1)


@cli.command()
@click.argument("path", default=".", type=click.Path(exists=True))
@click.option("--depth", "-d", default=3, help="Max depth to display")
@click.option("--no-color", is_flag=True, help="Disable colors")
@click.option("--refresh", is_flag=True, help="Force re-index")
def tree(path: str, depth: int, no_color: bool, refresh: bool):
    """Show project file tree.

    Displays indexed files as a tree structure.
    """
    try:
        idx = ProjectIndex(path, refresh=refresh)
        files = idx.db.get_all_files()
        idx.close()

        formatter = TreeFormatter(max_depth=depth, no_color=no_color)
        result = formatter.format(files)
        if result:
            click.echo(result)
        else:
            click.echo("No files indexed")
    except Exception as e:
        click.echo(f"Error: {e}", err=True)
        sys.exit(1)


def main():
    """Entry point."""
    cli()


if __name__ == "__main__":
    main()
