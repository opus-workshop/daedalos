"""CLI for error-db."""

import sys
import json
from typing import Optional

from .database import ErrorDatabase
from .matcher import PatternMatcher, format_match


def cmd_search(error_text: str, as_json: bool = False):
    """Search for matching patterns."""
    db = ErrorDatabase()
    matcher = PatternMatcher(db)

    result = matcher.search(error_text)
    db.close()

    if not result:
        if as_json:
            print(json.dumps({"error": "No matching patterns found"}))
        else:
            print("No matching patterns found.")
            print("\nAdd this error with: error-db add '<error pattern>'")
        return 1

    if as_json:
        output = {
            "pattern": {
                "id": result["pattern"].id,
                "pattern": result["pattern"].pattern,
                "language": result["pattern"].language,
            },
            "score": result["score"],
            "solutions": [
                {
                    "id": s.id,
                    "solution": s.solution,
                    "command": s.command,
                    "confidence": s.confidence,
                }
                for s in result["solutions"]
            ]
        }
        print(json.dumps(output, indent=2))
    else:
        print(format_match(result))

    return 0


def cmd_add(pattern: str, language: Optional[str] = None, solution: Optional[str] = None):
    """Add a new pattern."""
    db = ErrorDatabase()
    pattern_id = db.add_pattern(
        pattern=pattern,
        language=language,
        solution=solution
    )
    db.close()

    print(f"Pattern added: {pattern_id}")
    if not solution:
        print(f"Add a solution with: error-db solution {pattern_id} '<solution>'")


def cmd_solution(pattern_id: str, solution: str, command: Optional[str] = None):
    """Add solution to existing pattern."""
    db = ErrorDatabase()
    solution_id = db.add_solution(pattern_id, solution, command)
    db.close()

    print(f"Solution added: {solution_id}")


def cmd_confirm(solution_id: str):
    """Confirm a solution worked."""
    db = ErrorDatabase()
    db.confirm_solution(solution_id)
    db.close()

    print("Solution confirmed! Confidence increased.")


def cmd_report(solution_id: str):
    """Report a solution didn't work."""
    db = ErrorDatabase()
    db.report_failure(solution_id)
    db.close()

    print("Failure reported. Confidence decreased.")


def cmd_list(language: Optional[str] = None, as_json: bool = False):
    """List all patterns."""
    db = ErrorDatabase()
    patterns = db.get_all_patterns()
    db.close()

    if language:
        patterns = [p for p in patterns if p.language == language]

    if as_json:
        output = [
            {
                "id": p.id,
                "pattern": p.pattern,
                "language": p.language,
                "scope": p.scope,
            }
            for p in patterns
        ]
        print(json.dumps(output, indent=2))
    else:
        print(f"{'PATTERN':<50} {'LANG':<12} {'SCOPE':<10}")
        print("-" * 75)
        for p in patterns:
            pattern_display = p.pattern[:47] + "..." if len(p.pattern) > 47 else p.pattern
            lang = p.language or "-"
            print(f"{pattern_display:<50} {lang:<12} {p.scope:<10}")
        print(f"\nTotal: {len(patterns)} patterns")


def cmd_show(pattern_id: str):
    """Show pattern details."""
    db = ErrorDatabase()
    pattern = db.get_pattern(pattern_id)

    if not pattern:
        print(f"Pattern not found: {pattern_id}")
        db.close()
        return 1

    solutions = db.get_solutions(pattern_id)
    db.close()

    print(f"Pattern: {pattern.pattern}")
    print(f"ID: {pattern.id}")
    print(f"Scope: {pattern.scope}")
    if pattern.language:
        print(f"Language: {pattern.language}")
    print(f"Created: {pattern.created_at}")

    print(f"\nSolutions ({len(solutions)}):")
    for i, s in enumerate(solutions, 1):
        print(f"\n  [{i}] Confidence: {s.confidence:.0%}")
        print(f"  ID: {s.id}")
        for line in s.solution.split("\n"):
            print(f"    {line}")
        if s.command:
            print(f"  Command: {s.command}")
        print(f"  Success: {s.success_count}, Failures: {s.failure_count}")

    return 0


def cmd_stats():
    """Show database statistics."""
    db = ErrorDatabase()
    stats = db.stats()
    db.close()

    print("ERROR-DB Statistics")
    print("=" * 40)
    print(f"Total patterns: {stats['total_patterns']}")
    print(f"Total solutions: {stats['total_solutions']}")

    if stats['by_scope']:
        print("\nBy scope:")
        for scope, count in stats['by_scope'].items():
            print(f"  {scope}: {count}")

    if stats['by_language']:
        print("\nBy language:")
        for lang, count in stats['by_language'].items():
            print(f"  {lang}: {count}")


def cmd_help():
    """Show help message."""
    help_text = """
error-db - Error Pattern Database

USAGE
    error-db <command> [arguments]

COMMANDS
    search <error>       Search for matching patterns
    add <pattern>        Add a new error pattern
    solution <id> <sol>  Add solution to pattern
    confirm <id>         Mark solution as successful
    report <id>          Mark solution as failed
    list                 List all patterns
    show <id>            Show pattern details
    stats                Show database statistics

OPTIONS
    --json               Output as JSON
    --language <lang>    Filter by language
    --stdin              Read error from stdin

EXAMPLES
    # Search for an error
    error-db search "Cannot find module 'express'"

    # Pipe error from command
    npm test 2>&1 | error-db search --stdin

    # Add new pattern
    error-db add "TypeError: Cannot read property 'X' of undefined"

    # Add solution
    error-db solution <pattern-id> "Check if object is null before accessing"

    # Confirm solution worked
    error-db confirm <solution-id>
"""
    print(help_text)


def main():
    """Main entry point."""
    args = sys.argv[1:]

    if not args or args[0] in ["help", "--help", "-h"]:
        cmd_help()
        return 0

    cmd = args[0]
    args = args[1:]

    # Parse common flags
    as_json = "--json" in args
    if as_json:
        args.remove("--json")

    from_stdin = "--stdin" in args
    if from_stdin:
        args.remove("--stdin")

    language = None
    if "--language" in args:
        idx = args.index("--language")
        language = args[idx + 1]
        args = args[:idx] + args[idx + 2:]

    try:
        if cmd == "search":
            if from_stdin:
                error_text = sys.stdin.read()
            else:
                error_text = " ".join(args)
            return cmd_search(error_text, as_json)

        elif cmd == "add":
            pattern = args[0] if args else ""
            solution = None
            if len(args) > 1:
                solution = " ".join(args[1:])
            cmd_add(pattern, language, solution)

        elif cmd == "solution":
            if len(args) < 2:
                print("Usage: error-db solution <pattern-id> <solution>")
                return 1
            pattern_id = args[0]
            solution = " ".join(args[1:])
            cmd_solution(pattern_id, solution)

        elif cmd == "confirm":
            if not args:
                print("Usage: error-db confirm <solution-id>")
                return 1
            cmd_confirm(args[0])

        elif cmd == "report":
            if not args:
                print("Usage: error-db report <solution-id>")
                return 1
            cmd_report(args[0])

        elif cmd == "list":
            cmd_list(language, as_json)

        elif cmd == "show":
            if not args:
                print("Usage: error-db show <pattern-id>")
                return 1
            return cmd_show(args[0])

        elif cmd == "stats":
            cmd_stats()

        elif cmd == "version":
            print("error-db 1.0.0")

        else:
            print(f"Unknown command: {cmd}")
            return 1

    except Exception as e:
        print(f"Error: {e}", file=sys.stderr)
        return 1

    return 0


if __name__ == "__main__":
    sys.exit(main())
