# SPDX-FileCopyrightText: 2026 tao3k team and Contributors
#
# SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

"""Schema contract tests for embedded semantic graph vocabulary."""

from __future__ import annotations

import copy
import json
import unittest
from pathlib import Path

from jsonschema import Draft202012Validator

from tests.unit.semantic_search_action_fixture import (
    complete_search_playbook_command,
)

_PROTOCOL_REPO_ROOT = Path(__file__).resolve().parents[2]


def minimal_graph() -> dict[str, object]:
    return {
        "schemaId": "agent.semantic-protocols.semantic-graph",
        "schemaVersion": "1",
        "protocolId": "agent.semantic-protocols.semantic-language",
        "protocolVersion": "1",
        "languageId": "rust",
        "providerId": "asp-rust",
        "projectRoot": ".",
        "packageName": ".",
        "graphKind": "owner-dependency",
        "scope": "search-playbook",
        "rootOwners": ["src/lib.rs"],
        "nodes": [
            {
                "id": "O:src/lib.rs",
                "kind": "owner",
                "path": "src/lib.rs",
                "rank": 1,
                "fields": {"role": "facade"},
            },
            {
                "id": "T:tests/lib.rs",
                "kind": "test",
                "path": "tests/lib.rs",
                "fields": {},
            },
        ],
        "edges": [
            {
                "from": "T:tests/lib.rs",
                "kind": "test",
                "to": "O:src/lib.rs",
                "location": {"path": "tests/lib.rs", "line": 3, "column": 1},
                "weight": 1,
            }
        ],
        "synthesis": {
            "algorithm": "owner-rank-frontier",
            "scope": "search-playbook",
            "summary": "embedded graph slice for search planning",
            "selectedOwners": 1,
            "selectedEdges": 1,
            "highImpactOwners": ["src/lib.rs"],
            "frontierOwners": ["tests/lib.rs"],
            "seeds": [
                {
                    "kind": "owner",
                    "target": "src/lib.rs",
                    "ownerPath": "src/lib.rs",
                    "command": complete_search_playbook_command(
                        language="rust",
                        term="src/lib.rs",
                        path_hint="src/lib.rs",
                        globs=("*.rs",),
                    ),
                },
                {
                    "kind": "tests",
                    "target": "tests/lib.rs",
                    "ownerPath": "src/lib.rs",
                    "command": complete_search_playbook_command(
                        language="rust",
                        term="tests/lib.rs",
                        path_hint="tests/lib.rs",
                        globs=("*.rs",),
                    ),
                },
            ],
        },
    }


class SemanticGraphSchemaTests(unittest.TestCase):
    def setUp(self) -> None:
        graph_schema_path = (
            _PROTOCOL_REPO_ROOT / "schemas" / "semantic-graph.v1.schema.json"
        )
        with graph_schema_path.open("r", encoding="utf-8") as handle:
            self.graph_schema = json.load(handle)
        self.validator = Draft202012Validator(self.graph_schema)

    def validation_errors(self, graph: dict[str, object]) -> list[str]:
        return [error.message for error in self.validator.iter_errors(graph)]

    def test_embedded_search_graph_slice_is_valid(self) -> None:
        self.assertEqual([], self.validation_errors(minimal_graph()))

    def test_graph_agent_facing_scopes_match_synthesis_scopes(self) -> None:
        for scope in ("policy", "query", "query-set", "search-playbook", "tests"):
            with self.subTest(scope=scope):
                graph = minimal_graph()
                graph["scope"] = scope
                graph["synthesis"] = copy.deepcopy(graph["synthesis"])
                graph["synthesis"]["scope"] = scope

                self.assertEqual([], self.validation_errors(graph))

    def test_rank_prefixed_node_path_is_rejected(self) -> None:
        graph = minimal_graph()
        graph["nodes"] = copy.deepcopy(graph["nodes"])
        graph["nodes"][0]["path"] = "0:src/lib.rs"

        errors = self.validation_errors(graph)

        self.assertTrue(any("does not match" in message for message in errors))

    def test_relative_escape_synthesis_owner_is_rejected(self) -> None:
        graph = minimal_graph()
        graph["synthesis"] = copy.deepcopy(graph["synthesis"])
        graph["synthesis"]["highImpactOwners"] = ["../src/lib.rs"]

        errors = self.validation_errors(graph)

        self.assertTrue(any("does not match" in message for message in errors))

if __name__ == "__main__":
    unittest.main()
