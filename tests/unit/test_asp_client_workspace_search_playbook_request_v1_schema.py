# SPDX-FileCopyrightText: 2026 tao3k team and Contributors
#
# SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

from pathlib import Path

import jsonschema
import pytest

from unit.schema_validation import schema_validator_for


ROOT = Path(__file__).resolve().parents[2]
SCHEMA_PATH = ROOT / "schemas/asp-client-workspace-search-playbook-request.v1.schema.json"
VALIDATOR = schema_validator_for(SCHEMA_PATH)


def resident_plan() -> dict:
    digest = "blake3-256:" + "1" * 64
    return {
        "schemaId": "agent.semantic-protocols.resident-syntax-query-plan",
        "schemaVersion": "1",
        "profileId": "asp.enhanced-tree-sitter-query.v1",
        "planDigest": digest,
        "queryDigest": digest,
        "languageId": "rust",
        "providerId": "asp-rust",
        "parserAbiDigest": digest,
        "queryGrammarDigest": digest,
        "operatorTableDigest": digest,
        "capabilityTableDigest": digest,
        "generationDigest": digest,
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


def request() -> dict:
    return {
        "schemaId": "agent.semantic-protocols.asp-client-workspace-search-playbook-request",
        "schemaVersion": "1",
        "language": "rust|python",
        "rg": [["-n", "-g", "*.rs", "runtime|transport"]],
        "tantivy": [['title:"runtime transport"^2', "OR", "body:client"]],
        "composition": {
            "operator": "intersect",
            "children": [leaf("rg"), leaf("tantivy")],
        },
        "clauseOrder": [
            {"axis": "rg", "blockIndex": 0},
            {"axis": "tantivy", "blockIndex": 0},
        ],
    }


def leaf(axis: str, block_index: int = 0) -> dict:
    return {
        "operator": "leaf",
        "clause": {"axis": axis, "blockIndex": block_index},
    }


def test_workspace_playbook_request_v1_accepts_executable_search() -> None:
    VALIDATOR.validate(request())


def test_workspace_playbook_request_v1_accepts_topology_owner_membership() -> None:
    value = {
        "schemaId": "agent.semantic-protocols.asp-client-workspace-search-playbook-request",
        "schemaVersion": "1",
        "language": "rust",
        "topology": [
            {
                "kind": "file",
                "pathPrefix": "crates/",
                "extension": "rs",
                "pathGlob": "crates/**/src/*.rs",
            }
        ],
        "composition": leaf("topology"),
        "clauseOrder": [{"axis": "topology", "blockIndex": 0}],
    }
    VALIDATOR.validate(value)


def test_workspace_playbook_request_v1_rejects_topology_scope_escape() -> None:
    value = {
        "schemaId": "agent.semantic-protocols.asp-client-workspace-search-playbook-request",
        "schemaVersion": "1",
        "language": "rust",
        "topology": [{"kind": "file", "pathPrefix": "../outside"}],
        "composition": leaf("topology"),
        "clauseOrder": [{"axis": "topology", "blockIndex": 0}],
    }
    with pytest.raises(jsonschema.ValidationError):
        VALIDATOR.validate(value)


def test_workspace_playbook_request_v1_requires_a_producer_axis() -> None:
    value = request()
    del value["language"]
    with pytest.raises(jsonschema.ValidationError):
        VALIDATOR.validate(value)


def test_workspace_playbook_request_v1_accepts_document_producers() -> None:
    value = request()
    del value["language"]
    value["documents"] = "org|md"
    VALIDATOR.validate(value)


@pytest.mark.parametrize("fact_axis", ["syntax", "nativeSyntax"])
def test_workspace_playbook_request_v1_accepts_structural_only_search(
    fact_axis: str,
) -> None:
    value = {
        "schemaId": "agent.semantic-protocols.asp-client-workspace-search-playbook-request",
        "schemaVersion": "1",
        "language": "rust",
        fact_axis: (
            [{"producer": "rust", "plan": resident_plan()}]
            if fact_axis == "syntax"
            else ["rust://src/lib.rs#item/function/main"]
        ),
        "composition": leaf(
            "syntax" if fact_axis == "syntax" else "native-syntax"
        ),
        "clauseOrder": [
            {
                "axis": "syntax" if fact_axis == "syntax" else "native-syntax",
                "blockIndex": 0,
            }
        ],
    }
    VALIDATOR.validate(value)


def test_workspace_playbook_request_v1_rejects_contract_only_invocation() -> None:
    value = {
        "schemaId": "agent.semantic-protocols.asp-client-workspace-search-playbook-request",
        "schemaVersion": "1",
        "language": "rust",
    }
    with pytest.raises(jsonschema.ValidationError):
        VALIDATOR.validate(value)


def test_workspace_playbook_request_v1_rejects_unknown_axis() -> None:
    value = request()
    value["unknownAxis"] = [["argument"]]
    with pytest.raises(jsonschema.ValidationError):
        VALIDATOR.validate(value)


@pytest.mark.parametrize("missing", ["rg", "tantivy"])
def test_workspace_playbook_request_v1_admits_independent_retrieval_engine(
    missing: str,
) -> None:
    value = request()
    del value[missing]
    value["clauseOrder"] = [
        clause for clause in value["clauseOrder"] if clause["axis"] != missing
    ]
    remaining = "tantivy" if missing == "rg" else "rg"
    value["composition"] = leaf(remaining)
    VALIDATOR.validate(value)


def test_workspace_playbook_request_v1_keeps_syntax_query_and_native_selector_distinct() -> None:
    value = {
        "schemaId": "agent.semantic-protocols.asp-client-workspace-search-playbook-request",
        "schemaVersion": "1",
        "language": "rust",
        "rg": [["-n", "Registry"]],
        "tantivy": [["title:\"Registry\"^2", "OR", "body:implementation"]],
        "syntax": [
            {
                "producer": "rust",
                "plan": resident_plan(),
            }
        ],
        "nativeSyntax": [
            "rust://src/registry.rs#item/implementation/type/Registry"
        ],
        "composition": {
            "operator": "chain",
            "children": [
                {
                    "operator": "intersect",
                    "children": [leaf("rg"), leaf("tantivy")],
                },
                leaf("syntax"),
                leaf("native-syntax"),
            ],
        },
        "clauseOrder": [
            {"axis": "rg", "blockIndex": 0},
            {"axis": "tantivy", "blockIndex": 0},
            {"axis": "syntax", "blockIndex": 0},
            {"axis": "native-syntax", "blockIndex": 0},
        ],
    }
    VALIDATOR.validate(value)


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
        VALIDATOR.validate(value)


@pytest.mark.parametrize("legacy_field", ["query", "languages", "documents", "next"])
def test_workspace_playbook_request_v1_rejects_reasoning_and_continuation_fields(
    legacy_field: str,
) -> None:
    value = request()
    value[legacy_field] = "must remain agent-owned"
    with pytest.raises(jsonschema.ValidationError):
        VALIDATOR.validate(value)
