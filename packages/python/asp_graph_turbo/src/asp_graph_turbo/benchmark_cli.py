"""Benchmark graph turbo ranking for sandtable evidence."""

from __future__ import annotations

import argparse
import json
import math
import statistics
import sys
import time
from collections import Counter
from collections.abc import Mapping, Sequence

from .cli import _load_packet, _rank_packet
from .constants import ALGORITHM_ID
from .packet import result_to_packet


def main(argv: Sequence[str] | None = None) -> int:
    args = _parse_args(argv)
    packet = _load_packet(args.packet)
    benchmark = benchmark_packet(
        packet,
        runs=args.runs,
        warmup_runs=args.warmup_runs,
        cache_mode=args.cache_mode,
        profile=args.profile,
        seed=args.seed,
        limit=args.limit,
    )
    if args.format == "json":
        sys.stdout.write(json.dumps(benchmark, sort_keys=True) + "\n")
    else:
        sys.stdout.write(_render_text(benchmark) + "\n")
    return 0


def benchmark_packet(
    packet: Mapping[str, object],
    *,
    runs: int,
    warmup_runs: int,
    cache_mode: str,
    profile: str | None = None,
    seed: Sequence[str] = (),
    limit: int | None = None,
) -> dict[str, object]:
    benchmark, _last_result_packet = benchmark_packet_with_result(
        packet,
        runs=runs,
        warmup_runs=warmup_runs,
        cache_mode=cache_mode,
        profile=profile,
        seed=seed,
        limit=limit,
    )
    return benchmark


def benchmark_packet_with_result(
    packet: Mapping[str, object],
    *,
    runs: int,
    warmup_runs: int,
    cache_mode: str,
    profile: str | None = None,
    seed: Sequence[str] = (),
    limit: int | None = None,
) -> tuple[dict[str, object], dict[str, object]]:
    packet = _packet_with_cache_mode(packet, cache_mode)
    rank_args = _rank_args(profile=profile, seed=seed, limit=limit)
    warmup_cache_statuses: list[str] = []
    for _ in range(warmup_runs):
        warmup_packet = result_to_packet(_rank_packet(packet, rank_args))
        warmup_cache_statuses.append(_cache_status(warmup_packet))
    durations: list[float] = []
    cache_statuses: list[str] = []
    last_packet: dict[str, object] | None = None
    for _ in range(runs):
        started = time.perf_counter()
        result = _rank_packet(packet, rank_args)
        durations.append((time.perf_counter() - started) * 1000.0)
        last_packet = result_to_packet(result)
        cache_statuses.append(_cache_status(last_packet))
    if last_packet is None:
        raise SystemExit("graph turbo benchmark runs must be positive")
    metrics = last_packet["algorithmMetrics"]
    return (
        {
            "schemaId": "agent.semantic-protocols.semantic-graph-turbo-benchmark",
            "schemaVersion": "1",
            "protocolId": "agent.semantic-protocols.semantic-language",
            "protocolVersion": "1",
            "packetKind": "graph-turbo-benchmark",
            "algorithm": ALGORITHM_ID,
            "profile": last_packet["profile"],
            "runs": runs,
            "warmupRuns": warmup_runs,
            "cacheMode": cache_mode,
            "durationMs": _duration_summary(durations),
            "cacheStatusCounts": _status_counts(cache_statuses),
            "warmupCacheStatusCounts": _status_counts(warmup_cache_statuses),
            "lastAlgorithmMetrics": metrics,
            "lastProfileMatrix": _profile_matrix(last_packet),
            "lastTypedPathTrace": _last_typed_path_trace(last_packet),
        },
        last_packet,
    )


def _duration_summary(durations: list[float]) -> dict[str, float]:
    sorted_durations = sorted(durations)
    p95_index = max(
        0,
        min(len(sorted_durations) - 1, math.ceil(len(durations) * 0.95) - 1),
    )
    return {
        "min": round(min(durations), 6),
        "mean": round(statistics.mean(durations), 6),
        "median": round(statistics.median(durations), 6),
        "p95": round(sorted_durations[p95_index], 6),
        "max": round(max(durations), 6),
    }


