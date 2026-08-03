import json
from pathlib import Path

import jsonschema


ROOT = Path(__file__).resolve().parents[2]
SCHEMA = json.loads(
    (ROOT / "schemas/agent-session-lifecycle-projection.v1.schema.json").read_text()
)
FIXTURES = ROOT / "schemas/fixtures/agent-session-lifecycle-projection"


def load_fixture(name: str) -> object:
    return json.loads((FIXTURES / name).read_text())


def test_ready_projection_matches_schema() -> None:
    jsonschema.validate(load_fixture("valid-ready.v1.json"), SCHEMA)


def test_unobserved_projection_matches_schema() -> None:
    jsonschema.validate(load_fixture("valid-unobserved.v1.json"), SCHEMA)


def test_path_release_requires_both_indexed_receipts() -> None:
    with __import__("pytest").raises(jsonschema.ValidationError):
        jsonschema.validate(
            load_fixture("invalid-released-without-receipts.v1.json"), SCHEMA
        )


def test_observed_session_requires_generation() -> None:
    with __import__("pytest").raises(jsonschema.ValidationError):
        jsonschema.validate(
            load_fixture("invalid-observed-session-without-generation.v1.json"), SCHEMA
        )
