"""CLI entry point for graph search observation JSONL generation."""

from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path

from tools.semantic_sandtable.graph_search_observation_builder import (
    observations_from_report,
    write_jsonl,
)
from tools.semantic_sandtable.graph_search_observation_contract import SCHEMA_ID


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(
        description="Build graph search observation JSONL from a semantic sandtable report."
    )
    parser.add_argument("--sandtable-report", required=True, type=Path)
    parser.add_argument("--output", required=True, type=Path)
    parser.add_argument(
        "--source-ref",
        help="Relative artifact reference to store in the observation source.",
    )
    args = parser.parse_args(argv)

    report = json.loads(args.sandtable_report.read_text(encoding="utf-8"))
    observations = observations_from_report(report, source_ref=args.source_ref)
    write_jsonl(observations, args.output)
    sys.stdout.write(
        json.dumps(
            {
                "schemaId": SCHEMA_ID,
                "observations": len(observations),
                "output": str(args.output),
            },
            sort_keys=True,
        )
    )
    sys.stdout.write("\n")
    return 0
