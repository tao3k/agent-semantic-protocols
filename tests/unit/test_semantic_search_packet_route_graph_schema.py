from __future__ import annotations

import json
import unittest
from pathlib import Path
from typing import Any

from jsonschema import Draft202012Validator, RefResolver


_REPO_ROOT = Path(__file__).resolve().parents[2]
_SCHEMA_DIR = _REPO_ROOT / "schemas"


def _load_schema(name: str) -> dict[str, Any]:
    with (_SCHEMA_DIR / name).open(encoding="utf-8") as handle:
        return json.load(handle)


def _validator() -> Draft202012Validator:
    schema = _load_schema("semantic-search-packet.v1.schema.json")
    source_location_schema = _load_schema("semantic-source-location.v1.schema.json")
    resolver = RefResolver.from_schema(
        schema,
        store={
            source_location_schema["$id"]: source_location_schema,
            "semantic-source-location.v1.schema.json": source_location_schema,
        },
    )
    return Draft202012Validator(schema, resolver=resolver)


def _route_graph_packet() -> dict[str, Any]:
    owner_path = "crates/agent-semantic-client/src/native_prime.rs"
    return {
        "schemaId": "agent.semantic-protocols.semantic-search-packet",
        "schemaVersion": "1",
        "protocolId": "agent.semantic-protocols.semantic-language",
        "protocolVersion": "1",
        "languageId": "rust",
        "providerId": "rs-harness",
        "binary": "rs-harness",
        "namespace": "rust.search",
        "method": "search/owner",
        "projectRoot": ".",
        "view": "owner",
        "renderMode": "seeds",
        "header": {"kind": "search-owner", "fields": {}},
        "nodes": [],
        "edges": [],
        "owners": [],
        "hits": [],
        "findings": [],
        "nextActions": [],
        "notes": [],
        "routeGraph": {
            "profile": "asp-search-routing",
            "evidenceState": {
                "anchors": ["ownerPath", "symbol"],
                "ownerPath": owner_path,
                "symbol": "render_prime_seed_text",
                "workspaceKnown": True,
            },
            "routes": [
                {
                    "id": "KNOWN_OWNER",
                    "kind": "known-owner",
                    "preconditions": ["ownerPath"],
                    "targetRole": "path",
                    "projection": "skeleton",
                    "codePolicy": "disabled",
                    "cost": "low",
                    "actions": ["A1.owner-skeleton"],
                    "avoid": ["search-prime", "direct-source-read"],
                    "reason": "owner path is already known",
                }
            ],
            "chosenRouteId": "KNOWN_OWNER",
            "actionFrontier": ["A1.owner-skeleton"],
            "recommendedNext": "A1.owner-skeleton",
            "reason": "owner evidence makes prime broader than necessary",
            "omit": ["code", "full-json"],
            "avoid": ["search-prime", "line-range-selector", "direct-source-read"],
        },
        "actionFrontier": [
            {
                "id": "A1.owner-skeleton",
                "kind": "owner-skeleton",
                "routeId": "KNOWN_OWNER",
                "targetRole": "path",
                "target": owner_path,
                "ownerPath": owner_path,
                "projection": "skeleton",
                "codePolicy": "disabled",
                "displayLineRange": "225:267",
                "sourceLocatorHint": f"{owner_path}:225:267",
                "avoid": ["direct-source-read", "line-range-selector"],
            }
        ],
    }


class SemanticSearchPacketRouteGraphSchemaTests(unittest.TestCase):
    def setUp(self) -> None:
        self.validator = _validator()

    def validation_errors(self, packet: dict[str, Any]) -> list[str]:
        return [error.message for error in self.validator.iter_errors(packet)]

    def test_route_graph_and_action_frontier_are_valid(self) -> None:
        self.assertEqual([], self.validation_errors(_route_graph_packet()))

    def test_route_action_rejects_provider_command_strings(self) -> None:
        packet = _route_graph_packet()
        packet["actionFrontier"][0]["command"] = (
            "asp rust query crates/agent-semantic-client/src/native_prime.rs "
            "--query render_prime_seed_text --workspace . --code"
        )

        self.assertTrue(
            any("Additional properties are not allowed" in error for error in self.validation_errors(packet))
        )

    def test_route_action_rejects_absolute_owner_paths(self) -> None:
        packet = _route_graph_packet()
        packet["actionFrontier"][0]["ownerPath"] = "/Users/example/repo/src/lib.rs"

        self.assertTrue(
            any("does not match" in error for error in self.validation_errors(packet))
        )

    def test_route_action_rejects_absolute_source_locator_hints(self) -> None:
        packet = _route_graph_packet()
        packet["actionFrontier"][0]["sourceLocatorHint"] = "/Users/example/repo/src/lib.rs:1:4"

        self.assertTrue(
            any("does not match" in error for error in self.validation_errors(packet))
        )


if __name__ == "__main__":
    unittest.main()
