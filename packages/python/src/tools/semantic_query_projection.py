"""Cross-field validators for semantic query projection packets."""

from __future__ import annotations

import re
from collections import Counter
from typing import Any

_LAYOUT_PUNCTUATION_ONLY = re.compile(r"^\s*[{}()[\],;]+\s*$")


def semantic_query_projection_errors(packet: dict[str, Any]) -> list[str]:
    return [
        *projection_uniqueness_errors(packet),
        *projection_rendered_row_errors(packet),
        *compact_code_layout_punctuation_errors(packet),
    ]


def compact_code_layout_punctuation_errors(packet: dict[str, Any]) -> list[str]:
    errors: list[str] = []
    for match_index, match in enumerate(packet.get("matches", [])):
        projection = match.get("projection") if isinstance(match, dict) else None
        if not isinstance(projection, dict) or projection.get("mode") != "compact":
            continue
        code = match.get("code")
        if not isinstance(code, str):
            continue
        errors.extend(
            f"matches[{match_index}].code {error}"
            for error in compact_code_text_layout_errors(code)
        )
    return errors


def compact_code_text_layout_errors(code: str) -> list[str]:
    errors: list[str] = []
    for line_number, line in enumerate(code.splitlines(), start=1):
        if _LAYOUT_PUNCTUATION_ONLY.fullmatch(line):
            errors.append(f"line {line_number} is punctuation-only compact residue")
    return errors


def projection_rendered_row_errors(packet: dict[str, Any]) -> list[str]:
    errors: list[str] = []
    for match_index, match in enumerate(packet.get("matches", [])):
        if not isinstance(match, dict):
            continue
        projection = match.get("projection")
        if not isinstance(projection, dict) or projection.get("mode") != "compact":
            continue
        rows = projection.get("renderedRows")
        code = match.get("code")
        if rows is None and isinstance(code, str):
            errors.append(
                f"matches[{match_index}].projection compact code lacks renderedRows"
            )
            continue
        if rows is None:
            continue
        if not isinstance(rows, list):
            errors.append(f"matches[{match_index}].projection renderedRows must be a list")
            continue
        nodes = projection.get("nodes", [])
        node_id_set = {
            node.get("id") for node in nodes if isinstance(node, dict) and "id" in node
        }
        row_texts: list[str] = []
        row_node_ids: list[object] = []
        for row_index, row in enumerate(rows):
            if not isinstance(row, dict):
                errors.append(
                    f"matches[{match_index}].projection.renderedRows[{row_index}] "
                    "must be an object"
                )
                continue
            node_id = row.get("nodeId")
            text = row.get("text")
            if node_id not in node_id_set:
                errors.append(
                    f"matches[{match_index}].projection.renderedRows[{row_index}] "
                    f"nodeId {node_id} does not exist"
                )
            else:
                row_node_ids.append(node_id)
            if not isinstance(text, str) or not text.strip():
                errors.append(
                    f"matches[{match_index}].projection.renderedRows[{row_index}] "
                    "text must be non-empty"
                )
                continue
            if _LAYOUT_PUNCTUATION_ONLY.fullmatch(text):
                errors.append(
                    f"matches[{match_index}].projection.renderedRows[{row_index}] "
                    "text is punctuation-only compact residue"
                )
            row_texts.append(text)
        if isinstance(code, str) and "\n".join(row_texts) != code:
            errors.append(
                f"matches[{match_index}].projection renderedRows text does not match code"
            )
        rendered_node_ids = projection.get("renderedNodeIds")
        if isinstance(rendered_node_ids, list) and row_node_ids != rendered_node_ids:
            errors.append(
                f"matches[{match_index}].projection renderedRows node sequence "
                "does not match renderedNodeIds"
            )
    return errors


def projection_uniqueness_errors(packet: dict[str, Any]) -> list[str]:
    errors: list[str] = []
    for match_index, match in enumerate(packet.get("matches", [])):
        if not isinstance(match, dict):
            continue
        projection = match.get("projection")
        if not isinstance(projection, dict):
            continue
        if projection.get("mode") == "compact":
            _append_compact_identity_errors(errors, match_index, match, projection)

        nodes = projection.get("nodes", [])
        if not isinstance(nodes, list):
            continue
        node_ids = [node.get("id") for node in nodes if isinstance(node, dict)]
        node_id_set = set(node_ids)
        for node_id, count in Counter(node_ids).items():
            if count > 1:
                errors.append(f"matches[{match_index}].projection duplicate node id {node_id}")

        errors.extend(_parent_id_errors(match_index, nodes, node_id_set))
        errors.extend(_rendered_node_id_errors(match_index, projection, node_id_set))
        errors.extend(_omission_errors(match_index, projection, node_id_set))
        errors.extend(_expand_action_errors(match_index, projection, node_id_set))

    return errors


