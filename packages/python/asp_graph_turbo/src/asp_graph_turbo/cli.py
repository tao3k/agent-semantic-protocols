"""CLI for ASP graph turbo."""

from __future__ import annotations

import argparse
import json
import sys
from collections.abc import Mapping
from pathlib import Path
from typing import Sequence

from .constants import ALGORITHM_ID
from .model import GraphResult, TypedGraph
from .turbo import DEFAULT_PROFILES, rank_frontier, render_compact, result_to_packet


def main(argv: Sequence[str] | None = None) -> int:
    args = _parse_args(argv)
    packet = _load_packet(args.packet)
    result = _rank_packet(packet, args)
    _write_result(result, args.format)
    return 0


def _rank_packet(packet: Mapping[str, object], args: argparse.Namespace) -> GraphResult:
    _validate_algorithm(packet)
    profile = args.profile or _string_packet_field(packet, "profile", "owner-query")
    seeds = args.seed or _string_list_packet_field(packet, "seedIds")
    limit = args.limit if args.limit is not None else _positive_int_packet_field(packet, "budget", 8)
    kind_budgets = _kind_budgets_packet_field(packet)
    window_merge = _window_merge_packet_field(packet)
    path_budget = _positive_int_packet_field(packet, "pathBudget", 4)
    path_max_hops = _positive_int_packet_field(packet, "pathMaxHops", 4)
    cache_enabled = _cache_enabled_packet_field(packet)
    graph = TypedGraph.from_packet(packet)
    return rank_frontier(
        graph,
        profile=profile,
        seeds=seeds,
        limit=limit,
        kind_budgets=kind_budgets,
        window_merge_enabled=window_merge["enabled"],
        window_merge_max_gap_lines=window_merge["maxGapLines"],
        path_budget=path_budget,
        path_max_hops=path_max_hops,
        cache_enabled=cache_enabled,
    )


def _write_result(result: GraphResult, output_format: str) -> None:
    if output_format == "json":
        sys.stdout.write(json.dumps(result_to_packet(result), sort_keys=True) + "\n")
    else:
        sys.stdout.write(render_compact(result))


def _parse_args(argv: Sequence[str] | None) -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "packet",
        nargs="?",
        default="-",
        help="JSON packet path, or '-' for stdin.",
    )
    parser.add_argument(
        "--profile",
        default=None,
        choices=sorted(DEFAULT_PROFILES),
    )
    parser.add_argument("--seed", action="append", default=[])
    parser.add_argument("--limit", type=int, default=None)
    parser.add_argument("--format", choices=["compact", "json"], default="compact")
    return parser.parse_args(argv)


def _load_packet(path: str) -> Mapping[str, object]:
    if path == "-":
        packet = json.load(sys.stdin)
    else:
        packet = json.loads(Path(path).read_text(encoding="utf-8"))
    if not isinstance(packet, Mapping):
        raise SystemExit("graph turbo packet must be a JSON object")
    return packet


def _validate_algorithm(packet: Mapping[str, object]) -> None:
    algorithm = packet.get("algorithm")
    if algorithm is not None and algorithm != ALGORITHM_ID:
        raise SystemExit(f"unsupported graph turbo algorithm: {algorithm}")


def _string_packet_field(
    packet: Mapping[str, object], name: str, default: str
) -> str:
    value = packet.get(name, default)
    if not isinstance(value, str) or not value:
        raise SystemExit(f"graph turbo {name} must be a non-empty string")
    return value


def _string_list_packet_field(packet: Mapping[str, object], name: str) -> list[str]:
    value = packet.get(name, [])
    if not isinstance(value, list) or not all(isinstance(item, str) for item in value):
        raise SystemExit(f"graph turbo {name} must be a string array")
    return value


def _positive_int_packet_field(
    packet: Mapping[str, object], name: str, default: int
) -> int:
    value = packet.get(name, default)
    if not isinstance(value, int) or value < 1:
        raise SystemExit(f"graph turbo {name} must be a positive integer")
    return value


def _kind_budgets_packet_field(packet: Mapping[str, object]) -> dict[str, int]:
    value = packet.get("kindBudgets", {})
    if not isinstance(value, Mapping):
        raise SystemExit("graph turbo kindBudgets must be an object")
    budgets: dict[str, int] = {}
    for kind, budget in value.items():
        if not isinstance(kind, str) or not isinstance(budget, int) or budget < 1:
            raise SystemExit("graph turbo kindBudgets values must be positive integers")
        budgets[kind] = budget
    return budgets


def _window_merge_packet_field(packet: Mapping[str, object]) -> dict[str, int | bool]:
    value = packet.get("windowMerge", {})
    if not isinstance(value, Mapping):
        raise SystemExit("graph turbo windowMerge must be an object")
    enabled = value.get("enabled", True)
    max_gap_lines = value.get("maxGapLines", 8)
    if not isinstance(enabled, bool):
        raise SystemExit("graph turbo windowMerge.enabled must be a boolean")
    if not isinstance(max_gap_lines, int) or max_gap_lines < 0:
        raise SystemExit("graph turbo windowMerge.maxGapLines must be a non-negative integer")
    return {"enabled": enabled, "maxGapLines": max_gap_lines}


def _cache_enabled_packet_field(packet: Mapping[str, object]) -> bool:
    value = packet.get("cache", {})
    if not isinstance(value, Mapping):
        raise SystemExit("graph turbo cache must be an object")
    enabled = value.get("enabled", True)
    if not isinstance(enabled, bool):
        raise SystemExit("graph turbo cache.enabled must be a boolean")
    return enabled


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))
