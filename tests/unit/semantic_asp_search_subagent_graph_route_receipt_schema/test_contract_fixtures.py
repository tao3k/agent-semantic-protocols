from copy import deepcopy
import json
from pathlib import Path

import jsonschema


SCHEMA_PATH = (
    Path(__file__).resolve().parents[3]
    / "schemas"
    / "semantic-asp-search-subagent-graph-route-receipt.v1.schema.json"
)
SCHEMA = json.loads(SCHEMA_PATH.read_text())
VALIDATOR = jsonschema.Draft202012Validator(SCHEMA)


def valid_receipt() -> dict:
    selector = "rust://crates/agent-semantic-runtime#item/struct/RuntimeServingEndpoint"
    return {
        "schemaId": "semantic-asp-search-subagent-graph-route-receipt.v1",
        "schemaVersion": "1",
        "protocolId": "agent.semantic-protocols.search-subagent",
        "protocolVersion": "1",
        "kind": "asp-search-subagent",
        "receiptSchema": "asp-search-subagent.graph.v1",
        "state": "candidates",
        "queryGrammar": "asp query --selector <exact-selector> --projection <callable-skeleton|source>",
        "evidence": [
            {
                "id": "E1",
                "kind": "item",
                "role": "primary",
                "owner": "crates/agent-semantic-runtime",
                "item": "struct/RuntimeServingEndpoint",
                "selector": selector,
                "matchedBy": ["rg:0", "syntax:0", "graph:0"],
                "relation": "publishes-runtime-endpoint",
                "state": "selector-ready",
            },
            {
                "id": "E2",
                "kind": "test",
                "role": "guard",
                "owner": "crates/agent-semantic-runtime/tests",
                "item": "test/runtime_transport_owner",
                "selector": "rust://crates/agent-semantic-runtime/tests#item/test/runtime_transport_owner",
                "matchedBy": ["fd:0", "syntax:0"],
                "relation": "guards",
            },
        ],
        "edges": [
            {"from": "E2", "relation": "guards", "to": "E1"},
        ],
    }


def errors(receipt: dict) -> list[jsonschema.ValidationError]:
    return list(VALIDATOR.iter_errors(receipt))


def test_schema_is_valid_draft_2020_12() -> None:
    jsonschema.Draft202012Validator.check_schema(SCHEMA)


def test_accepts_bounded_selector_only_handoff() -> None:
    assert errors(valid_receipt()) == []


def test_accepts_successful_empty_result_without_invented_candidates() -> None:
    receipt = valid_receipt()
    receipt["state"] = "empty"
    del receipt["queryGrammar"]
    receipt["evidence"] = []
    receipt["edges"] = []
    assert errors(receipt) == []


def test_accepts_typed_unavailable_result() -> None:
    receipt = valid_receipt()
    receipt["state"] = "unavailable"
    del receipt["queryGrammar"]
    receipt["evidence"] = []
    receipt["edges"] = []
    receipt["stage"] = "runtime"
    receipt["reasonKind"] = "transport-unavailable"
    assert errors(receipt) == []


def test_rejects_ambiguous_no_output_state() -> None:
    receipt = valid_receipt()
    receipt["state"] = "no-output"
    receipt["evidence"] = []
    receipt["edges"] = []
    assert errors(receipt)


def test_rejects_evidence_without_owner() -> None:
    receipt = valid_receipt()
    del receipt["evidence"][0]["owner"]
    assert errors(receipt)


def test_rejects_evidence_without_relation() -> None:
    receipt = valid_receipt()
    del receipt["evidence"][0]["relation"]
    assert errors(receipt)


def test_requires_query_grammar_only_for_candidates() -> None:
    receipt = valid_receipt()
    del receipt["queryGrammar"]
    assert errors(receipt)

    receipt = valid_receipt()
    receipt["queryGrammar"] = "asp query --selector <exact-selector>"
    assert errors(receipt)

    receipt = valid_receipt()
    receipt["state"] = "empty"
    receipt["evidence"] = []
    assert errors(receipt)


def test_rejects_evidence_without_item_or_supporting_clauses() -> None:
    for field in ("item", "matchedBy"):
        receipt = valid_receipt()
        del receipt["evidence"][0][field]
        assert errors(receipt)


def test_rejects_line_range_selector_identity() -> None:
    receipt = valid_receipt()
    receipt["evidence"][0]["selector"] = "src/runtime.rs:20-42"
    assert errors(receipt)


def test_rejects_unbounded_evidence_dump() -> None:
    receipt = valid_receipt()
    receipt["evidence"] = [deepcopy(receipt["evidence"][0]) for _ in range(31)]
    assert errors(receipt)


def test_rejects_source_bearing_escape_fields() -> None:
    for field in ("source", "sourceBody", "snippet", "excerpt", "fields"):
        receipt = valid_receipt()
        receipt[field] = "forbidden"
        assert errors(receipt), field

        receipt = valid_receipt()
        receipt["evidence"][0][field] = "forbidden"
        assert errors(receipt), field


def test_rejects_prescribed_next_action() -> None:
    receipt = valid_receipt()
    receipt["next"] = "asp query --selector rust://workspace/item/RuntimeServingEndpoint"
    assert errors(receipt)
