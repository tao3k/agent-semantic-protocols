"""Large-library benchmark snapshot helper.

Builds the large-library report once and emits a deterministic, compact
view for benchmark gates and covered search command coverage.
"""

from __future__ import annotations

import argparse
from pathlib import Path
from typing import Any

from .large_library_report_chain import build_large_library_report_chain
from .output import emit_json


def benchmark_snapshot(
    repo_root: Path | None = None,
    languages: tuple[str, ...] | None = None,
) -> dict[str, Any]:
    """Return a compact benchmark snapshot from a large-library report."""
    report = build_large_library_report_chain(
        repo_root or Path("."),
        **({"languages": languages} if languages is not None else {}),
    )
    return {
        "benchmarkData": report["benchmarkData"],
        "searchCommandSet": report["searchCommandSet"],
    }


def _parse_languages(values: str | None) -> tuple[str, ...] | None:
    if not values:
        return None
    parts = [item.strip() for item in values.split(",")]
    return tuple(item for item in parts if item)


def _arg_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(
        description="Emit large-library benchmark snapshot for reporting/CI checks."
    )
    parser.add_argument(
        "--repo-root",
        default=".",
        help="Repository root to scan for sandtable scenario inputs.",
    )
    parser.add_argument(
        "--languages",
        default=None,
        help=(
            "Comma-separated language list to include, e.g. julia,python,rust,typescript."
        ),
    )
    return parser


def main(argv: list[str] | None = None) -> int:
    parser = _arg_parser()
    args = parser.parse_args(argv)
    snapshot = benchmark_snapshot(
        repo_root=Path(args.repo_root),
        languages=_parse_languages(args.languages),
    )
    emit_json(snapshot)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
