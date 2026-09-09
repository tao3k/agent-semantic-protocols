# SPDX-FileCopyrightText: 2026 tao3k team and Contributors
#
# SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

"""Workspace native Syntax Query contract tests."""

from pathlib import Path

import jsonschema
import pytest

from unit.schema_validation import schema_validator_for


ROOT = Path(__file__).resolve().parents[2]
SCHEMAS = ROOT / "schemas"


def test_workspace_syntax_query_request_preserves_pipe_and_native_argv() -> None:
    packet = {
        "schemaId": "agent.semantic-protocols.asp-client-workspace-syntax-query-request",
        "schemaVersion": "1",
        "languages": "rust|python",
        "documents": "org|md",
        "workspace": "workspace-main",
        "syntax": [
            {
                "producer": "rust",
                "argv": [
                    "--treesitter-query",
                    '((identifier) @symbol (#match? @symbol "Runtime|Client"))',
                ],
            }
        ],
        "projection": "matches",
    }
    schema_validator_for(
        SCHEMAS / "asp-client-workspace-syntax-query-request.v1.schema.json"
    ).validate(packet)


def test_workspace_syntax_query_producer_is_sufficient_without_calibration() -> None:
    packet = {
        "schemaId": "agent.semantic-protocols.asp-client-workspace-syntax-query-request",
        "schemaVersion": "1",
        "syntax": [
            {
                "producer": "rust",
                "argv": ["--treesitter-query", "((function_item) @function)"],
            }
        ],
        "projection": "matches",
    }
    schema_validator_for(
        SCHEMAS / "asp-client-workspace-syntax-query-request.v1.schema.json"
    ).validate(packet)


@pytest.mark.parametrize("workspace", [".", "../other", "/tmp/other"])
def test_workspace_syntax_query_rejects_filesystem_paths(workspace: str) -> None:
    packet = {
        "schemaId": "agent.semantic-protocols.asp-client-workspace-syntax-query-request",
        "schemaVersion": "1",
        "workspace": workspace,
        "syntax": [{"producer": "rust", "argv": ["query"]}],
        "projection": "matches",
    }
    with pytest.raises(jsonschema.ValidationError):
        schema_validator_for(
            SCHEMAS / "asp-client-workspace-syntax-query-request.v1.schema.json"
        ).validate(packet)


def test_workspace_syntax_query_response_is_top3_selector_only_evidence() -> None:
    packet = {
        "schemaId": "agent.semantic-protocols.asp-client-workspace-syntax-query-response",
        "schemaVersion": "1",
        "state": "ready",
        "evidence": [
            {
                "owner": "src/lib.rs",
                "selector": "rust://src/lib.rs#item/function/run",
                "relation": "syntax-capture:function.name",
            }
        ],
    }
    schema_validator_for(
        SCHEMAS / "asp-client-workspace-syntax-query-response.v1.schema.json"
    ).validate(packet)
    encoded = str(packet)
    assert "recommendedNext" not in encoded
    assert "nextCommand" not in encoded
    assert "sourceContent" not in encoded
    assert "digest" not in encoded
