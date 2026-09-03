import json
from pathlib import Path

from jsonschema import Draft202012Validator


ROOT = Path(__file__).resolve().parents[2]
SCHEMA_PATH = ROOT / "schemas" / "runtime-server-workspace-generation-admission.schema.json"


def load_schema() -> dict:
    return json.loads(SCHEMA_PATH.read_text(encoding="utf-8"))


def test_runtime_owned_incremental_admission_keeps_schema_version_one() -> None:
    schema = load_schema()

    Draft202012Validator.check_schema(schema)
    assert schema["properties"]["schemaVersion"]["const"] == "1"
    assert schema["properties"]["buildOwner"]["const"] == "runtime-server"
    assert schema["properties"]["cancellationAuthority"]["const"] == "runtime-server"
    assert schema["properties"]["requestLifetimeIndependent"]["const"] is True


def test_runtime_owned_incremental_admission_requires_origin_and_mode() -> None:
    schema = load_schema()
    required = set(schema["required"])

    assert {
        "trigger",
        "admissionMode",
        "buildOwner",
        "cancellationAuthority",
        "requestLifetimeIndependent",
    } <= required
    assert schema["properties"]["trigger"]["enum"] == [
        "query-demand",
        "workspace-change",
        "operator-mutation",
        "artifact-publication",
        "runtime-recovery",
    ]
    assert schema["properties"]["admissionMode"]["enum"] == [
        "complete-generation",
        "incremental-overlay",
        "full-recovery",
    ]
