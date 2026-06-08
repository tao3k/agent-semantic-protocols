"""Apply graph-turbo calibration packets to profile weights."""

from __future__ import annotations

from collections.abc import Iterable, Mapping
from dataclasses import replace
from typing import Any

from .model import GraphProfile


def apply_profile_calibrations(
    profile: GraphProfile,
    calibration_packets: Iterable[Mapping[str, Any]],
) -> GraphProfile:
    """Return a copy of profile with schema-owned calibration deltas applied."""

    kind_bonus = dict(profile.kind_bonus)
    relation_multiplier = dict(profile.relation_weight_multiplier)
    for packet in _profile_calibration_packets(profile.name, calibration_packets):
        _apply_packet_kind_deltas(kind_bonus, packet)
        _apply_packet_relation_deltas(relation_multiplier, packet)
    return replace(
        profile,
        kind_bonus=kind_bonus,
        relation_weight_multiplier=relation_multiplier,
    )


def _profile_calibration_packets(
    profile_name: str,
    calibration_packets: Iterable[Mapping[str, Any]],
) -> Iterable[Mapping[str, Any]]:
    return (
        packet for packet in calibration_packets if _packet_profile(packet) == profile_name
    )


def _apply_packet_kind_deltas(
    kind_bonus: dict[str, float],
    packet: Mapping[str, Any],
) -> None:
    entries = (_kind_delta_entry(entry) for entry in _mapping_entries(packet, "kindDeltas"))
    for kind, score_delta in filter(None, entries):
        kind_bonus[kind] = kind_bonus.get(kind, 0.0) + score_delta


def _kind_delta_entry(entry: Mapping[str, Any]) -> tuple[str, float] | None:
    kind = entry.get("kind")
    if not isinstance(kind, str):
        return None
    return kind, _number(entry.get("scoreDelta"))


def _apply_packet_relation_deltas(
    relation_multiplier: dict[str, float],
    packet: Mapping[str, Any],
) -> None:
    entries = (
        _relation_delta_entry(entry)
        for entry in _mapping_entries(packet, "relationDeltas")
    )
    for relation, weight_delta in filter(None, entries):
        relation_multiplier[relation] = max(
            0.05,
            relation_multiplier.get(relation, 1.0) + weight_delta,
        )


def _relation_delta_entry(entry: Mapping[str, Any]) -> tuple[str, float] | None:
    relation = entry.get("relation")
    if not isinstance(relation, str):
        return None
    return relation, _number(entry.get("weightMultiplierDelta"))


def _packet_profile(packet: Mapping[str, Any]) -> str | None:
    profile = packet.get("profile")
    return profile if isinstance(profile, str) else None


def _mapping_entries(
    packet: Mapping[str, Any],
    name: str,
) -> tuple[Mapping[str, Any], ...]:
    value = packet.get(name, [])
    if not isinstance(value, list):
        return ()
    return tuple(item for item in value if isinstance(item, Mapping))


def _number(value: object) -> float:
    return float(value) if isinstance(value, int | float) else 0.0
