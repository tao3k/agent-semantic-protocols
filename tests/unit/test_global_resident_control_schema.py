import json
import unittest
from pathlib import Path

from jsonschema import Draft202012Validator


SCHEMA = json.loads(
    (
        Path(__file__).parents[2]
        / "schemas"
        / "global-resident-control.v1.schema.json"
    ).read_text()
)
VALIDATOR = Draft202012Validator(SCHEMA)


class GlobalResidentControlSchemaTest(unittest.TestCase):
    def test_accepts_typed_request_and_receipt(self) -> None:
        request = {
            "schemaId": "agent.semantic-protocols.global-resident-control-request.v1",
            "schemaVersion": "1",
            "operation": "reconcile",
            "requestId": "request-1",
        }
        receipt = {
            "schemaId": "agent.semantic-protocols.global-resident-control-receipt.v1",
            "schemaVersion": "1",
            "requestId": "request-1",
            "state": "healthy",
            "runtimeArtifactDigest": "blake3-256:runtime",
            "transportContractDigest": "blake3-256:transport",
            "workspaceEntryCount": 3,
        }

        VALIDATOR.validate(request)
        VALIDATOR.validate(receipt)

    def test_rejects_pid_and_untyped_operations(self) -> None:
        request = {
            "schemaId": "agent.semantic-protocols.global-resident-control-request.v1",
            "schemaVersion": "1",
            "operation": "kill",
            "requestId": "request-2",
            "pid": 1234,
        }

        errors = list(VALIDATOR.iter_errors(request))
        self.assertTrue(errors)
