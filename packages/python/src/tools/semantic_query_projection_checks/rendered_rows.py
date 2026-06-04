"""Rendered row validators for compact semantic query projections."""

from __future__ import annotations

from collections.abc import Iterable

from .layout import is_layout_punctuation_only


def projection_rendered_row_errors(packet: dict[str, object]) -> list[str]:
    matches = packet.get("matches", [])
    if not isinstance(matches, Iterable):
        return []
    return [
        error
        for match_index, match in enumerate(matches)
        if isinstance(match, dict)
        for error in _compact_match_row_errors(match_index, match)
    ]


def _compact_match_row_errors(match_index: int, match: dict[object, object]) -> list[str]:
    projection = match.get("projection")
    if not isinstance(projection, dict) or projection.get("mode") != "compact":
        return []
    rows = projection.get("renderedRows")
    code = match.get("code")
    if rows is None and isinstance(code, str):
        return [f"matches[{match_index}].projection compact code lacks renderedRows"]
    if rows is None:
        return []
    if not isinstance(rows, list):
        return [f"matches[{match_index}].projection renderedRows must be a list"]
    return _validated_rows_errors(match_index, projection, rows, code)


def _validated_rows_errors(
    match_index: int,
    projection: dict[object, object],
    rows: list[object],
    code: object,
) -> list[str]:
    node_id_set = _projection_node_ids(projection)
    row_result = _row_result(match_index, rows, node_id_set)
    errors = list(row_result["errors"])
    if isinstance(code, str) and "\n".join(row_result["texts"]) != code:
        errors.append(
            f"matches[{match_index}].projection renderedRows text does not match code"
        )
    rendered_node_ids = projection.get("renderedNodeIds")
    if isinstance(rendered_node_ids, list) and row_result["node_ids"] != rendered_node_ids:
        errors.append(
            f"matches[{match_index}].projection renderedRows node sequence "
            "does not match renderedNodeIds"
        )
    return errors


def _row_result(
    match_index: int,
    rows: list[object],
    node_id_set: set[object],
) -> dict[str, list[object]]:
    errors: list[object] = []
    texts: list[object] = []
    row_node_ids: list[object] = []
    for row_index, row in enumerate(rows):
        row_errors, row_text, node_id = _single_row_errors(
            match_index, row_index, row, node_id_set
        )
        errors.extend(row_errors)
        if row_text is not None:
            texts.append(row_text)
        if node_id is not None:
            row_node_ids.append(node_id)
    return {"errors": errors, "texts": texts, "node_ids": row_node_ids}


def _single_row_errors(
    match_index: int,
    row_index: int,
    row: object,
    node_id_set: set[object],
) -> tuple[list[str], str | None, object | None]:
    if not isinstance(row, dict):
        return [
            f"matches[{match_index}].projection.renderedRows[{row_index}] "
            "must be an object"
        ], None, None
    errors = _row_node_errors(match_index, row_index, row.get("nodeId"), node_id_set)
    text_errors, row_text = _row_text_errors(match_index, row_index, row.get("text"))
    return [*errors, *text_errors], row_text, row.get("nodeId") if not errors else None


def _row_node_errors(
    match_index: int,
    row_index: int,
    node_id: object,
    node_id_set: set[object],
) -> list[str]:
    if node_id in node_id_set:
        return []
    return [
        f"matches[{match_index}].projection.renderedRows[{row_index}] "
        f"nodeId {node_id} does not exist"
    ]


def _row_text_errors(
    match_index: int,
    row_index: int,
    text: object,
) -> tuple[list[str], str | None]:
    if not isinstance(text, str) or not text.strip():
        return [
            f"matches[{match_index}].projection.renderedRows[{row_index}] "
            "text must be non-empty"
        ], None
    if is_layout_punctuation_only(text):
        return [
            f"matches[{match_index}].projection.renderedRows[{row_index}] "
            "text is punctuation-only compact residue"
        ], text
    return [], text


def _projection_node_ids(projection: dict[object, object]) -> set[object]:
    nodes = projection.get("nodes", [])
    if not isinstance(nodes, list):
        return set()
    return {node.get("id") for node in nodes if isinstance(node, dict) and "id" in node}
