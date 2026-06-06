#!/usr/bin/env python3
"""Validate Julia ASP cache miss/hit performance evidence."""

from __future__ import annotations

import json
import pathlib
import sys


def load_receipt(root: pathlib.Path, name: str) -> dict:
    receipt_path = root / f"{name}.receipt.json"
    for line in receipt_path.read_text().splitlines():
        if line.startswith("{"):
            return json.loads(line)
    raise SystemExit(f"missing {name} receipt json in {receipt_path}")


def _main() -> int:
    if len(sys.argv) != 2:
        raise SystemExit("usage: perf-calibrate-julia-cache.py <evidence-dir>")
    root = pathlib.Path(sys.argv[1])
    miss = load_receipt(root, "miss")
    hit = load_receipt(root, "hit")

    assert miss["route"] == "local-native", miss
    assert miss["providerProcessesSpawned"] >= 1, miss
    assert miss.get("packetBytes", 0) > 0, miss
    assert miss.get("sqliteWriteCount", 0) >= 1, miss
    assert hit["route"] == "local-cache", hit
    assert hit["cacheStatus"] == "hit", hit
    assert hit["providerProcessesSpawned"] == 0, hit
    assert hit["providerCommandCount"] == 0, hit
    assert (root / "miss.out").read_bytes() == (root / "hit.out").read_bytes()

    sys.stdout.write(
        "[perf-calibrate-julia-cache] "
        f"missElapsedMs={miss.get('elapsedMs')} "
        f"hitElapsedMs={hit.get('elapsedMs')} "
        f"packetBytes={miss.get('packetBytes')} "
        f"sqliteWriteCount={miss.get('sqliteWriteCount')} "
        f"evidence={root}\n"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(_main())
