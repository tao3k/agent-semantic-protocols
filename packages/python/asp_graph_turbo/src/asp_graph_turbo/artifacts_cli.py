"""CLI for evaluating graph turbo against cached ASP search artifacts."""

from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path
from typing import Sequence

from .artifacts import evaluate_search_artifacts


def main(argv: Sequence[str] | None = None) -> int:
    args = _parse_args(argv)
    report = evaluate_search_artifacts(
        args.artifact_dir,
        limit=args.limit,
        budget=args.budget,
        examples=args.examples,
    )
    if args.format == "json":
        sys.stdout.write(json.dumps(report, sort_keys=True) + "\n")
    else:
        _write_text_report(report)
    return 0


def _parse_args(argv: Sequence[str] | None) -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "artifact_dir",
        nargs="?",
        type=Path,
        default=Path(".cache/agent-semantic-protocol/artifacts"),
    )
    parser.add_argument("--limit", type=int, default=None)
    parser.add_argument("--budget", type=int, default=10)
    parser.add_argument("--examples", type=int, default=5)
    parser.add_argument("--format", choices=["text", "json"], default="text")
    return parser.parse_args(argv)


def _write_text_report(report: dict[str, object]) -> None:
    averages = report.get("averages")
    _emit(
        "[graph-turbo-artifacts] "
        f"scanned={report['scanned']} converted={report['converted']} "
        f"skipped={report['skipped']} elapsedMs={report['elapsedMs']} "
        f"secondPassCacheHits={report['secondPassCacheHits']}"
    )
    _emit(f"profiles={report['profileCounts']}")
    _emit(f"languages={report['languageCounts']}")
    _emit(f"methods={report['methodCounts']}")
    _emit(f"averages={averages}")
    coverage = report.get("historicalCommandCoverage")
    if isinstance(coverage, dict):
        _emit(
            "historicalCommandCoverage="
            f"labels={coverage.get('labelCount')} "
            f"measurable={coverage.get('measurableLabels')} "
            f"covered={coverage.get('coveredLabels')} "
            f"top3={coverage.get('top3')} "
            f"top5={coverage.get('top5')} "
            f"mrr={coverage.get('mrr')}"
        )
    for example in report.get("examples", []):
        _emit(
            "[graph-turbo-example] "
            f"method={example['method']} profile={example['profile']} "
            f"inputTargets={example['inputTargets']} "
            f"ranked={len(example['rankedNodes'])} "
            f"dup={example['inputMaxDuplicate']}->{example['rankedMaxDuplicate']} "
            f"paths={example['pathCount']} cache2={example['cacheStatus2']} "
            f"path={example['path']}"
        )


def _emit(line: str) -> None:
    sys.stdout.write(line + "\n")


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))
