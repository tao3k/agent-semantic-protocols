"""Graph identity validators for semantic query projections."""

from __future__ import annotations

from collections import Counter
from collections.abc import Iterable


def projection_uniqueness_errors(packet: dict[str, object]) -> list[str]:
    matches = packet.get("matches", [])
    if not isinstance(matches, Iterable):
        return []
    return [
        error
        for match_index, match in enumerate(matches)
        if isinstance(match, dict)
        for error in _projection_errors(match_index, match)
    ]


def _projection_errors(match_index: int, match: dict[object, object]) -> list[str]:
    projection = match.get("projection")
    if not isinstance(projection, dict):
        return []
    nodes = projection.get("nodes", [])
    if not isinstance(nodes, list):
        return _compact_identity_errors(match_index, match, projection)
    node_ids = [node.get("id") for node in nodes if isinstance(node, dict)]
    node_id_set = set(node_ids)
    return [
        *_compact_identity_errors(match_index, match, projection),
        *_duplicate_node_id_errors(match_index, node_ids),
        *_parent_id_errors(match_index, nodes, node_id_set),
        *_rendered_node_id_errors(match_index, projection, node_id_set),
        *_omission_errors(match_index, projection, node_id_set),
        *_expand_action_errors(match_index, projection, node_id_set),
    ]


def _compact_identity_errors(
    match_index: int,
    match: dict[object, object],
    projection: dict[object, object],
) -> list[str]:
    if projection.get("mode") != "compact":
        return []
    match_read = match.get("read")
    exact_read = projection.get("exactRead")
    source_fingerprint = projection.get("sourceFingerprint")
    errors = []
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
    return errors


def _duplicate_node_id_errors(match_index: int, node_ids: list[object]) -> list[str]:
    return [
        f"matches[{match_index}].projection duplicate node id {node_id}"
        for node_id, count in Counter(node_ids).items()
        if count > 1
    ]


def _parent_id_errors(
    match_index: int,
    nodes: list[object],
    node_id_set: set[object],
) -> list[str]:
    return [
        f"matches[{match_index}].projection node {node.get('id')} "
        f"references missing parentId {node.get('parentId')}"
        for node in nodes
        if isinstance(node, dict)
        and node.get("parentId") is not None
        and node.get("parentId") not in node_id_set
    ]


def _rendered_node_id_errors(
    match_index: int,
    projection: dict[object, object],
    node_id_set: set[object],
) -> list[str]:
    rendered_node_ids = projection.get("renderedNodeIds", [])
    if not isinstance(rendered_node_ids, list):
        return []
    return [
        error
        for node_id, count in Counter(rendered_node_ids).items()
        for error in _rendered_node_id_error(match_index, node_id, count, node_id_set)
    ]


def _rendered_node_id_error(
    match_index: int,
    node_id: object,
    count: int,
    node_id_set: set[object],
) -> list[str]:
    errors = []
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
    projection: dict[object, object],
    node_id_set: set[object],
) -> list[str]:
    omitted = projection.get("omitted", [])
    if not isinstance(omitted, list):
        return []
    return [
        error
        for omission in omitted
        for error in _single_omission_errors(match_index, omission, node_id_set)
    ]


def _single_omission_errors(
    match_index: int,
    omission: object,
    node_id_set: set[object],
) -> list[str]:
    node_id = omission.get("nodeId") if isinstance(omission, dict) else None
    has_read = isinstance(omission, dict) and "read" in omission
    errors = []
    if node_id is None and not has_read:
        errors.append(
            f"matches[{match_index}].projection omitted fact lacks nodeId/read"
        )
    if node_id is not None and node_id not in node_id_set:
        errors.append(
            f"matches[{match_index}].projection omitted fact references "
            f"missing node id {node_id}"
        )
    return errors


def _expand_action_errors(
    match_index: int,
    projection: dict[object, object],
    node_id_set: set[object],
) -> list[str]:
    expand_actions = projection.get("expandActions", [])
    if not isinstance(expand_actions, list):
        return []
    return [
        error
        for action in expand_actions
        if isinstance(action, dict)
        for error in _single_expand_action_errors(match_index, action, node_id_set)
    ]


def _single_expand_action_errors(
    match_index: int,
    action: dict[object, object],
    node_id_set: set[object],
) -> list[str]:
    action_kind = action.get("kind")
    target = action.get("target")
    has_read = "read" in action
    errors = []
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
    return errors