def _append_compact_identity_errors(
    errors: list[str],
    match_index: int,
    match: dict[str, Any],
    projection: dict[str, Any],
) -> None:
    match_read = match.get("read")
    exact_read = projection.get("exactRead")
    source_fingerprint = projection.get("sourceFingerprint")
    if exact_read != match_read:
        errors.append(
            f"matches[{match_index}].projection exactRead {exact_read} "
            f"does not match read locator {match_read}"
        )
    if isinstance(source_fingerprint, str) and isinstance(exact_read, str):
        if exact_read not in source_fingerprint:
            errors.append(
                f"matches[{match_index}].projection sourceFingerprint "
                "does not include exactRead locator"
            )


def _parent_id_errors(
    match_index: int,
    nodes: list[object],
    node_id_set: set[object],
) -> list[str]:
    errors: list[str] = []
    for node in nodes:
        parent_id = node.get("parentId") if isinstance(node, dict) else None
        if parent_id is not None and parent_id not in node_id_set:
            node_id = node.get("id") if isinstance(node, dict) else None
            errors.append(
                f"matches[{match_index}].projection node {node_id} "
                f"references missing parentId {parent_id}"
            )
    return errors


def _rendered_node_id_errors(
    match_index: int,
    projection: dict[str, Any],
    node_id_set: set[object],
) -> list[str]:
    errors: list[str] = []
    rendered_node_ids = projection.get("renderedNodeIds", [])
    if not isinstance(rendered_node_ids, list):
        return errors
    for node_id, count in Counter(rendered_node_ids).items():
        if count > 1:
            errors.append(
                f"matches[{match_index}].projection duplicate rendered node id {node_id}"
            )
        if node_id not in node_id_set:
            errors.append(
                f"matches[{match_index}].projection rendered node id {node_id} "
                "does not exist"
            )
    return errors


def _omission_errors(
    match_index: int,
    projection: dict[str, Any],
    node_id_set: set[object],
) -> list[str]:
    errors: list[str] = []
    omitted = projection.get("omitted", [])
    if not isinstance(omitted, list):
        return errors
    for omission in omitted:
        node_id = omission.get("nodeId") if isinstance(omission, dict) else None
        has_read = isinstance(omission, dict) and "read" in omission
        if node_id is None and not has_read:
            errors.append(f"matches[{match_index}].projection omitted fact lacks nodeId/read")
        if node_id is not None and node_id not in node_id_set:
            errors.append(
                f"matches[{match_index}].projection omitted fact references "
                f"missing node id {node_id}"
            )
    return errors


def _expand_action_errors(
    match_index: int,
    projection: dict[str, Any],
    node_id_set: set[object],
) -> list[str]:
    errors: list[str] = []
    expand_actions = projection.get("expandActions", [])
    if not isinstance(expand_actions, list):
        return errors
    for action in expand_actions:
        if not isinstance(action, dict):
            continue
        action_kind = action.get("kind")
        target = action.get("target")
        has_read = "read" in action
        if action_kind == "node-query" and target not in node_id_set:
            errors.append(
                f"matches[{match_index}].projection node-query target {target} "
                "does not exist"
            )
        elif target not in node_id_set and not has_read:
            errors.append(
                f"matches[{match_index}].projection expand action target {target} "
                "is neither a node id nor an exact read action"
            )
        errors.extend(_expand_action_selector_errors(match_index, action_kind, action))
    return errors


def _expand_action_selector_errors(
    match_index: int,
    action_kind: object,
    action: dict[str, Any],
) -> list[str]:
    argv = action.get("argv")
    if (
        action_kind not in {"exact-read", "hot-block"}
        or not isinstance(argv, list)
        or "--selector" not in argv
    ):
        return []
    selector_index = argv.index("--selector")
    selector = argv[selector_index + 1] if selector_index + 1 < len(argv) else None
    if selector == action.get("read"):
        return []
    return [
        f"matches[{match_index}].projection {action_kind} argv selector "
        f"{selector} does not match read locator {action.get('read')}"
    ]
