import json
from pathlib import Path

from jsonschema import Draft202012Validator


ROOT = Path(__file__).resolve().parents[2]
SCHEMA = json.loads(
    (ROOT / "schemas/workspace-search-playbook-result.v1.schema.json").read_text()
)
VALIDATOR = Draft202012Validator(SCHEMA)


def result(**overrides):
    value = {
        "schemaId": "agent.semantic-protocols.workspace-search-playbook-result",
        "schemaVersion": "1",
        "result": "exact-selector-ready",
        "queryGrammar": "asp query --selector <exact-selector> --projection <callable-skeleton|source>",
        "evidence": [
            {
                "owner": "src/runtime.rs",
                "item": "struct/RuntimeServingEndpoint",
                "selector": "rust://src/runtime.rs#item/struct/RuntimeServingEndpoint",
                "matchedBy": ["rg:0", "tantivy:0", "syntax:0"],
                "relation": "syntax-capture:struct",
            }
        ],
    }
    value.update(overrides)
    return value


def assert_valid(value):
    assert list(VALIDATOR.iter_errors(value)) == []


def assert_invalid(value):
    assert list(VALIDATOR.iter_errors(value))


def test_accepts_bounded_selector_only_result():
    assert_valid(result())


def test_accepts_explicit_zero_evidence_result():
    value = result(result="refinement-required", evidence=[])
    del value["queryGrammar"]
    assert_valid(value)


def test_rejects_more_than_top_thirty_evidence():
    evidence = [
        {
            "owner": f"src/item_{index}.rs",
            "item": "function/run",
            "selector": f"rust://src/item_{index}.rs#item/function/run",
            "matchedBy": ["rg:0", "syntax:0", "graph:0"],
            "relation": "syntax-capture:function",
        }
        for index in range(31)
    ]
    assert_invalid(result(evidence=evidence))


def test_rejects_source_digest_plan_and_next_fields():
    for field, value in [
        ("source", "fn leaked() {}"),
        ("digest", "blake3-256:deadbeef"),
        ("plan", {"stage": "rg"}),
        ("next", "asp query --selector ..."),
        ("recommendedNext", "asp query --selector ..."),
    ]:
        assert_invalid(result(**{field: value}))


def test_rejects_non_exact_selector_evidence():
    evidence = result()["evidence"]
    evidence[0]["selector"] = "src/runtime.rs:12"
    assert_invalid(result(evidence=evidence))


def test_requires_query_grammar_exactly_when_evidence_exists():
    value = result()
    del value["queryGrammar"]
    assert_invalid(value)

    value = result(queryGrammar="asp query --selector <exact-selector>")
    assert_invalid(value)

    value = result(result="no-match", evidence=[])
    assert_invalid(value)


def test_rejects_missing_item_or_supporting_clause_references():
    for field in ("item", "matchedBy"):
        value = result()
        del value["evidence"][0][field]
        assert_invalid(value)
