"""Target extraction from cached semantic search packets."""

from __future__ import annotations

from collections.abc import Iterable, Mapping
from dataclasses import dataclass
from typing import Any


@dataclass(frozen=True)
class PacketTargets:
    owners: tuple[str, ...]
    tests: tuple[str, ...]
    dependencies: tuple[str, ...]
    items: tuple[str, ...]
    query: str
    profile: str


def packet_targets(packet: Mapping[str, Any]) -> PacketTargets:
    owners = _owner_targets(packet)
    items = _item_targets(packet)
    return PacketTargets(
        owners=owners,
        tests=_test_targets(packet),
        dependencies=_dependency_targets(packet),
        items=items,
        query=str(packet.get("query") or ""),
        profile=_profile_for_packet(packet, owners=owners, items=items),
    )


def search_packet_input_targets(packet: Mapping[str, Any]) -> tuple[str, ...]:
    synthesis = _synthesis(packet)
    values: list[str] = []
    values.extend(_owner_targets(packet))
    values.extend(_test_targets(packet))
    values.extend(_dependency_targets(packet))
    values.extend(_item_targets(packet))
    values.extend(_targets(synthesis.get("seeds")))
    values.extend(_targets(synthesis.get("windowSet")))
    return tuple(value for value in values if value)


def _profile_for_packet(
    packet: Mapping[str, Any], *, owners: tuple[str, ...], items: tuple[str, ...]
) -> str:
    method = str(packet.get("method") or "")
    if items:
        return "owner-query"
    if method == "search/owner" and owners:
        return "owner-tests"
    if method in {"search/prime", "search/workspace"}:
        return "prime"
    return "query-deps" if packet.get("query") else "prime"


def _owner_targets(packet: Mapping[str, Any]) -> tuple[str, ...]:
    synthesis = _synthesis(packet)
    values: list[str] = []
    values.extend(_owner_paths(packet.get("owners")))
    values.extend(_string_list(synthesis.get("highImpactOwners")))
    values.extend(_string_list(synthesis.get("frontierOwners")))
    values.extend(_string_list(synthesis.get("editFrontier")))
    values.extend(_targets_by_kind(synthesis.get("seeds"), {"owner", "package"}))
    values.extend(_targets_by_kind(synthesis.get("windowSet"), {"owner", "package"}))
    values.extend(_targets_by_kind(packet.get("nextActions"), {"owner", "package"}))
    return _unique(values)


def _test_targets(packet: Mapping[str, Any]) -> tuple[str, ...]:
    synthesis = _synthesis(packet)
    values: list[str] = []
    values.extend(_string_list(synthesis.get("testFrontier")))
    values.extend(_targets_by_kind(synthesis.get("seeds"), {"tests", "test"}))
    values.extend(_targets_by_kind(synthesis.get("windowSet"), {"tests", "test"}))
    values.extend(_targets_by_kind(packet.get("nextActions"), {"tests", "test"}))
    return _unique(values)


def _dependency_targets(packet: Mapping[str, Any]) -> tuple[str, ...]:
    values = _targets_by_kind(packet.get("nextActions"), {"deps", "dependency", "import"})
    return _unique(values)


def _item_targets(packet: Mapping[str, Any]) -> tuple[str, ...]:
    return _unique(_targets_by_kind(packet.get("nextActions"), {"text", "symbol"}))


def _synthesis(packet: Mapping[str, Any]) -> Mapping[str, Any]:
    value = packet.get("searchSynthesis")
    return value if isinstance(value, Mapping) else {}


def _targets_by_kind(items: Any, kinds: set[str]) -> list[str]:
    if not isinstance(items, list):
        return []
    return [
        target
        for item in items
        if isinstance(item, Mapping)
        and isinstance(kind := item.get("kind"), str)
        and kind in kinds
        and isinstance(target := item.get("target"), str)
        and target
    ]


def _targets(items: Any) -> list[str]:
    if not isinstance(items, list):
        return []
    return [
        target
        for item in items
        if isinstance(item, Mapping)
        and isinstance(target := item.get("target"), str)
        and target
    ]


def _owner_paths(items: Any) -> list[str]:
    if not isinstance(items, list):
        return []
    return [
        path
        for item in items
        if isinstance(item, Mapping)
        and isinstance(path := item.get("path"), str)
        and path
    ]


def _string_list(value: Any) -> list[str]:
    return [item for item in value if isinstance(item, str) and item] if isinstance(value, list) else []


def _unique(values: Iterable[str]) -> tuple[str, ...]:
    return tuple(dict.fromkeys(values))
