"""Validate Julia ASP cache miss/hit performance evidence."""

from __future__ import annotations

import argparse
import json
import pathlib
import sys
from collections.abc import Sequence
from dataclasses import dataclass
from typing import Any


Receipt = dict[str, Any]


@dataclass(frozen=True, slots=True)
class CachePerformanceEvidence:
    """Validated Julia cache miss/hit receipt pair."""

    root: pathlib.Path
    miss: Receipt
    hit: Receipt


def load_receipt(root: pathlib.Path, name: str) -> Receipt:
    receipt_path = root / f"{name}.receipt.json"
    for line in receipt_path.read_text().splitlines():
        if line.startswith("{"):
            return json.loads(line)
    raise SystemExit(f"missing {name} receipt json in {receipt_path}")


def main(argv: Sequence[str] | None = None) -> int:
    args = _parse_args(argv)
    evidence = _load_evidence(args.evidence_dir)
    _validate_evidence(evidence)
    sys.stdout.write(_render_summary(evidence))
    return 0


def _load_evidence(root: pathlib.Path) -> CachePerformanceEvidence:
    return CachePerformanceEvidence(
        root=root,
        miss=load_receipt(root, "miss"),
        hit=load_receipt(root, "hit"),
    )


def _validate_evidence(evidence: CachePerformanceEvidence) -> None:
    miss = evidence.miss
    hit = evidence.hit

    assert miss["route"] == "local-native", miss
    assert miss["providerProcessesSpawned"] >= 1, miss
    assert miss.get("packetBytes", 0) > 0, miss
    assert miss.get("sqliteWriteCount", 0) >= 1, miss
    assert hit["route"] == "local-cache", hit
    assert hit["cacheStatus"] == "hit", hit
    assert hit["providerProcessesSpawned"] == 0, hit
    assert hit["providerCommandCount"] == 0, hit
    assert (evidence.root / "miss.out").read_bytes() == (
        evidence.root / "hit.out"
    ).read_bytes()


def _render_summary(evidence: CachePerformanceEvidence) -> str:
    miss = evidence.miss
    hit = evidence.hit
    return (
        "[perf-calibrate-julia-cache] "
        f"missElapsedMs={miss.get('elapsedMs')} "
        f"hitElapsedMs={hit.get('elapsedMs')} "
        f"packetBytes={miss.get('packetBytes')} "
        f"sqliteWriteCount={miss.get('sqliteWriteCount')} "
        f"evidence={evidence.root}\n"
    )


def _parse_args(argv: Sequence[str] | None) -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        prog="python -m tools cache validate julia-performance",
        description="Validate Julia ASP cache miss/hit performance evidence.",
    )
    parser.add_argument("evidence_dir", type=pathlib.Path)
    return parser.parse_args(argv)


if __name__ == "__main__":
    raise SystemExit(main())
