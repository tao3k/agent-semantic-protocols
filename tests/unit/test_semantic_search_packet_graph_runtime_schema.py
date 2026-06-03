"""Schema contract tests for graph and runtime search packet fields."""

from __future__ import annotations

import json
import unittest
from pathlib import Path

from jsonschema import Draft202012Validator


_PROTOCOL_REPO_ROOT = Path(__file__).resolve().parents[2]


def semantic_search_graph_runtime_minimal_packet() -> dict[str, object]:
    return {
        "schemaId": "agent.semantic-protocols.semantic-search-packet",
        "schemaVersion": "1",
        "protocolId": "agent.semantic-protocols.semantic-language",
        "protocolVersion": "1",
        "languageId": "typescript",
        "providerId": "ts-harness",
        "binary": "ts-harness",
        "namespace": "agent.semantic-protocols.languages.typescript.ts-harness",
        "method": "search/prime",
        "projectRoot": ".",
        "view": "prime",
        "renderMode": "seeds",
        "header": {"kind": "search-prime", "fields": {}},
        "nodes": [],
        "edges": [],
        "owners": [],
        "hits": [],
        "findings": [],
        "nextActions": [],
        "notes": [],
    }


class SemanticSearchPacketGraphRuntimeSchemaTests(unittest.TestCase):
    def setUp(self) -> None:
        schema_path = (
            _PROTOCOL_REPO_ROOT / "schemas" / "semantic-search-packet.v1.schema.json"
        )
        with schema_path.open("r", encoding="utf-8") as handle:
            self.validator = Draft202012Validator(json.load(handle))

    def validation_errors(self, packet: dict[str, object]) -> list[str]:
        return [error.message for error in self.validator.iter_errors(packet)]

    def test_search_packet_accepts_embedded_graph_rank_and_weight(self) -> None:
        packet = semantic_search_graph_runtime_minimal_packet()
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
                "location": {"path": "tests/lib.rs", "lineRange": "4:4"},
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

    def test_search_synthesis_accepts_agent_facing_graph_scopes(self) -> None:
        for scope in ("policy", "query", "query-set", "fzf", "tests", "ingest"):
            with self.subTest(scope=scope):
                packet = semantic_search_graph_runtime_minimal_packet()
                packet["searchSynthesis"] = {
                    "algorithm": "owner-rank-frontier",
                    "scope": scope,
                    "seeds": [{"kind": "tests", "target": "tests/lib.rs"}],
                    "windowSet": [{"kind": "tests", "target": "tests/lib.rs"}],
                }

                self.assertEqual([], self.validation_errors(packet))

    def test_large_library_packet_can_report_coverage_tests_and_runtime(self) -> None:
        packet = semantic_search_graph_runtime_minimal_packet()
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
                "reason": "root-scoped tests search found unrelated package tests",
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
