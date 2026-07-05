from copy import deepcopy
from pathlib import Path

import jsonschema


REPO_ROOT = Path(__file__).resolve().parents[3]
SCHEMA_PATH = (
    REPO_ROOT
    / "schemas"
    / "semantic-asp-search-subagent-graph-route-receipt.v1.schema.json"
)


def schema_document() -> dict:
    import json

    return json.loads(SCHEMA_PATH.read_text())


def valid_receipt() -> dict:
    return {
        "schemaId": "semantic-asp-search-subagent-graph-route-receipt.v1",
        "schemaVersion": "1",
        "protocolId": "agent.semantic-protocols.search-subagent",
        "protocolVersion": "1",
        "kind": "asp-search-subagent",
        "receiptSchema": "asp-search-subagent.graph.v1",
        "intent": "receipt-validation",
        "route": "owner -> item -> test",
        "state": "selector-ready",
        "evidence": [
            {
                "id": "E1",
                "kind": "item",
                "role": "primary",
                "owner": "src/lib.rs",
                "selector": "rust://src/lib.rs#item/function/run",
                "relation": "selected",
                "state": "selector-ready",
            },
            {
                "id": "E2",
                "kind": "test",
                "role": "guard",
                "owner": "tests/run.rs",
                "selector": "rust://tests/run.rs#item/function/run_is_guarded",
                "relation": "covers",
            },
        ],
        "edges": [{"from": "E1", "relation": "covered-by", "to": "E2"}],
        "next": {
            "ref": "E1",
            "command": "asp rust query --selector rust://src/lib.rs#item/function/run --workspace . --code",
        },
        "alt": [
            {
                "ref": "E2",
                "command": "asp rust query --selector rust://tests/run.rs#item/function/run_is_guarded --workspace . --code",
            }
        ],
        "avoid": ["raw-read", "flat-selector-list"],
        "omit": [
            "source",
            "line-range",
            "confidence",
            "long-explanation",
            "not-found-inventory",
        ],
    }


def validate_receipt(receipt: dict) -> None:
    schema = schema_document()
    jsonschema.Draft202012Validator.check_schema(schema)
    jsonschema.Draft202012Validator(schema).validate(receipt)


def assert_invalid(receipt: dict) -> None:
    schema = schema_document()
    validator = jsonschema.Draft202012Validator(schema)
    assert list(validator.iter_errors(receipt))


def test_accepts_compact_graph_route_receipt_fixture() -> None:
    validate_receipt(valid_receipt())


def test_rejects_evidence_without_owner() -> None:
    receipt = valid_receipt()
    del receipt["evidence"][0]["owner"]

    assert_invalid(receipt)


def test_rejects_line_range_selector_identity() -> None:
    receipt = valid_receipt()
    receipt["evidence"][0]["selector"] = "src/lib.rs:1-2"

    assert_invalid(receipt)


def test_rejects_unbounded_evidence_dump() -> None:
    receipt = valid_receipt()
    extra = deepcopy(receipt["evidence"][0])
    receipt["evidence"].extend([{**extra, "id": "E3"}, {**extra, "id": "E4"}])

    assert_invalid(receipt)
