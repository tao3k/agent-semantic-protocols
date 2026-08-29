"""Selector normalization helpers for graph turbo nodes."""

from __future__ import annotations

from collections.abc import Mapping
from dataclasses import dataclass

from .model import Node


@dataclass(frozen=True)
class GraphTurboSelectorRange:
    path: str
    start_line: int
    end_line: int


def graph_turbo_selector_for_node(node: Node) -> str | None:
    fields = node.fields.get("fields")
    if isinstance(fields, Mapping):
        context_locator = fields.get("contextLocator")
        if isinstance(context_locator, str) and context_locator:
            return context_locator
        locator = fields.get("locator") or fields.get("location")
        if isinstance(locator, str) and locator:
            return locator
    locator = node.fields.get("locator") or node.fields.get("location")
    if isinstance(locator, str) and locator:
        return locator
    node_range = graph_turbo_node_range(node)
    if node_range is not None:
        return f"{node_range.path}:{node_range.start_line}:{node_range.end_line}"
    return None


def graph_turbo_node_range(node: Node) -> GraphTurboSelectorRange | None:
    selector = graph_turbo_selector_for_node_without_range(node)
    if selector is not None and (parsed := graph_turbo_parse_selector(selector)):
        return parsed
    return graph_turbo_range_from_fields(node.fields)


def graph_turbo_selector_for_node_without_range(node: Node) -> str | None:
    fields = node.fields.get("fields")
    if isinstance(fields, Mapping):
        context_locator = fields.get("contextLocator")
        if isinstance(context_locator, str) and context_locator:
            return context_locator
        locator = fields.get("locator") or fields.get("location")
        if isinstance(locator, str) and locator:
            return locator
    locator = node.fields.get("locator") or node.fields.get("location")
    if isinstance(locator, str) and locator:
        return locator
    return None


def graph_turbo_owner_path_for_node(node: Node) -> str | None:
    owner = node.fields.get("ownerPath") or node.fields.get("owner")
    if isinstance(owner, str) and owner:
        return owner
    path = node.fields.get("path")
    if isinstance(path, str) and path:
        return path
    node_range = graph_turbo_node_range(node)
    return node_range.path if node_range is not None else None


def graph_turbo_range_from_fields(
    fields: Mapping[str, object],
) -> GraphTurboSelectorRange | None:
    path = fields.get("path")
    start = fields.get("startLine") or fields.get("start")
    end = fields.get("endLine") or fields.get("end")
    if isinstance(path, str) and isinstance(start, int) and isinstance(end, int):
        return GraphTurboSelectorRange(path, start, end)
    return None


def graph_turbo_parse_selector(selector: str) -> GraphTurboSelectorRange | None:
    parsed = _parse_colon_range(selector)
    if parsed is not None:
        return parsed
    return _parse_dash_range(selector)


def graph_turbo_ranges_overlap(
    left: GraphTurboSelectorRange, right: GraphTurboSelectorRange
) -> bool:
    return (
        left.path == right.path
        and left.start_line <= right.end_line
        and right.start_line <= left.end_line
    )


def graph_turbo_ranges_adjacent(
    left: GraphTurboSelectorRange, right: GraphTurboSelectorRange, *, max_gap_lines: int
) -> bool:
    return (
        left.path == right.path
        and left.start_line <= right.end_line + max_gap_lines
        and right.start_line <= left.end_line + max_gap_lines
    )


def _parse_colon_range(selector: str) -> GraphTurboSelectorRange | None:
    path, sep, end_text = selector.rpartition(":")
    if not sep:
        return None
    path, sep, start_text = path.rpartition(":")
    if not sep:
        return None
    return _parsed_range(path, start_text, end_text)


def _parse_dash_range(selector: str) -> GraphTurboSelectorRange | None:
    path, sep, range_text = selector.rpartition(":")
    if not sep:
        return None
    start_text, sep, end_text = range_text.partition("-")
    if not sep:
        return None
    return _parsed_range(path, start_text, end_text)


def _parsed_range(
    path: str, start_text: str, end_text: str
) -> GraphTurboSelectorRange | None:
    try:
        start = int(start_text)
        end = int(end_text)
    except ValueError:
        return None
    if not path or end < start:
        return None
    return GraphTurboSelectorRange(path, start, end)
