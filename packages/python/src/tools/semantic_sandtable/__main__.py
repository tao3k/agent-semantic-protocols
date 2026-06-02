"""Command-line entrypoint for semantic sandtable scenarios."""

from .runner import semantic_sandtable_main

if __name__ == "__main__":
    raise SystemExit(semantic_sandtable_main())
