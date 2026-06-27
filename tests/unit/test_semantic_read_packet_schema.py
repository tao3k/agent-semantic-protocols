"""Validate provider-owned semantic read packet schema boundaries."""

from __future__ import annotations

from pathlib import Path
import unittest



_REPO_ROOT = Path(__file__).resolve().parents[2]


def semantic_read_minimal_packet() -> dict[str, object]:
    return {
        "schemaId": "agent.semantic-protocols.semantic-read-packet",
        "schemaVersion": "1",
        "protocolId": "agent.semantic-protocols.semantic-language",
        "protocolVersion": "1",
        "languageId": "rust",
        "providerId": "rs-harness",
        "binary": "rs-harness",
        "namespace": "agent.semantic-protocols.languages.rust.rs-harness",
        "method": "query/direct-source-read",
        "projectRoot": "/workspace/project",
        "ownerPath": "src/lib.rs",
        "selector": "src/lib.rs",
        "fromHook": "direct-source-read",
        "fallbackReason": "parser structural selector unavailable for this direct read",
        "outputMode": "read-packet",
        "sourceWindows": [
            {
                "ownerPath": "src/lib.rs",
                "itemName": "load",
                "itemKind": "fn",
                "location": {"path": "src/lib.rs", "lineRange": "6:6"},
                "read": "src/lib.rs:6:6",
                        "lineCount": 1,
                "reason": "direct-selector",
                "text": "pub fn load() -> Thing { domain::make_thing() }",
                "truncated": False,
            }
        ],
        "truncated": False,
        "notes": [],
    }


class SemanticReadPacketSchemaTests(unittest.TestCase):
    def setUp(self) -> None:
        from unit.schema_validation import schema_validator_for

        schema_path = _REPO_ROOT / "schemas" / "semantic-read-packet.v1.schema.json"
        self.validator = schema_validator_for(schema_path)

    def validation_errors(self, packet: dict[str, object]) -> list[str]:
        return [error.message for error in self.validator.iter_errors(packet)]

    def test_minimal_provider_read_packet_is_valid(self) -> None:
        self.assertEqual([], self.validation_errors(semantic_read_minimal_packet()))

    def test_read_packet_accepts_git_source_version_metadata(self) -> None:
        packet = semantic_read_minimal_packet()
        packet["repositoryRoot"] = "/workspace/project/languages/rust-lang-project-harness"
        packet["sourceVersion"] = "index"
        packet["gitBlobOid"] = "a" * 40

        self.assertEqual([], self.validation_errors(packet))

    def test_read_packet_accepts_worktree_hash_metadata(self) -> None:
        packet = semantic_read_minimal_packet()
        packet["sourceVersion"] = "worktree"
        packet["worktreeHash"] = "sha256:" + ("b" * 64)

        self.assertEqual([], self.validation_errors(packet))

    def test_read_packet_rejects_unknown_source_version(self) -> None:
        packet = semantic_read_minimal_packet()
        packet["sourceVersion"] = "staged"
        errors = self.validation_errors(packet)

        self.assertTrue(any("is not one of" in message for message in errors))

    def test_read_packet_accepts_tree_sitter_syntax_refs(self) -> None:
        packet = semantic_read_minimal_packet()
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

    def test_read_plan_frontier_packet_is_valid_without_source_windows_or_symbols(self) -> None:
        packet = semantic_read_minimal_packet()
        packet.pop("sourceWindows")
        packet["selector"] = "src/lib.rs:115-240"
        packet["readPlan"] = {
            "mode": "range-frontier",
            "code": False,
            "reason": "locator-frontier",
            "maxWindowLines": 40,
            "algorithm": "range-split",
            "syn": "function_item/name",
            "ranges": [
                {
                    "path": "src/lib.rs",
                    "requested": "115:120",
                    "selected": "115:120",
                    "matched": "110:120",
                    "coverage": "tail-only",
                    "density": "normal",
                }
            ],
            "windows": [
                {
                    "path": "src/lib.rs",
                    "lineRange": "115:154",
                    "read": "src/lib.rs:115:154",
                    "lineCount": 40,
                    "reason": "split",
                }
            ],
            "frontier": [
                {
                    "id": "W",
                    "kind": "window",
                    "target": "src/lib.rs@115:154",
                    "read": "src/lib.rs:115:154",
                    "action": "code",
                    "rank": 1,
                    "reason": "split",
                }
            ],
            "avoid": ["repeat-wide-read", "manual-window-scan", "raw-read"],
            "omit": ["code"],
        }
        self.assertEqual([], self.validation_errors(packet))

    def test_read_plan_symbol_frontier_packet_is_valid_without_windows(self) -> None:
        packet = semantic_read_minimal_packet()
        packet.pop("sourceWindows")
        packet["selector"] = "src/lib.rs:1:80"
        packet["readPlan"] = {
            "mode": "range-frontier",
            "code": False,
            "reason": "wide-selector",
            "maxWindowLines": 40,
            "algorithm": "symbol-frontier",
            "ranges": [
                {
                    "path": "src/lib.rs",
                    "requested": "1:80",
                    "selected": "1:80",
                    "matched": "1:80",
                    "coverage": "full",
                    "density": "normal",
                }
            ],
            "symbols": [
                {
                    "itemName": "load",
                    "itemKind": "fn",
                    "lineRange": "6:6",
                    "read": "src/lib.rs:6:6",
                }
            ],
            "frontier": [
                {
                    "id": "S",
                    "kind": "symbol",
                    "target": "src/lib.rs@6:6",
                    "read": "src/lib.rs:6:6",
                    "action": "code",
                    "rank": 1,
                    "reason": "parser-item",
                }
            ],
            "avoid": ["repeat-wide-read", "manual-window-scan", "raw-read"],
            "omit": ["code"],
        }
        self.assertEqual([], self.validation_errors(packet))

    def test_read_packet_rejects_root_hook_protocol(self) -> None:
        packet = semantic_read_minimal_packet()
        packet["protocolId"] = "agent.semantic-protocols.hook"
        errors = self.validation_errors(packet)
        self.assertTrue(any("was expected" in message for message in errors))

    def test_read_packet_requires_query_method(self) -> None:
        packet = semantic_read_minimal_packet()
        packet["method"] = "agent/read"
        errors = self.validation_errors(packet)
        self.assertTrue(any("does not match" in message for message in errors))

    def test_selector_rejects_rank_prefixed_path(self) -> None:
        packet = semantic_read_minimal_packet()
        packet["selector"] = "0:src/lib.rs"
        errors = self.validation_errors(packet)
        self.assertTrue(
            any(
                "does not match" in message
                or "is not valid under any of the given schemas" in message
                for message in errors
            )
        )

    def test_window_read_locator_rejects_rank_prefix_path(self) -> None:
        packet = semantic_read_minimal_packet()
        windows = packet["sourceWindows"]
        assert isinstance(windows, list)
        window = windows[0]
        assert isinstance(window, dict)
        window["read"] = "0:src/lib.rs:6:6"
        errors = self.validation_errors(packet)
        self.assertTrue(any("does not match" in message for message in errors))


if __name__ == "__main__":
    unittest.main()
