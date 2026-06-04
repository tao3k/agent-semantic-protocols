"""Projection expand action schema tests."""

from __future__ import annotations

from .support import semantic_query_minimal_packet, validation_errors


def test_projection_rejects_owner_name_routing_action() -> None:
    packet = semantic_query_minimal_packet()
    packet["matches"][0]["projection"] = _projection_with_actions(
        [
            {
                "kind": "owner-names",
                "target": "src/lib.rs",
                "reason": "return owner-local item names without code windows",
            }
        ]
    )

    assert validation_errors(packet) != []


def test_projection_exact_read_action_requires_read_locator() -> None:
    packet = semantic_query_minimal_packet()
    packet["matches"][0]["projection"] = _projection_with_actions(
        [
            {
                "kind": "exact-read",
                "target": "load",
                "reason": "read exact source before editing",
            }
        ]
    )

    assert validation_errors(packet) != []


def test_projection_hot_block_action_requires_read_locator() -> None:
    packet = semantic_query_minimal_packet()
    packet["matches"][0]["projection"] = _projection_with_actions(
        [
            {
                "kind": "hot-block",
                "target": "load:branch",
                "reason": "expand hot control-flow block",
            }
        ]
    )

    assert validation_errors(packet) != []


def test_projection_node_query_action_requires_argv() -> None:
    packet = semantic_query_minimal_packet()
    packet["matches"][0]["projection"] = _projection_with_actions(
        [
            {
                "kind": "node-query",
                "target": "load:branch",
                "reason": "expand node through provider query",
            }
        ]
    )

    assert validation_errors(packet) != []


def test_projection_node_query_action_rejects_empty_argv() -> None:
    packet = semantic_query_minimal_packet()
    packet["matches"][0]["projection"] = _projection_with_actions(
        [
            {
                "kind": "node-query",
                "target": "load:branch",
                "argv": [],
                "reason": "expand node through provider query",
            }
        ]
    )

    assert validation_errors(packet) != []


def test_projection_node_query_action_rejects_read_locator() -> None:
    packet = semantic_query_minimal_packet()
    packet["matches"][0]["projection"] = _projection_with_actions(
        [
            {
                "kind": "node-query",
                "target": "load:branch",
                "read": "src/lib.rs:6:6",
                "argv": ["rs-harness", "search", "owner", "src/lib.rs", "items", "."],
                "reason": "node queries must use provider argv, not read locators",
            }
        ]
    )

    assert validation_errors(packet) != []


def _projection_with_actions(actions: list[dict[str, object]]) -> dict[str, object]:
    return {
        "mode": "compact",
        "syntax": "semantic-outline",
        "sourceAuthority": "native-parser",
        "losslessStructure": True,
        "exactRead": "src/lib.rs:6:6",
        "expandActions": actions,
    }
