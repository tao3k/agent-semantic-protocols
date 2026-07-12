"""Schema contract tests for search packet query-set fields."""

from __future__ import annotations

import json
import unittest
from pathlib import Path



_PROTOCOL_REPO_ROOT = Path(__file__).resolve().parents[2]


def semantic_search_query_set_minimal_packet() -> dict[str, object]:
    return {
        "schemaId": "agent.semantic-protocols.semantic-search-packet",
        "schemaVersion": "1",
        "protocolId": "agent.semantic-protocols.semantic-language",
        "protocolVersion": "1",
        "languageId": "typescript",
        "providerId": "ts-harness",
        "binary": "ts-harness",
        "namespace": "agent.semantic-protocols.languages.typescript.ts-harness",
        "method": "search/lexical",
        "projectRoot": ".",
        "view": "lexical",
        "renderMode": "seeds",
        "header": {"kind": "search-lexical", "fields": {}},
        "nodes": [],
        "edges": [],
        "owners": [],
        "hits": [],
        "findings": [],
        "nextActions": [],
        "notes": [],
    }


class SemanticSearchPacketQuerySetSchemaTests(unittest.TestCase):
    def setUp(self) -> None:
        schema_path = (
            _PROTOCOL_REPO_ROOT / "schemas" / "semantic-search-packet.v1.schema.json"
        )
        from unit.schema_validation import schema_validator_for

        self.validator = schema_validator_for(schema_path)

    def validation_errors(self, packet: dict[str, object]) -> list[str]:
        return [error.message for error in self.validator.iter_errors(packet)]

    def test_lexical_query_set_can_classify_fixture_hits_and_avoid_false_owner(
        self,
    ) -> None:
        packet = semantic_search_query_set_minimal_packet()
        packet["querySet"] = [
            {"value": "AgentHookEvent", "kind": "text", "selector": "exact"},
            {"value": "runCodexAgentHook", "kind": "text", "selector": "exact"},
        ]
        packet["queryComposition"] = {
            "mode": "query-set",
            "view": "lexical",
            "selector": "exact-set",
            "scope": {"projectRoot": "."},
            "merge": ["owners", "hits", "nextActions", "notes"],
        }
        packet["hits"] = [
            {
                "kind": "text",
                "ownerPath": "tests/unit/cli.test.ts",
                "location": {
                    "path": "tests/unit/cli.test.ts",
                    "lineRange": "505:505",
                },
                "score": 2,
                "reason": "source-text",
                "snippet": "src/cli/agent-hooks.ts",
                "surface": "test-fixture-string",
                "realOwner": False,
                "fixturePath": "src/cli/agent-hooks.ts",
                "fixtureOwner": "tests/unit/cli.test.ts",
            }
        ]
        packet["queryCoverage"] = [
            {
                "value": "AgentHookEvent",
                "kind": "text",
                "status": "miss",
                "hitCount": 0,
            },
            {
                "value": "runCodexAgentHook",
                "kind": "text",
                "status": "hit",
                "hitCount": 2,
                "surfaces": ["test-fixture-string"],
                "ownerPaths": ["tests/unit/cli.test.ts"],
                "fixturePaths": ["src/cli/agent-hooks.ts"],
            },
        ]
        packet["ownerResolution"] = [
            {
                "target": "src/cli/agent-hooks.ts",
                "status": "fixture-path",
                "realOwner": False,
                "fixturePath": "src/cli/agent-hooks.ts",
                "fixtureOwner": "tests/unit/cli.test.ts",
                "reason": "path appears only as a test fixture string",
            }
        ]
        packet["searchSynthesis"] = {
            "algorithm": "query-set-owner-resolution",
            "scope": "query-set",
            "summary": "fixture string points at protocol CLI implementation axis",
            "seeds": [
                {"kind": "symbol", "target": "runProtocolCli"},
                {"kind": "symbol", "target": "parseProtocolArgs"},
                {"kind": "owner", "target": "src/cli/protocol.ts"},
            ],
        }
        packet["avoidNextActions"] = [
            {
                "kind": "owner",
                "target": "src/cli/agent-hooks.ts",
                "reason": "fixture-path-not-workspace-owner",
            }
        ]

        self.assertEqual([], self.validation_errors(packet))

    def test_project_paths_are_canonical_not_display_locators(self) -> None:
        owner_path = "src/components/WorkflowExecution.tsx"
        packet = semantic_search_query_set_minimal_packet()
        packet["querySet"] = [
            {"value": owner_path, "kind": "owner", "selector": "exact"}
        ]
        packet["owners"] = [
            {"path": owner_path, "role": "source", "public": False, "fields": {}}
        ]
        packet["items"] = [
            {
                "name": "WorkflowExecution",
                "kind": "component",
                "ownerPath": owner_path,
                "location": {"path": owner_path, "lineRange": "1:1"},
                "fields": {},
            }
        ]
        packet["hits"] = [
            {
                "kind": "text",
                "ownerPath": owner_path,
                "location": {"path": owner_path, "lineRange": "1:1"},
                "score": 1,
                "reason": "owner-match",
            }
        ]
        packet["nextActions"] = [
            {"kind": "owner", "target": owner_path, "ownerPath": owner_path}
        ]
        self.assertEqual([], self.validation_errors(packet))

        invalid_paths = [
            "0:src/components/WorkflowExecution.tsx",
            "owner:src/components/WorkflowExecution.tsx",
            "src/components/WorkflowExecution.tsx:10:20",
            "/Users/example/project/src/components/WorkflowExecution.tsx",
            "src\\components\\WorkflowExecution.tsx",
            "../src/components/WorkflowExecution.tsx",
        ]
        slots = [
            ("querySet", 0, "value"),
            ("owners", 0, "path"),
            ("items", 0, "ownerPath"),
            ("hits", 0, "ownerPath"),
            ("nextActions", 0, "ownerPath"),
        ]
        for invalid_path in invalid_paths:
            for section, index, key in slots:
                with self.subTest(invalid_path=invalid_path, section=section):
                    bad_packet = json.loads(json.dumps(packet))
                    bad_packet[section][index][key] = invalid_path
                    self.assertTrue(self.validation_errors(bad_packet))

            with self.subTest(invalid_path=invalid_path, section="location"):
                bad_packet = json.loads(json.dumps(packet))
                bad_packet["hits"][0]["location"]["path"] = invalid_path
                self.assertTrue(self.validation_errors(bad_packet))

    def test_query_set_change_frontier_synthesis_is_schema_owned(self) -> None:
        packet = semantic_search_query_set_minimal_packet()
        packet["querySet"] = [
            {"value": "load", "kind": "text", "selector": "exact"},
            {"value": "Thing", "kind": "text", "selector": "exact"},
        ]
        packet["queryComposition"] = {
            "mode": "query-set",
            "view": "lexical",
            "selector": "exact-set",
            "merge": ["owners", "notes", "nextActions"],
        }
        packet["searchSynthesis"] = {
            "algorithm": "change-frontier-query-set",
            "scope": "query-set",
            "selectedOwners": 3,
            "editFrontier": ["src/domain/mod.rs", "src/lib.rs"],
            "testFrontier": ["tests/domain.rs"],
            "windowSet": [
                {"kind": "owner", "target": "src/domain/mod.rs"},
                {"kind": "owner", "target": "src/lib.rs"},
                {"kind": "tests", "target": "tests/domain.rs"},
            ],
            "seeds": [
                {"kind": "owner", "target": "src/domain/mod.rs"},
                {"kind": "owner", "target": "src/lib.rs"},
                {"kind": "tests", "target": "tests/domain.rs"},
            ],
        }

        self.assertEqual([], self.validation_errors(packet))


if __name__ == "__main__":
    unittest.main()
