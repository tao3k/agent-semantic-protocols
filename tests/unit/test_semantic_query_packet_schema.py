"""Schema tests for owner-item semantic query packets."""

import json
import unittest
from pathlib import Path

from jsonschema import Draft202012Validator

_REPO_ROOT = Path(__file__).resolve().parents[2]


def semantic_query_minimal_packet() -> dict[str, object]:
    return {
        "schemaId": "agent.semantic-protocols.semantic-query-packet",
        "schemaVersion": "1",
        "protocolId": "agent.semantic-protocols.semantic-language",
        "protocolVersion": "1",
        "languageId": "rust",
        "providerId": "rs-harness",
        "binary": "rs-harness",
        "namespace": "agent.semantic-protocols.languages.rust.rs-harness",
        "method": "query/owner-items",
        "projectRoot": "/workspace/project",
        "ownerPath": "src/lib.rs",
        "query": "load|clone_value",
        "queryTerms": ["load", "clone_value"],
        "matchMode": "exact",
        "outputMode": "code",
        "queryCoverage": [
            {
                "value": "load",
                "status": "hit",
                "match": "exact",
                "matchCount": 1,
            }
        ],
        "matches": [
            {
                "name": "load",
                "kind": "fn",
                "visibility": "public",
                "doc": False,
                "location": {"path": "src/lib.rs", "lineRange": "6:6"},
                "read": "src/lib.rs:6:6",
                "code": "pub fn load() -> Thing { domain::make_thing() }",
                "projection": {
                    "mode": "compact",
                    "syntax": "brace-block",
                    "sourceAuthority": "native-parser",
                    "sourceFingerprint": "src/lib.rs:6:6:39",
                    "losslessStructure": True,
                    "exactRead": "src/lib.rs:6:6",
                    "nodes": [
                        {
                            "id": "load",
                            "nativeId": "rust:fn:load",
                            "kind": "fn",
                            "role": "declaration",
                            "label": "load",
                            "depth": 0,
                            "read": "src/lib.rs:6:6",
                            "structuralFingerprint": "fn:declaration:load",
                            "flags": ["call", "return"],
                        }
                    ],
                    "renderedNodeIds": ["load"],
                    "omitted": [
                        {
                            "kind": "body-detail",
                            "reason": "single-line compact projection keeps exact source behind read locator",
                            "count": 1,
                            "read": "src/lib.rs:6:6",
                        }
                    ],
                    "expandActions": [
                        {
                            "kind": "exact-read",
                            "target": "load",
                            "read": "src/lib.rs:6:6",
                            "argv": [
                                "rs-harness",
                                "query",
                                "--from-hook",
                                "direct-source-read",
                                "--selector",
                                "src/lib.rs:6:6",
                                ".",
                            ],
                            "reason": "read exact source before editing",
                        }
                    ],
                },
                "truncated": False,
            }
        ],
        "truncated": False,
        "notes": [],
    }


