# SPDX-FileCopyrightText: 2026 tao3k team and Contributors
#
# SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

"""Validate shared semantic handle schema examples."""

from __future__ import annotations

import copy
import unittest
from pathlib import Path

from unit.schema_validation import schema_validator_for


_REPO_ROOT = Path(__file__).resolve().parents[2]


def minimal_policy_handle() -> dict[str, object]:
    return {
        "id": "PY-PROJ-R001",
        "kind": "policy-rule",
        "source": "provider-policy",
        "title": "Prefer src layout for packaged Python projects",
        "aliases": ["src-layout", "packaged-project-layout"],
        "labels": ["layout", "project-policy"],
        "status": "active",
        "ownerPath": "src/asp_python/_project_policy_catalog.py",
        "implementationOwnerPath": "src/asp_python/_project_policy_layout.py",
        "testPaths": ["tests/unit/asp_python/project_policy/test_layout.py"],
        "locations": [
            {
                "path": "src/asp_python/_project_policy_catalog.py",
                "lineRange": "14:14",
            }
        ],
        "queryTerms": ["PY-PROJ-R001", "src-layout"],
        "relations": [
            {
                "kind": "implements",
                "target": "src/asp_python/_project_policy_layout.py",
            }
        ],
        "fields": {"pack": "project", "severity": "warning"},
    }


def minimal_handle_packet() -> dict[str, object]:
    return {
        "schemaId": "agent.semantic-protocols.semantic-handle",
        "schemaVersion": "1",
        "protocolId": "agent.semantic-protocols.semantic-language",
        "protocolVersion": "1",
        "languageId": "python",
        "providerId": "asp-python",
        "projectRoot": ".",
        "scope": "policy",
        "query": "PY-PROJ-R001",
        "handles": [minimal_policy_handle()],
        "notes": [],
    }


class SemanticHandleSchemaTests(unittest.TestCase):
    def setUp(self) -> None:
        schema_path = _REPO_ROOT / "schemas" / "semantic-handle.v1.schema.json"
        self.validator = schema_validator_for(schema_path)

    def validation_errors(self, packet: dict[str, object]) -> list[str]:
        return [error.message for error in self.validator.iter_errors(packet)]

    def test_policy_handle_packet_is_valid(self) -> None:
        self.assertEqual([], self.validation_errors(minimal_handle_packet()))

    def test_handle_rejects_rank_prefixed_owner_path(self) -> None:
        packet = minimal_handle_packet()
        handle = copy.deepcopy(minimal_policy_handle())
        handle["ownerPath"] = "1:src/lib.rs"
        packet["handles"] = [handle]
        errors = self.validation_errors(packet)
        self.assertTrue(any("does not match" in message for message in errors))


if __name__ == "__main__":
    unittest.main()
