import json
from pathlib import Path

from jsonschema import Draft202012Validator


ROOT = Path(__file__).resolve().parents[2]
SCHEMA_PATH = ROOT / "schemas" / "semantic-behavior-snapshot.v1.schema.json"


def load_schema() -> dict:
    return json.loads(SCHEMA_PATH.read_text())


def test_semantic_behavior_snapshot_schema_is_valid() -> None:
    Draft202012Validator.check_schema(load_schema())


def test_semantic_behavior_snapshot_accepts_expect_test_snapshot() -> None:
    schema = load_schema()
    value = {
        "schemaId": "agent.semantic-protocols.semantic-behavior-snapshot",
        "schemaVersion": "1",
        "protocolId": "agent.semantic-protocols.behavior-snapshot",
        "protocolVersion": "1",
        "snapshotId": "rust.expect-test.public-api-shape",
        "producer": {
            "languageId": "rust",
            "providerId": "rs-harness",
            "namespace": "agent.semantic-protocols.languages.rust.rs-harness",
        },
        "subject": {
            "kind": "public-api",
            "path": "src/lib.rs",
            "symbol": "parse_public_api_shape",
        },
        "status": "matched",
        "observations": [
            {
                "kind": "snapshot",
                "message": "expect-test snapshot matched",
                "path": "tests/public_api_shape.rs",
                "line": 12,
            }
        ],
        "expected": {
            "format": "text",
            "value": "pub fn parse_public_api_shape(...)",
        },
        "actual": {
            "format": "text",
            "value": "pub fn parse_public_api_shape(...)",
        },
        "receiptIds": ["rust.expect-test:expect-test:passed"],
        "candidateIds": ["agent-r027:src/lib.rs:1"],
    }
    Draft202012Validator(schema).validate(value)


def test_semantic_behavior_snapshot_rejects_absolute_paths() -> None:
    schema = load_schema()
    value = {
        "schemaId": "agent.semantic-protocols.semantic-behavior-snapshot",
        "schemaVersion": "1",
        "protocolId": "agent.semantic-protocols.behavior-snapshot",
        "protocolVersion": "1",
        "snapshotId": "bad",
        "producer": {
            "languageId": "rust",
            "providerId": "rs-harness",
            "namespace": "agent.semantic-protocols.languages.rust.rs-harness",
        },
        "subject": {
            "kind": "function",
            "path": "/tmp/src/lib.rs",
        },
        "status": "matched",
        "observations": [{"kind": "note", "message": "bad path"}],
    }
    errors = list(Draft202012Validator(schema).iter_errors(value))
    assert errors
