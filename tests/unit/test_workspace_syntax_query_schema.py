# SPDX-FileCopyrightText: 2026 tao3k team and Contributors
#
# SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

"""Workspace native Syntax Query contract tests."""

from pathlib import Path

import json
import jsonschema
import pytest

from unit.schema_validation import schema_validator_for


ROOT = Path(__file__).resolve().parents[2]
SCHEMAS = ROOT / "schemas"
DIGEST = "blake3-256:" + "1" * 64


def resident_plan() -> dict[str, object]:
    return {
        "schemaId": "agent.semantic-protocols.resident-syntax-query-plan",
        "schemaVersion": "1",
        "profileId": "asp.enhanced-tree-sitter-query.v1",
        "planDigest": DIGEST,
        "queryDigest": DIGEST,
        "languageId": "rust",
        "providerId": "asp-rust",
        "parserAbiDigest": DIGEST,
        "queryGrammarDigest": DIGEST,
        "operatorTableDigest": DIGEST,
        "capabilityTableDigest": DIGEST,
        "generationDigest": DIGEST,
        "patterns": [
            {
                "index": 0,
                "captures": [
                    {
                        "name": "item",
                        "residentFactPath": "selector",
                        "cardinality": {"minimum": 1, "maximum": 1},
                        "capabilityRowId": "rust.capture.item",
                    }
                ],
                "structure": {
                    "kind": "true",
                    "origin": {
                        "kind": "capture",
                        "capabilityRowId": "rust.capture.item",
                    },
                },
                "predicates": [],
            }
        ],
        "selectedFields": ["selector"],
        "requiredCapabilityRows": ["rust.capture.item"],
        "regexPrograms": [],
    }


def test_workspace_syntax_query_request_preserves_pipe_and_resident_plan() -> None:
    packet = {
        "schemaId": "agent.semantic-protocols.asp-client-workspace-syntax-query-request",
        "schemaVersion": "1",
        "languages": "rust|python",
        "documents": "org|md",
        "workspace": "workspace-main",
        "syntax": [
            {
                "producer": "rust",
                "plan": resident_plan(),
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
                "plan": resident_plan(),
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
        "syntax": [{"producer": "rust", "plan": resident_plan()}],
        "projection": "matches",
    }
    with pytest.raises(jsonschema.ValidationError):
        schema_validator_for(
            SCHEMAS / "asp-client-workspace-syntax-query-request.v1.schema.json"
        ).validate(packet)


def test_workspace_syntax_query_response_contains_the_selected_match_fields() -> None:
    packet = {
        "schemaId": "agent.semantic-protocols.asp-client-workspace-syntax-query-response",
        "schemaVersion": "1",
        "state": "ready",
        "evidence": [
            {
                "owner": "src/lib.rs",
                "selector": "rust://src/lib.rs#item/function/run",
                "capture": "item",
                "relation": "syntax-capture:function.name",
                "selected": {
                    "kind": "function",
                    "name": "run",
                    "selector": "rust://src/lib.rs#item/function/run",
                },
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


def test_workspace_syntax_plan_context_is_identity_only() -> None:
    request = {
        "schemaId": "agent.semantic-protocols.asp-client-workspace-syntax-plan-context-request",
        "schemaVersion": "1",
        "producer": "rust",
    }
    schema_validator_for(
        SCHEMAS / "asp-client-workspace-syntax-plan-context-request.v1.schema.json"
    ).validate(request)
    capability = json.loads(
        (
            ROOT
            / "languages/asp-rust/tree-sitter/tree-sitter-rust/enhanced-query-capabilities.v1.json"
        ).read_text(encoding="utf-8")
    )
    response = {
        "schemaId": "agent.semantic-protocols.asp-client-workspace-syntax-plan-context-response",
        "schemaVersion": "1",
        "generationDigest": DIGEST,
        "capability": capability,
    }
    schema_validator_for(
        SCHEMAS / "asp-client-workspace-syntax-plan-context-response.v1.schema.json"
    ).validate(response)
    assert "querySource" not in request
    assert "plan" not in response
