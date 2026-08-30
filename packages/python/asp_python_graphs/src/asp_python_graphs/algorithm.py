"""Private Graph-Turbo algorithm API used by the ASP Server service.

This module deliberately has no command-line entry point.  Runtime callers
submit schema-owned packets to ``service_cli`` over the server-owned gRPC
session; offline evidence tools may import these pure helpers without
becoming a second lifecycle or authority.
"""

from __future__ import annotations

import json
from collections.abc import Mapping, Sequence
from dataclasses import dataclass
from pathlib import Path

from .calibration import apply_profile_calibrations
from .constants import ALGORITHM_ID
from .feedback import merge_feedback_into_packet
from .model import GraphProfile, GraphResult, TypedGraph
from .profiles import resolve_profile
from .ranking import rank_frontier


@dataclass(frozen=True)
class RankOptions:
    """Optional algorithm controls supplied by an offline evidence caller."""

    profile: str | None = None
    seed: tuple[str, ...] = ()
    limit: int | None = None
    feedback: tuple[str, ...] = ()
    calibration: tuple[str, ...] = ()


def rank_packet(
    packet: Mapping[str, object], options: object | None = None
) -> GraphResult:
    """Rank one validated Graph-Turbo packet without parsing a CLI."""

    _validate_algorithm(packet)
    feedback_paths = _string_paths(options, "feedback")
    if feedback_paths:
        mutable_packet = dict(packet)
        packet = merge_feedback_into_packet(
            mutable_packet,
            [_load_feedback_packet(path) for path in feedback_paths],
        )
    profile_override = _option(options, "profile")
    profile = profile_override or _string_packet_field(packet, "profile", "owner-query")
    calibration_paths = _string_paths(options, "calibration")
    selected_profile = apply_profile_calibrations(
        resolve_profile(profile),
        [_load_calibration_packet(path) for path in calibration_paths],
    )
    seed_override = _string_paths(options, "seed")
    seeds = seed_override or _string_list_packet_field(packet, "seedIds")
    limit_override = _option(options, "limit")
    controls = dict(packet)
    controls["seedIds"] = seeds
    controls["budget"] = (
        limit_override
        if isinstance(limit_override, int)
        else _positive_int_packet_field(packet, "budget", 8)
    )
    graph = TypedGraph.from_packet(packet)
    return rank_graph(
        graph,
        controls,
        profile=selected_profile,
    )


def rank_graph(
    graph: TypedGraph,
    controls: Mapping[str, object],
    *,
    profile: str | GraphProfile = "owner-query",
) -> GraphResult:
    """Rank an already-admitted graph from one schema-owned control mapping."""

    selected_profile = resolve_profile(profile)
    window_merge = _window_merge_packet_field(controls)
    return rank_frontier(
        graph,
        profile=selected_profile,
        seeds=_string_list_packet_field(controls, "seedIds"),
        limit=_positive_int_packet_field(controls, "budget", 8),
        kind_budgets=_kind_budgets_packet_field(controls),
        window_merge_enabled=window_merge["enabled"],
        window_merge_max_gap_lines=window_merge["maxGapLines"],
        path_budget=_positive_int_packet_field(controls, "pathBudget", 4),
        path_max_hops=_positive_int_packet_field(controls, "pathMaxHops", 4),
        cache_enabled=_cache_enabled_packet_field(controls),
        seen_selectors=_read_memory_seen_selectors(controls),
        query_clauses=_string_list_packet_field(controls, "queryClauses"),
        query_adjustment_policy=_query_adjustment_policy_packet_field(controls),
    )


def load_packet(path: str) -> Mapping[str, object]:
    """Load a packet for an offline evidence tool, never for Runtime admission."""

    if path == "-":
        import sys

        packet = json.load(sys.stdin)
    else:
        packet = json.loads(Path(path).read_text(encoding="utf-8"))
    if not isinstance(packet, Mapping):
        raise SystemExit("graph turbo packet must be a JSON object")
    return packet


def _load_feedback_packet(path: str) -> Mapping[str, object]:
    packet = load_packet(path)
    if packet.get("schemaId") != "agent.semantic-protocols.semantic-graph-turbo-feedback":
        raise SystemExit(f"unsupported graph turbo feedback packet: {path}")
    return packet


def _load_calibration_packet(path: str) -> Mapping[str, object]:
    packet = load_packet(path)
    if packet.get("schemaId") != "agent.semantic-protocols.semantic-graph-turbo-calibration":
        raise SystemExit(f"unsupported graph turbo calibration packet: {path}")
    return packet


def _option(options: object | None, name: str) -> object | None:
    return getattr(options, name, None) if options is not None else None


def _string_paths(options: object | None, name: str) -> list[str]:
    value = _option(options, name)
    if value is None:
        return []
    if isinstance(value, str):
        return [value]
    if isinstance(value, Sequence) and not isinstance(value, (bytes, bytearray)):
        if not all(isinstance(item, str) for item in value):
            raise SystemExit(f"graph turbo {name} must be a string sequence")
        return list(value)
    raise SystemExit(f"graph turbo {name} must be a string sequence")


def _query_adjustment_policy_packet_field(
    packet: Mapping[str, object],
) -> Mapping[str, bool]:
    policy = packet.get("queryAdjustmentPolicy")
    if not isinstance(policy, Mapping):
        return {}
    return {
        key: value
        for key, value in policy.items()
        if isinstance(key, str) and isinstance(value, bool)
    }


def _validate_algorithm(packet: Mapping[str, object]) -> None:
    algorithm = packet.get("algorithm")
    if algorithm is not None and algorithm != ALGORITHM_ID:
        raise SystemExit(f"unsupported graph turbo algorithm: {algorithm}")


def _string_packet_field(packet: Mapping[str, object], name: str, default: str) -> str:
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
    if not isinstance(value, int) or isinstance(value, bool) or value < 1:
        raise SystemExit(f"graph turbo {name} must be a positive integer")
    return value


def _kind_budgets_packet_field(packet: Mapping[str, object]) -> dict[str, int]:
    value = packet.get("kindBudgets", {})
    if not isinstance(value, Mapping):
        raise SystemExit("graph turbo kindBudgets must be an object")
    budgets: dict[str, int] = {}
    for kind, budget in value.items():
        if not isinstance(kind, str) or not isinstance(budget, int) or isinstance(budget, bool) or budget < 1:
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
    if not isinstance(max_gap_lines, int) or isinstance(max_gap_lines, bool) or max_gap_lines < 0:
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


def _read_memory_seen_selectors(packet: Mapping[str, object]) -> list[str]:
    value = packet.get("readMemory", {})
    if not isinstance(value, Mapping):
        raise SystemExit("graph turbo readMemory must be an object")
    selectors = value.get("seenSelectors", [])
    if not isinstance(selectors, list) or not all(isinstance(selector, str) for selector in selectors):
        raise SystemExit("graph turbo readMemory.seenSelectors must be a string array")
    return selectors
