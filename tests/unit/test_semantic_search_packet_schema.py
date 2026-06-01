"""Schema contract tests for semantic-search packet path values."""

from __future__ import annotations

import copy
import json
import unittest
from pathlib import Path

from jsonschema import Draft202012Validator


_PROTOCOL_REPO_ROOT = Path(__file__).resolve().parents[2]


def minimal_packet() -> dict[str, object]:
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
                    "line": 42,
                    "column": 17,
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
        self.assertEqual([], self.validation_errors(minimal_packet()))

    def test_root_dot_path_token_is_valid(self) -> None:
        packet = minimal_packet()
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
        packet = minimal_packet()
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
        packet = minimal_packet()
        packet["hits"] = copy.deepcopy(packet["hits"])
        packet["hits"][0]["location"]["path"] = "../src/components/WorkflowExecution.tsx"

        errors = self.validation_errors(packet)

        self.assertTrue(any("does not match" in message for message in errors))

    def test_path_query_terms_are_canonical_paths(self) -> None:
        packet = minimal_packet()
        packet["querySet"] = [
            {"value": "0:src/components/WorkflowExecution.tsx", "kind": "path", "selector": "exact"}
        ]

        errors = self.validation_errors(packet)

        self.assertTrue(any("does not match" in message for message in errors))

    def test_text_query_set_can_classify_fixture_hits_and_avoid_false_owner(self) -> None:
        packet = minimal_packet()
        packet["method"] = "search/text"
        packet["view"] = "text"
        packet["renderMode"] = "seeds"
        packet["querySet"] = [
            {"value": "AgentHookEvent", "kind": "text", "selector": "exact"},
            {"value": "runCodexAgentHook", "kind": "text", "selector": "exact"},
        ]
        packet["queryComposition"] = {
            "mode": "query-set",
            "view": "text",
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
                    "line": 505,
                    "column": 17,
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
                {"kind": "text", "target": "runProtocolCli"},
                {"kind": "text", "target": "parseProtocolArgs"},
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

    def test_query_set_change_frontier_synthesis_is_schema_owned(self) -> None:
        packet = minimal_packet()
        packet["method"] = "search/text"
        packet["view"] = "text"
        packet["renderMode"] = "seeds"
        packet["querySet"] = [
            {"value": "load", "kind": "text", "selector": "exact"},
            {"value": "Thing", "kind": "text", "selector": "exact"},
        ]
        packet["queryComposition"] = {
            "mode": "query-set",
            "view": "text",
            "selector": "exact-set",
            "merge": ["owners", "notes", "nextActions"],
        }
        packet["searchSynthesis"] = {
            "algorithm": "change-frontier-query-set",
            "scope": "query-set",
            "selectedOwners": 3,
            "editFrontier": ["src/domain/mod.rs", "src/lib.rs"],
            "testFrontier": ["tests/domain.rs"],
            "seeds": [
                {"kind": "owner", "target": "src/domain/mod.rs"},
                {"kind": "owner", "target": "src/lib.rs"},
                {"kind": "tests", "target": "tests/domain.rs"},
            ],
        }

        self.assertEqual([], self.validation_errors(packet))

    def test_search_packet_accepts_embedded_graph_rank_and_weight(self) -> None:
        packet = minimal_packet()
        packet["method"] = "search/prime"
        packet["view"] = "prime"
        packet["renderMode"] = "seeds"
        packet["nodes"] = [
            {
                "id": "O:src/lib.rs",
                "kind": "owner",
                "path": "src/lib.rs",
                "rank": 1,
                "fields": {},
            },
            {
                "id": "X:src/lib.rs",
                "kind": "custom",
                "path": "src/lib.rs",
                "fields": {"axis": "research"},
            },
        ]
        packet["edges"] = [
            {
                "from": "O:src/lib.rs",
                "kind": "test",
                "to": "O:tests/lib.rs",
                "weight": 1,
                "location": {"path": "tests/lib.rs", "line": 4, "column": 1},
                "fields": {},
            }
        ]
        packet["searchSynthesis"] = {
            "algorithm": "owner-rank-frontier",
            "scope": "prime",
            "highImpactOwners": ["src/lib.rs"],
            "frontierOwners": ["tests/lib.rs"],
            "seeds": [{"kind": "tests", "target": "tests/lib.rs"}],
        }

        self.assertEqual([], self.validation_errors(packet))

    def test_large_library_packet_can_report_coverage_tests_and_runtime(self) -> None:
        packet = minimal_packet()
        packet["method"] = "search/prime"
        packet["view"] = "prime"
        packet["renderMode"] = "seeds"
        packet["projectRoot"] = "packages/vite"
        packet["sourceCoverage"] = [
            {
                "scope": {"projectRoot": "packages/vite", "packageName": "vite"},
                "status": "partial",
                "coverageKind": "config-root",
                "configPaths": ["packages/vite/tsconfig.json"],
                "coveredRoots": ["packages/vite/scripts"],
                "missingRoots": ["packages/vite/src/node"],
                "missingOwners": [
                    "packages/vite/src/node/server/pluginContainer.ts"
                ],
                "sourceFiles": 4,
                "visibleOwners": 4,
                "missingOwnersCount": 1,
                "reason": "package tsconfig excludes nested source tsconfigs",
            }
        ]
        packet["testResolution"] = [
            {
                "targetOwner": "packages/runtime-core/src/scheduler.ts",
                "status": "noisy",
                "scope": {"projectRoot": "."},
                "testPaths": ["packages/runtime-core/__tests__/scheduler.spec.ts"],
                "unrelatedTestPaths": [
                    "packages/vue-compat/__tests__/options.spec.ts"
                ],
                "candidateCount": 8,
                "selectedCount": 1,
                "noiseCount": 7,
                "reason": "root-scoped tests search found the colocated spec with unrelated package tests",
            }
        ]
        packet["runtimeCost"] = {
            "cacheStatus": "cold",
            "elapsedMs": 84292,
            "sourceFilesParsed": 708,
            "packagesScanned": 21,
            "parserFactsReused": False,
            "reason": "large monorepo search cold-started parser facts",
        }

        self.assertEqual([], self.validation_errors(packet))


if __name__ == "__main__":
    unittest.main()
