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
        "language": "rust|python",
        "rg": [["-n", "-g", "*.rs", "runtime|transport", "crates"]],
        "tantivy": [['title:"runtime transport"^2', "OR", "body:client"]],
        "clauseOrder": [
            {"axis": "rg", "blockIndex": 0},
            {"axis": "tantivy", "blockIndex": 0},
        ],
    }


def test_workspace_playbook_request_v1_accepts_executable_search() -> None:
    jsonschema.Draft202012Validator(SCHEMA).validate(request())


def test_workspace_playbook_request_v1_requires_a_producer_axis() -> None:
    value = request()
    del value["language"]
    with pytest.raises(jsonschema.ValidationError):
        jsonschema.Draft202012Validator(SCHEMA).validate(value)


def test_workspace_playbook_request_v1_accepts_document_producers() -> None:
    value = request()
    del value["language"]
    value["documents"] = "org|md"
    jsonschema.Draft202012Validator(SCHEMA).validate(value)


@pytest.mark.parametrize("fact_axis", ["syntax", "nativeSyntax"])
def test_workspace_playbook_request_v1_rejects_fact_extraction_without_file_context(
    fact_axis: str,
) -> None:
    value = {
        "schemaId": "agent.semantic-protocols.asp-client-workspace-search-playbook-request",
        "schemaVersion": "1",
        "language": "rust",
        fact_axis: (
            [{"producer": "rust", "argv": ["query"]}]
            if fact_axis == "syntax"
            else ["rust://src/lib.rs#item/function/main"]
        ),
        "clauseOrder": [
            {
                "axis": "syntax" if fact_axis == "syntax" else "native-syntax",
                "blockIndex": 0,
            }
        ],
    }
    with pytest.raises(jsonschema.ValidationError):
        jsonschema.Draft202012Validator(SCHEMA).validate(value)


def test_workspace_playbook_request_v1_rejects_contract_only_invocation() -> None:
    value = {
        "schemaId": "agent.semantic-protocols.asp-client-workspace-search-playbook-request",
        "schemaVersion": "1",
        "language": "rust",
    }
    with pytest.raises(jsonschema.ValidationError):
        jsonschema.Draft202012Validator(SCHEMA).validate(value)


def test_workspace_playbook_request_v1_rejects_unknown_axis() -> None:
    value = request()
    value["unknownAxis"] = [["argument"]]
    with pytest.raises(jsonschema.ValidationError):
        jsonschema.Draft202012Validator(SCHEMA).validate(value)


@pytest.mark.parametrize("missing", ["rg", "tantivy"])
def test_workspace_playbook_request_v1_pairs_rg_and_tantivy(missing: str) -> None:
    value = request()
    del value[missing]
    value["clauseOrder"] = [
        clause for clause in value["clauseOrder"] if clause["axis"] != missing
    ]
    with pytest.raises(jsonschema.ValidationError):
        jsonschema.Draft202012Validator(SCHEMA).validate(value)


def test_workspace_playbook_request_v1_keeps_syntax_query_and_native_selector_distinct() -> None:
    value = {
        "schemaId": "agent.semantic-protocols.asp-client-workspace-search-playbook-request",
        "schemaVersion": "1",
        "language": "rust",
        "rg": [["-n", "Registry", "src"]],
        "tantivy": [["title:\"Registry\"^2", "OR", "body:implementation"]],
        "syntax": [
            {
                "producer": "rust",
                "argv": ["--treesitter-query", "((function_item) @function)"],
            }
        ],
        "nativeSyntax": [
            "rust://src/registry.rs#item/implementation/type/Registry"
        ],
        "clauseOrder": [
            {"axis": "rg", "blockIndex": 0},
            {"axis": "tantivy", "blockIndex": 0},
            {"axis": "syntax", "blockIndex": 0},
            {"axis": "native-syntax", "blockIndex": 0},
        ],
    }
    jsonschema.Draft202012Validator(SCHEMA).validate(value)


@pytest.mark.parametrize(
    ("field", "invalid"),
    [
        ("workspace", "../other"),
        ("syntax", [{"producer": "rust|python", "argv": ["query"]}]),
        ("graph", [{"language": "cypher", "argv": ["MATCH (n) RETURN n"]}]),
    ],
)
def test_workspace_playbook_request_v1_rejects_invalid_registered_values(
    field: str, invalid: object
) -> None:
    value = request()
    value[field] = invalid
    if field == "syntax":
        value["clauseOrder"].append({"axis": "syntax", "blockIndex": 0})
    if field == "graph":
        value["clauseOrder"].append({"axis": "graph", "blockIndex": 0})
    with pytest.raises(jsonschema.ValidationError):
        jsonschema.Draft202012Validator(SCHEMA).validate(value)


@pytest.mark.parametrize("legacy_field", ["query", "languages", "documents", "next"])
def test_workspace_playbook_request_v1_rejects_reasoning_and_continuation_fields(
    legacy_field: str,
) -> None:
    value = request()
    value[legacy_field] = "must remain agent-owned"
    with pytest.raises(jsonschema.ValidationError):
        jsonschema.Draft202012Validator(SCHEMA).validate(value)
