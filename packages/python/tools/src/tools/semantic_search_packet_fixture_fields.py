"""Extract selector identity fields from semantic-search-packet JSON."""

from __future__ import annotations

from typing import Any


SelectorField = tuple[str, str, str, str, bool]


def packet_selector_fields(packet: dict[str, Any]) -> list[SelectorField]:
    return [
        *item_selector_fields(packet.get("items", [])),
        *action_frontier_selector_fields(packet.get("actionFrontier", [])),
        *window_set_target_selector_fields(
            packet.get("routeGraph", {}).get("windowSetTargets", [])
        ),
    ]


def item_selector_fields(items: list[dict[str, Any]]) -> list[SelectorField]:
    return [
        field
        for index, item in enumerate(items)
        for field in _selector_group(
            item,
            f"items[{index}]",
            "structuralSelector",
        )
    ]


def action_frontier_selector_fields(actions: list[dict[str, Any]]) -> list[SelectorField]:
    return [
        field
        for index, action in enumerate(actions)
        for field in _selector_group(
            action,
            f"actionFrontier[{index}]",
            "selector",
        )
    ]


def window_set_target_selector_fields(targets: list[dict[str, Any]]) -> list[SelectorField]:
    return [
        field
        for index, target in enumerate(targets)
        for field in _selector_group(
            target,
            f"routeGraph.windowSetTargets[{index}]",
            "structuralSelector",
        )
    ]


def _selector_group(
    record: dict[str, Any],
    owner: str,
    executable_field: str,
) -> list[SelectorField]:
    return [
        (
            owner,
            executable_field,
            "executable-selector",
            f"{owner}.{executable_field}",
            bool(record.get(executable_field)),
        ),
        (
            owner,
            "displayLineRange",
            "display-only",
            f"{owner}.displayLineRange",
            bool(record.get("displayLineRange")),
        ),
        (
            owner,
            "sourceLocatorHint",
            "display-only",
            f"{owner}.sourceLocatorHint",
            bool(record.get("sourceLocatorHint")),
        ),
    ]
