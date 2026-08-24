import json
from pathlib import Path

from jsonschema import Draft202012Validator


ROOT = Path(__file__).resolve().parents[2]


def test_runtime_server_readiness_schema_and_state_union() -> None:
    schema = json.loads((ROOT / "schemas/runtime-server-readiness-receipt.schema.json").read_text())
    Draft202012Validator.check_schema(schema)
    validator = Draft202012Validator(schema)
    base = {
        "schemaId": "agent.semantic-protocols.runtime-server-readiness",
        "schemaVersion": "1", "requestId": "request-1", "readinessToken": "token-1",
        "processId": 42, "ownerEpoch": 7, "endpointBindingToken": "binding-1",
        "runtimeBinaryIdentity": "a" * 64, "artifactCatalogDigest": "b" * 64,
        "transportContractDigest": "c" * 64,
    }
    for state in ("starting", "ready", "failed", "cancelled"):
        receipt = {**base, "state": state}
        if state != "starting":
            receipt["reasonKind"] = "test"
        validator.validate(receipt)
    assert list(validator.iter_errors({**base, "state": "failed"}))
