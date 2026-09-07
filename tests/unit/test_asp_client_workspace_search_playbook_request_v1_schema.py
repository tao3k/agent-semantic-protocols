# SPDX-FileCopyrightText: 2026 tao3k team and Contributors
#
# SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

import json
from pathlib import Path

import jsonschema
import pytest


ROOT = Path(__file__).resolve().parents[2]
SCHEMA = json.loads(
    (ROOT / "schemas/asp-client-workspace-search-playbook-request.v1.schema.json").read_text()
)


def request() -> dict:
    return {
        "schemaId": "agent.semantic-protocols.asp-client-workspace-search-playbook-request",
        "schemaVersion": "1",
        "languages": "rust|python",
        "rg": [["-n", "runtime|transport", "."]],
        "clauseOrder": [{"axis": "rg", "blockIndex": 0}],
    }


def test_workspace_playbook_request_v1_accepts_executable_search() -> None:
    jsonschema.Draft202012Validator(SCHEMA).validate(request())


def test_workspace_playbook_request_v1_rejects_contract_only_invocation() -> None:
    value = {
        "schemaId": "agent.semantic-protocols.asp-client-workspace-search-playbook-request",
        "schemaVersion": "1",
        "languages": "rust",
    }
    with pytest.raises(jsonschema.ValidationError):
        jsonschema.Draft202012Validator(SCHEMA).validate(value)


def test_workspace_playbook_request_v1_keeps_syntax_query_and_native_selector_distinct() -> None:
    value = request()
    value["syntax"] = [
        {
            "producer": "python",
            "argv": [
                "--treesitter-query",
                '((identifier) @symbol (#match? @symbol "runtime_client|transport"))',
            ],
        }
    ]
    value["nativeSyntax"] = [
        "rust://src/registry.rs#item/implementation/type/Registry"
    ]
    value["clauseOrder"].extend(
        [
            {"axis": "syntax", "blockIndex": 0},
            {"axis": "native-syntax", "blockIndex": 0},
        ]
    )
    jsonschema.Draft202012Validator(SCHEMA).validate(value)


@pytest.mark.parametrize(
    "native_selector",
    [
        "python",
        "--treesitter-query",
        "rust://src/registry.rs",
        "rust://src/registry.rs#item/with whitespace",
    ],
)
def test_workspace_playbook_request_v1_rejects_non_selector_native_syntax(
    native_selector: str,
) -> None:
    value = request()
    value["nativeSyntax"] = [native_selector]
    value["clauseOrder"].append({"axis": "native-syntax", "blockIndex": 0})
    with pytest.raises(jsonschema.ValidationError):
        jsonschema.Draft202012Validator(SCHEMA).validate(value)


@pytest.mark.parametrize("legacy_field", ["intent", "query", "language", "next"])
def test_workspace_playbook_request_v1_rejects_reasoning_and_continuation_fields(
    legacy_field: str,
) -> None:
    value = request()
    value[legacy_field] = "must remain agent-owned"
    with pytest.raises(jsonschema.ValidationError):
        jsonschema.Draft202012Validator(SCHEMA).validate(value)