class SemanticQueryPacketSchemaTests(unittest.TestCase):
    def setUp(self) -> None:
        schema_path = _REPO_ROOT / "schemas" / "semantic-query-packet.v1.schema.json"
        with schema_path.open("r", encoding="utf-8") as handle:
            self.validator = Draft202012Validator(json.load(handle))

    def validation_errors(self, packet: dict[str, object]) -> list[str]:
        return [error.message for error in self.validator.iter_errors(packet)]

    def test_minimal_query_packet_is_valid(self) -> None:
        self.assertEqual([], self.validation_errors(semantic_query_minimal_packet()))

    def test_projection_schema_keeps_formatter_profile_provider_owned(self) -> None:
        projection_schema = self.validator.schema["$defs"]["projection"]
        projection_properties = projection_schema["properties"]

        self.assertNotIn("formatter", projection_properties)
        self.assertNotIn("formatterProfile", projection_properties)
        self.assertNotIn(
            "formatter-normalized",
            projection_properties["sourceAuthority"]["enum"],
        )

    def test_names_only_query_packet_can_omit_code_and_report_candidates(self) -> None:
        packet = semantic_query_minimal_packet()
        packet["outputMode"] = "names"
        packet["matchMode"] = "mixed"
        packet["query"] = "parse_ripgrep_scope"
        packet["queryTerms"] = ["parse_ripgrep_scope"]
        packet["queryCoverage"] = [
            {
                "value": "parse_ripgrep_scope",
                "status": "miss",
                "match": "none",
                "matchCount": 0,
                "candidateNames": ["parse_ripgrep_like"],
                "nextAction": "query:parse_ripgrep_like",
            }
        ]
        packet["candidateItems"] = [
            {
                "name": "parse_ripgrep_like",
                "reason": "prefix",
                "term": "parse_ripgrep_scope",
            }
        ]
        del packet["matches"][0]["code"]  # type: ignore[index]
        del packet["matches"][0]["projection"]  # type: ignore[index]
        self.assertEqual([], self.validation_errors(packet))

    def test_outline_projection_can_report_hot_blocks(self) -> None:
        packet = semantic_query_minimal_packet()
        packet["outputMode"] = "outline"
        del packet["matches"][0]["code"]  # type: ignore[index]
        packet["matches"][0]["projection"] = {  # type: ignore[index]
            "mode": "outline",
            "syntax": "semantic-outline",
            "sourceAuthority": "native-parser",
            "losslessStructure": True,
            "exactRead": "src/lib.rs:6:24",
        }
        packet["matches"][0]["outline"] = {  # type: ignore[index]
            "summary": "load constructs Thing through the domain factory",
            "inputs": ["none"],
            "returns": "Thing",
            "guards": [],
            "flow": ["call domain::make_thing", "return Thing"],
            "effects": ["calls domain::make_thing"],
            "hotBlocks": [
                {
                    "label": "factory-return",
                    "read": "src/lib.rs:6:6",
                    "reason": "exact item body",
                }
            ],
        }
        self.assertEqual([], self.validation_errors(packet))

    def test_projection_rejects_owner_name_routing_action(self) -> None:
        packet = semantic_query_minimal_packet()
        packet["matches"][0]["projection"] = {  # type: ignore[index]
            "mode": "compact",
            "syntax": "semantic-outline",
            "sourceAuthority": "native-parser",
            "losslessStructure": True,
            "exactRead": "src/lib.rs:6:6",
            "expandActions": [
                {
                    "kind": "owner-names",
                    "target": "src/lib.rs",
                    "reason": "return owner-local item names without code windows",
                }
            ],
        }

        errors = self.validation_errors(packet)

        self.assertNotEqual([], errors)

    def test_projection_exact_read_action_requires_read_locator(self) -> None:
        packet = semantic_query_minimal_packet()
        packet["matches"][0]["projection"] = {  # type: ignore[index]
            "mode": "compact",
            "syntax": "semantic-outline",
            "sourceAuthority": "native-parser",
            "losslessStructure": True,
            "exactRead": "src/lib.rs:6:6",
            "expandActions": [
                {
                    "kind": "exact-read",
                    "target": "load",
                    "reason": "read exact source before editing",
                }
            ],
        }

        errors = self.validation_errors(packet)

        self.assertNotEqual([], errors)

    def test_projection_hot_block_action_requires_read_locator(self) -> None:
        packet = semantic_query_minimal_packet()
        packet["matches"][0]["projection"] = {  # type: ignore[index]
            "mode": "compact",
            "syntax": "semantic-outline",
            "sourceAuthority": "native-parser",
            "losslessStructure": True,
            "exactRead": "src/lib.rs:6:6",
            "expandActions": [
                {
                    "kind": "hot-block",
                    "target": "load:branch",
                    "reason": "expand hot control-flow block",
                }
            ],
        }

        errors = self.validation_errors(packet)

        self.assertNotEqual([], errors)

    def test_projection_node_query_action_requires_argv(self) -> None:
        packet = semantic_query_minimal_packet()
        packet["matches"][0]["projection"] = {  # type: ignore[index]
            "mode": "compact",
            "syntax": "semantic-outline",
            "sourceAuthority": "native-parser",
            "losslessStructure": True,
            "exactRead": "src/lib.rs:6:6",
            "expandActions": [
                {
                    "kind": "node-query",
                    "target": "load:branch",
                    "reason": "expand node through provider query",
                }
            ],
        }

        errors = self.validation_errors(packet)

        self.assertNotEqual([], errors)

    def test_projection_node_query_action_rejects_empty_argv(self) -> None:
        packet = semantic_query_minimal_packet()
        packet["matches"][0]["projection"] = {  # type: ignore[index]
            "mode": "compact",
            "syntax": "semantic-outline",
            "sourceAuthority": "native-parser",
            "losslessStructure": True,
            "exactRead": "src/lib.rs:6:6",
            "expandActions": [
                {
                    "kind": "node-query",
                    "target": "load:branch",
                    "argv": [],
                    "reason": "expand node through provider query",
                }
            ],
        }

        errors = self.validation_errors(packet)

        self.assertNotEqual([], errors)

    def test_projection_node_query_action_rejects_read_locator(self) -> None:
        packet = semantic_query_minimal_packet()
        packet["matches"][0]["projection"] = {  # type: ignore[index]
            "mode": "compact",
            "syntax": "semantic-outline",
            "sourceAuthority": "native-parser",
            "losslessStructure": True,
            "exactRead": "src/lib.rs:6:6",
            "expandActions": [
                {
                    "kind": "node-query",
                    "target": "load:branch",
                    "read": "src/lib.rs:6:6",
                    "argv": ["rs-harness", "search", "owner", "src/lib.rs", "items", "."],
                    "reason": "node queries must use provider argv, not read locators",
                }
            ],
        }

        errors = self.validation_errors(packet)

        self.assertNotEqual([], errors)

    def test_compact_projection_requires_navigation_metadata(self) -> None:
        packet = semantic_query_minimal_packet()
        packet["matches"][0]["projection"] = {  # type: ignore[index]
            "mode": "compact",
            "syntax": "semantic-outline",
            "sourceAuthority": "native-parser",
        }

        errors = self.validation_errors(packet)

        self.assertNotEqual([], errors)

    def test_compact_projection_node_requires_native_identity_metadata(self) -> None:
        for field_name in ("nativeId", "structuralFingerprint"):
            with self.subTest(field_name=field_name):
                packet = semantic_query_minimal_packet()
                node = packet["matches"][0]["projection"]["nodes"][0]  # type: ignore[index]
                del node[field_name]  # type: ignore[index]

                errors = self.validation_errors(packet)

                self.assertNotEqual([], errors)

    def test_compact_match_can_declare_patch_verify_safety(self) -> None:
        packet = semantic_query_minimal_packet()
        packet["patchSafety"] = {
            "level": "read-safe",
            "reason": "packet default requires exact source read before editing",
            "exactRead": "src/lib.rs:6:6",
        }
        packet["matches"][0]["patchSafety"] = {  # type: ignore[index]
            "level": "patch-verify-safe",
            "target": {
                "ownerPath": "src/lib.rs",
                "locator": "src/lib.rs#fn:load",
                "read": "src/lib.rs:6:6",
                "location": {"path": "src/lib.rs", "lineRange": "6:6"},
                "itemName": "load",
                "itemKind": "fn",
            },
            "preimageSource": "exact-read",
            "sourceFingerprint": "sha256:abc123",
            "parserVersion": "rust:rust-lang-project-harness",
            "allowedOperations": ["replace_statement", "append_to_block"],
            "losslessStructure": True,
        }

        self.assertEqual([], self.validation_errors(packet))

    def test_patch_verify_safety_requires_exact_read_preimage_source(self) -> None:
        packet = semantic_query_minimal_packet()
        packet["matches"][0]["patchSafety"] = {  # type: ignore[index]
            "level": "patch-verify-safe",
            "target": {
                "ownerPath": "src/lib.rs",
                "locator": "src/lib.rs#fn:load",
                "read": "src/lib.rs:6:6",
                "location": {"path": "src/lib.rs", "lineRange": "6:6"},
            },
            "sourceFingerprint": "sha256:abc123",
            "parserVersion": "rust:rust-lang-project-harness",
            "allowedOperations": ["replace_statement"],
        }

        errors = self.validation_errors(packet)

        self.assertNotEqual([], errors)

    def test_patch_verify_safety_rejects_start_line_end_line(self) -> None:
        packet = semantic_query_minimal_packet()
        packet["matches"][0]["patchSafety"] = {  # type: ignore[index]
            "level": "patch-verify-safe",
            "target": {
                "ownerPath": "src/lib.rs",
                "locator": "src/lib.rs#fn:load",
                "read": "src/lib.rs:6:6",
                "location": {"path": "src/lib.rs", "lineRange": "6:6"},
                "startLine": 6,
                "endLine": 6,
            },
            "preimageSource": "exact-read",
            "sourceFingerprint": "sha256:abc123",
            "parserVersion": "rust:rust-lang-project-harness",
            "allowedOperations": ["replace_statement"],
        }

        errors = self.validation_errors(packet)

        self.assertNotEqual([], errors)

    def test_patch_verify_safety_requires_source_fingerprint(self) -> None:
        packet = semantic_query_minimal_packet()
        packet["matches"][0]["patchSafety"] = {  # type: ignore[index]
            "level": "patch-verify-safe",
            "target": {
                "ownerPath": "src/lib.rs",
                "locator": "src/lib.rs#fn:load",
                "read": "src/lib.rs:6:6",
                "location": {"path": "src/lib.rs", "lineRange": "6:6"},
            },
            "parserVersion": "rust:rust-lang-project-harness",
            "allowedOperations": ["replace_statement"],
        }

        errors = self.validation_errors(packet)

        self.assertNotEqual([], errors)

    def test_read_locator_rejects_rank_prefix_path(self) -> None:
        packet = semantic_query_minimal_packet()
        packet["matches"][0]["read"] = "0:src/lib.rs:6:6"  # type: ignore[index]

        errors = self.validation_errors(packet)

        self.assertTrue(any("does not match" in message for message in errors))


if __name__ == "__main__":
    unittest.main()
