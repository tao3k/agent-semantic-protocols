"""Schema contract tests for semantic-search packet path values."""

from __future__ import annotations

import copy
import json
import unittest
from pathlib import Path

from jsonschema import Draft202012Validator


_PROTOCOL_REPO_ROOT = Path(__file__).resolve().parents[2]


def _semantic_search_minimal_packet() -> dict[str, object]:
    return {
        "schemaId": "agent.semantic-protocols.semantic-search-packet",
        "schemaVersion": "1",
        "protocolId": "agent.semantic-protocols.semantic-language",
        "protocolVersion": "1",
        "languageId": "typescript",
        "providerId": "ts-harness",
        "binary": "ts-harness",
        "namespace": "agent.semantic-protocols.languages.typescript.ts-harness",
        "method": "search/owner",
        "projectRoot": ".",
        "view": "owner",
        "renderMode": "graph",
        "header": {"kind": "search-owner", "fields": {}},
        "nodes": [
            {
                "id": "O:src/components/WorkflowExecution.tsx",
                "kind": "owner",
                "path": "src/components/WorkflowExecution.tsx",
                "fields": {},
            }
        ],
        "edges": [],
        "owners": [
            {
                "path": "src/components/WorkflowExecution.tsx",
                "role": "source",
                "public": False,
                "fields": {},
            }
        ],
        "hits": [
            {
                "kind": "text",
                "ownerPath": "src/components/WorkflowExecution.tsx",
                "location": {
                    "path": "src/components/WorkflowExecution.tsx",
                    "lineRange": "42:42",
                },
                "score": 1.0,
                "reason": "parser-visible-source",
            }
        ],
        "findings": [],
        "nextActions": [
            {
                "kind": "owner",
                "target": "src/data/workflows.ts",
                "ownerPath": "src/components/WorkflowExecution.tsx",
            }
        ],
        "notes": [],
    }


class SemanticSearchPacketSchemaTests(unittest.TestCase):
    def setUp(self) -> None:
        schema_path = (
            _PROTOCOL_REPO_ROOT / "schemas" / "semantic-search-packet.v1.schema.json"
        )
        with schema_path.open("r", encoding="utf-8") as handle:
            self.validator = Draft202012Validator(json.load(handle))

    def validation_errors(self, packet: dict[str, object]) -> list[str]:
        return [error.message for error in self.validator.iter_errors(packet)]

    def test_project_root_relative_paths_are_valid(self) -> None:
        self.assertEqual([], self.validation_errors(_semantic_search_minimal_packet()))

    def test_search_packet_accepts_tree_sitter_syntax_refs(self) -> None:
        packet = _semantic_search_minimal_packet()
        packet["syntaxQueryRef"] = "semantic-tree-sitter-query/rust-owner-items.v1"
        packet["syntaxMatchRefs"] = ["match.1"]
        packet["syntaxCaptureRefs"] = ["capture.1"]
        packet["syntaxAnchor"] = {
            "nodeType": "function_item",
            "field": "name",
            "capture": "function.name",
            "location": {"path": "src/lib.rs", "lineRange": "6:6"},
        }

        self.assertEqual([], self.validation_errors(packet))

    def test_root_dot_path_token_is_valid(self) -> None:
        packet = _semantic_search_minimal_packet()
        packet["owners"] = [
            {
                "path": ".",
                "role": "workspace-root",
                "public": False,
                "fields": {},
            }
        ]
        packet["hits"] = copy.deepcopy(packet["hits"])
        packet["hits"][0]["ownerPath"] = "."

        self.assertEqual([], self.validation_errors(packet))

    def test_rank_prefixed_owner_paths_are_rejected(self) -> None:
        packet = _semantic_search_minimal_packet()
        packet["owners"] = [
            {
                "path": "0:src/components/WorkflowExecution.tsx",
                "role": "source",
                "public": False,
                "fields": {},
            }
        ]

        errors = self.validation_errors(packet)

        self.assertTrue(any("does not match" in message for message in errors))

    def test_relative_escape_location_paths_are_rejected(self) -> None:
        packet = _semantic_search_minimal_packet()
        packet["hits"] = copy.deepcopy(packet["hits"])
        packet["hits"][0]["location"]["path"] = (
            "../src/components/WorkflowExecution.tsx"
        )

        errors = self.validation_errors(packet)

        self.assertTrue(any("does not match" in message for message in errors))

    def test_path_query_terms_are_canonical_paths(self) -> None:
        packet = _semantic_search_minimal_packet()
        packet["querySet"] = [
            {
                "value": "0:src/components/WorkflowExecution.tsx",
                "kind": "path",
                "selector": "exact",
            }
        ]

        errors = self.validation_errors(packet)

        self.assertTrue(any("does not match" in message for message in errors))

    def test_window_set_accepts_graph_frontier_kinds(self) -> None:
        packet = _semantic_search_minimal_packet()
        packet["searchSynthesis"] = {
            "algorithm": "owner-rank-frontier",
            "scope": "prime",
            "windowSet": [
                {"kind": "features", "target": "feature:cli"},
                {"kind": "deps", "target": "serde"},
                {"kind": "import", "target": "src/parser.rs"},
            ],
        }

        self.assertEqual([], self.validation_errors(packet))

    def test_search_synthesis_seed_accepts_canonical_read_locator(self) -> None:
        packet = _semantic_search_minimal_packet()
        packet["searchSynthesis"] = {
            "algorithm": "owner-rank-frontier",
            "scope": "owner",
            "seeds": [
                {
                    "kind": "symbol",
                    "target": "SemanticSearchOwnerFallback",
                    "targetRole": "symbol",
                    "read": "src/cli/semantic-search/owner-fallback.ts:1:5",
                }
            ],
        }

        self.assertEqual([], self.validation_errors(packet))

    def test_search_synthesis_seed_rejects_locator_alias(self) -> None:
        packet = _semantic_search_minimal_packet()
        packet["searchSynthesis"] = {
            "algorithm": "owner-rank-frontier",
            "scope": "owner",
            "seeds": [
                {
                    "kind": "symbol",
                    "target": "SemanticSearchOwnerFallback",
                    "targetRole": "symbol",
                    "locator": "src/cli/semantic-search/owner-fallback.ts:1:5",
                }
            ],
        }

        errors = self.validation_errors(packet)

        self.assertTrue(
            any("Additional properties are not allowed" in message for message in errors)
        )

    def test_unknown_window_set_kind_is_rejected(self) -> None:
        packet = _semantic_search_minimal_packet()
        packet["searchSynthesis"] = {
            "algorithm": "owner-rank-frontier",
            "scope": "prime",
            "windowSet": [{"kind": "maybe", "target": "src/lib.rs"}],
        }

        errors = self.validation_errors(packet)

        self.assertTrue(any("'maybe' is not one of" in message for message in errors))

    def test_window_set_display_locators_are_rejected(self) -> None:
        packet = _semantic_search_minimal_packet()
        packet["searchSynthesis"] = {
            "algorithm": "owner-rank-frontier",
            "scope": "prime",
            "windowSet": [{"kind": "read", "target": "src/lib.rs:12"}],
        }

        errors = self.validation_errors(packet)

        self.assertTrue(
            any(
                "is not valid under any of the given schemas" in message
                for message in errors
            )
        )


if __name__ == "__main__":
    unittest.main()
