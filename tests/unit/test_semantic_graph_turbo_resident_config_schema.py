import json
from pathlib import Path

from jsonschema import Draft202012Validator


ROOT = Path(__file__).resolve().parents[2]
SCHEMA = ROOT / "schemas/semantic-graph-turbo-resident-config.v1.schema.json"
FIXTURES = ROOT / "schemas/fixtures/semantic-graph-turbo-resident"


def _validator() -> Draft202012Validator:
    schema = json.loads(SCHEMA.read_text(encoding="utf-8"))
    Draft202012Validator.check_schema(schema)
    return Draft202012Validator(schema)


def test_resident_config_fixture_is_valid() -> None:
    document = json.loads(
        (FIXTURES / "valid-resident-config.v1.json").read_text(encoding="utf-8")
    )
    assert list(_validator().iter_errors(document)) == []


def test_resident_config_rejects_unknown_schema_version() -> None:
    document = json.loads(
        (
            FIXTURES / "invalid-resident-config-unknown-version.v1.json"
        ).read_text(encoding="utf-8")
    )
    assert list(_validator().iter_errors(document))