def _cache_status(packet: Mapping[str, object]) -> str:
    metrics = packet.get("algorithmMetrics")
    if isinstance(metrics, Mapping):
        status = metrics.get("cacheStatus")
        if isinstance(status, str) and status:
            return status
    return "unknown"


def _status_counts(statuses: Sequence[str]) -> dict[str, int]:
    return dict(sorted(Counter(statuses).items()))


def _last_typed_path_trace(packet: Mapping[str, object]) -> Mapping[str, object]:
    trace = packet.get("algorithmTrace")
    if not isinstance(trace, list):
        return {}
    for step in trace:
        if isinstance(step, Mapping) and step.get("step") == "typed-paths":
            return step
    return {}


def _profile_matrix(packet: Mapping[str, object]) -> Mapping[str, object]:
    profile = packet.get("profile")
    matrices = packet.get("profileMatrices")
    if not isinstance(matrices, list):
        return {}
    for matrix in matrices:
        if isinstance(matrix, Mapping) and matrix.get("profile") == profile:
            return matrix
    first = matrices[0] if matrices else {}
    return first if isinstance(first, Mapping) else {}


def _rank_args(
    *, profile: str | None, seed: Sequence[str], limit: int | None
) -> argparse.Namespace:
    return argparse.Namespace(profile=profile, seed=list(seed), limit=limit)


def _packet_with_cache_mode(
    packet: Mapping[str, object], cache_mode: str
) -> Mapping[str, object]:
    if cache_mode == "packet":
        return packet
    mutable = json.loads(json.dumps(packet))
    mutable["cache"] = {"enabled": cache_mode == "enabled"}
    return mutable


def _render_text(packet: Mapping[str, object]) -> str:
    duration = packet["durationMs"]
    metrics = packet["lastAlgorithmMetrics"]
    cache_counts = packet.get("cacheStatusCounts")
    if not isinstance(duration, Mapping) or not isinstance(metrics, Mapping):
        raise SystemExit("invalid graph turbo benchmark packet")
    return (
        "[graph-benchmark] "
        f"profile={packet['profile']} runs={packet['runs']} "
        f"warmup={packet['warmupRuns']} cacheMode={packet['cacheMode']} "
        f"medianMs={duration['median']} p95Ms={duration['p95']} "
        f"cacheStatuses={_render_counts(cache_counts)}\n"
        "metrics="
        f"pathBackend={metrics.get('pathBackend')},"
        f"pathPairs={metrics.get('pathPairCount')},"
        f"pathCandidates={metrics.get('pathCandidateCount')},"
        f"pathFallbacks={metrics.get('pathFallbackCount')},"
        f"pprIterations={metrics.get('pprIterations')},"
        f"cache={metrics.get('cacheStatus')},"
        f"depthCache={metrics.get('depthCacheStatus')},"
        f"pprCache={metrics.get('pprCacheStatus')},"
        f"reachableEdgesCache={metrics.get('reachableEdgesCacheStatus')}"
    )


def _render_counts(counts: object) -> str:
    if not isinstance(counts, Mapping):
        return "-"
    return ",".join(f"{key}:{value}" for key, value in sorted(counts.items()))


def _parse_args(argv: Sequence[str] | None) -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("packet", nargs="?", default="-")
    parser.add_argument("--runs", type=_positive_int, default=30)
    parser.add_argument("--warmup-runs", type=_non_negative_int, default=3)
    parser.add_argument(
        "--cache-mode",
        choices=["packet", "enabled", "disabled"],
        default="packet",
    )
    parser.add_argument("--profile", default=None)
    parser.add_argument("--seed", action="append", default=[])
    parser.add_argument("--limit", type=int, default=None)
    parser.add_argument("--format", choices=["json", "text"], default="json")
    return parser.parse_args(argv)


def _positive_int(value: str) -> int:
    parsed = int(value)
    if parsed < 1:
        raise argparse.ArgumentTypeError("value must be positive")
    return parsed


def _non_negative_int(value: str) -> int:
    parsed = int(value)
    if parsed < 0:
        raise argparse.ArgumentTypeError("value must be non-negative")
    return parsed


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))
