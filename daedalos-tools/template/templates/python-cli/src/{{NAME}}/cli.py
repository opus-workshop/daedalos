"""{{NAME}} CLI - {{DESCRIPTION}}"""

import click

from . import __version__


@click.group()
@click.version_option(version=__version__)
def main():
    """{{DESCRIPTION}}"""
    pass


@main.command()
def hello():
    """Say hello."""
    click.echo("Hello from {{NAME}}!")


if __name__ == "__main__":
    main()
