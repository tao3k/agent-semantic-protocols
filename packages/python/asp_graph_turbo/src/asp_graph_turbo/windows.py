"""Window merge helpers for graph turbo response evidence."""

from __future__ import annotations

from collections.abc import Iterable

from .model import MergedWindow, Node


def merge_ranked_windows(
    ranked_nodes: Iterable[Node], *, enabled: bool, max_gap_lines: int
) -> tuple[MergedWindow, ...]:
    if not enabled:
        return ()
    windows = [
        window
        for node in ranked_nodes
        if node.kind == "window" and (window := _window_from_node(node)) is not None
    ]
    windows.sort(key=lambda window: (window.path, window.start_line, window.end_line))
    merged: list[MergedWindow] = []
    for window in windows:
        if not merged or not _can_merge(merged[-1], window, max_gap_lines):
            merged.append(window)
            continue
        previous = merged[-1]
        merged[-1] = MergedWindow(
            path=previous.path,
            start_line=min(previous.start_line, window.start_line),
            end_line=max(previous.end_line, window.end_line),
            node_ids=(*previous.node_ids, *window.node_ids),
        )
    return tuple(merged)


def _window_from_node(node: Node) -> MergedWindow | None:
    path = _string_field(node, "path")
    start_line = _int_field(node, "startLine", "start")
    end_line = _int_field(node, "endLine", "end")
    if path is None or start_line is None or end_line is None:
        return None
    if end_line < start_line:
        return None
    return MergedWindow(path=path, start_line=start_line, end_line=end_line, node_ids=(node.id,))


def _can_merge(previous: MergedWindow, current: MergedWindow, max_gap_lines: int) -> bool:
    return previous.path == current.path and current.start_line <= previous.end_line + max_gap_lines


def _string_field(node: Node, name: str) -> str | None:
    value = node.fields.get(name)
    return value if isinstance(value, str) and value else None


def _int_field(node: Node, *names: str) -> int | None:
    for name in names:
        value = node.fields.get(name)
        if isinstance(value, int):
            return value
    return None
