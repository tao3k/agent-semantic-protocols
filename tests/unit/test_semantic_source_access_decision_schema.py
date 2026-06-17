"""Validate the no-daemon source access decision schema."""

from __future__ import annotations

import json
import unittest
from pathlib import Path

from jsonschema import Draft202012Validator


_REPO_ROOT = Path(__file__).resolve().parents[2]


def hard_fs_deny() -> dict[str, object]:
    return {
        "schemaId": "agent.semantic-protocols.source-access.decision",
        "schemaVersion": "1",
        "protocolId": "agent.semantic-protocols.source-access",
        "protocolVersion": "1",
        "client": "codex",
        "boundary": "codex-fs-api",
        "operation": "read-file",
        "enforcement": "hard",
        "decision": "deny",
        "reasonKind": "direct-source-read",
        "sourceBytesReturned": False,
        "modelVisibleBytesReturned": False,
        "authorization": "none",
        "languageIds": ["rust"],
        "subject": {
            "rpcMethod": "fs/readFile",
            "paths": ["src/lib.rs"],
        },
        "routes": [
            {
                "languageId": "rust",
                "providerId": "rs-harness",
                "binary": "asp",
                "kind": "query",
                "argv": [
                    "asp",
                    "rust",
                    "query",
                    "--from-hook",
                    "direct-source-read",
                    "--selector",
                    "src/lib.rs",
                    "--code",
                    ".",
                ],
            }
        ],
        "message": "direct-source-read denied; route: asp rust query --from-hook direct-source-read --selector src/lib.rs --code .",
    }


class SemanticSourceAccessDecisionSchemaTests(unittest.TestCase):
    def setUp(self) -> None:
        schema_path = (
            _REPO_ROOT / "schemas" / "semantic-source-access-decision.v1.schema.json"
        )
        with schema_path.open("r", encoding="utf-8") as handle:
            self.validator = Draft202012Validator(json.load(handle))

    def validation_errors(self, decision: dict[str, object]) -> list[str]:
        return [error.message for error in self.validator.iter_errors(decision)]

    def test_hard_codex_fs_deny_is_valid(self) -> None:
        self.assertEqual([], self.validation_errors(hard_fs_deny()))

    def test_read_directory_source_enumeration_reason_kind_is_valid(self) -> None:
        decision = hard_fs_deny()
        decision.update(
            {
                "boundary": "codex-tool-action",
                "operation": "read-directory",
                "reasonKind": "source-directory-enumeration",
                "subject": {
                    "toolName": "command_execution.command_action.listFiles",
                    "command": "ls crates/agent-semantic-hook/src",
                    "paths": ["crates/agent-semantic-hook/src"],
                },
                "routes": [
                    {
                        "languageId": "rust",
                        "providerId": "rs-harness",
                        "binary": "asp",
                        "kind": "ingest",
                        "argv": [
                            "asp",
                            "rust",
                            "search",
                            "ingest",
                            "items",
                            "tests",
                            "--workspace",
                            ".",
                            "--view",
                            "seeds",
                        ],
                    }
                ],
                "message": "source-directory-enumeration denied; route: asp rust search ingest items tests --workspace . --view seeds",
            }
        )

        self.assertEqual([], self.validation_errors(decision))

    def test_shell_egress_suppression_can_hide_subprocess_source_output(self) -> None:
        decision = hard_fs_deny()
        decision.update(
            {
                "boundary": "codex-shell-egress",
                "operation": "tool-output",
                "enforcement": "egress",
                "decision": "suppress",
                "reasonKind": "bulk-source-dump",
                "sourceBytesReturned": True,
                "modelVisibleBytesReturned": False,
                "subject": {
                    "toolName": "Bash",
                    "command": "sed -n '1,120p' src/lib.rs",
                    "paths": ["src/lib.rs"],
                    "outputDigest": "sha256:source-like-output",
                },
                "message": "bulk-source-dump suppressed; use asp rust query --from-hook direct-source-read --selector src/lib.rs --code .",
            }
        )

        self.assertEqual([], self.validation_errors(decision))

    def test_provider_capability_allow_is_valid(self) -> None:
        decision = hard_fs_deny()
        decision.update(
            {
                "boundary": "codex-tool-action",
                "operation": "read-file",
                "enforcement": "hard",
                "decision": "allow",
                "reasonKind": "provider-authorized",
                "sourceBytesReturned": True,
                "modelVisibleBytesReturned": True,
                "authorization": "provider-capability",
                "providerId": "rs-harness",
                "subject": {
                    "toolName": "asp",
                    "command": "asp rust query --from-hook direct-source-read --selector src/lib.rs --code .",
                    "paths": ["src/lib.rs"],
                },
                "message": "provider-capability allowed compact source access.",
            }
        )

        self.assertEqual([], self.validation_errors(decision))

    def test_mcp_boundary_is_out_of_scope(self) -> None:
        decision = hard_fs_deny()
        decision["boundary"] = "mcp-resource"
        errors = self.validation_errors(decision)

        self.assertTrue(any("is not one of" in error for error in errors))

    def test_deny_cannot_return_model_visible_source_bytes(self) -> None:
        decision = hard_fs_deny()
        decision["sourceBytesReturned"] = True
        decision["modelVisibleBytesReturned"] = True
        errors = self.validation_errors(decision)

        self.assertTrue(any("False was expected" in error for error in errors))

    def test_suppress_cannot_return_model_visible_source_bytes(self) -> None:
        decision = hard_fs_deny()
        decision.update(
            {
                "boundary": "codex-shell-egress",
                "operation": "tool-output",
                "enforcement": "egress",
                "decision": "suppress",
                "reasonKind": "bulk-source-dump",
                "sourceBytesReturned": True,
                "modelVisibleBytesReturned": True,
                "subject": {
                    "toolName": "Bash",
                    "command": "sed -n '1,120p' src/lib.rs",
                    "paths": ["src/lib.rs"],
                    "outputDigest": "sha256:source-like-output",
                },
            }
        )
        errors = self.validation_errors(decision)

        self.assertTrue(any("False was expected" in error for error in errors))

    def test_provider_authorized_requires_top_level_provider_id(self) -> None:
        decision = hard_fs_deny()
        decision.update(
            {
                "boundary": "codex-tool-action",
                "operation": "read-file",
                "decision": "allow",
                "reasonKind": "provider-authorized",
                "sourceBytesReturned": True,
                "modelVisibleBytesReturned": True,
                "authorization": "provider-capability",
                "subject": {
                    "toolName": "asp",
                    "command": "asp rust query --from-hook direct-source-read --selector src/lib.rs --code .",
                    "paths": ["src/lib.rs"],
                },
            }
        )
        errors = self.validation_errors(decision)

        self.assertTrue(
            any("'providerId' is a required property" in error for error in errors)
        )

    def test_unknown_enforcement_mode_is_rejected(self) -> None:
        decision = hard_fs_deny()
        decision["enforcement"] = "kernel"
        errors = self.validation_errors(decision)

        self.assertTrue(any("is not one of" in error for error in errors))


if __name__ == "__main__":
    unittest.main()
