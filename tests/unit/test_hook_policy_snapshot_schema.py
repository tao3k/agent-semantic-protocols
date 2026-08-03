import json
from pathlib import Path

from jsonschema import Draft202012Validator


ROOT = Path(__file__).resolve().parents[2]
SCHEMA = ROOT / "schemas" / "hook-policy-snapshot.v1.schema.json"
FIXTURES = ROOT / "schemas" / "fixtures" / "hook-policy-snapshot"


def load_json(path: Path) -> object:
    return json.loads(path.read_text(encoding="utf-8"))


def validator() -> Draft202012Validator:
    schema = load_json(SCHEMA)
    Draft202012Validator.check_schema(schema)
    return Draft202012Validator(schema)


def test_valid_hook_policy_snapshot_v1() -> None:
    errors = list(validator().iter_errors(load_json(FIXTURES / "valid.v1.json")))
    assert errors == []


def test_hook_policy_snapshot_rejects_synchronous_daemon_dependency() -> None:
    errors = list(
        validator().iter_errors(
            load_json(FIXTURES / "invalid-synchronous-daemon-dependency.v1.json")
        )
    )
    assert errors
    assert any(list(error.path) == ["synchronousDependencies"] for error in errors)
