import json
from pathlib import Path

import pytest
from jsonschema import Draft202012Validator, ValidationError


SCHEMA = json.loads(
    (Path(__file__).parents[2] / "schemas" / "asp-cli-failure.schema.json").read_text()
)
VALIDATOR = Draft202012Validator(SCHEMA)


def host_eperm() -> dict[str, object]:
    return {
        "schemaId": "agent.semantic-protocols.cli-failure",
        "schemaVersion": 1,
        "state": "blocked",
        "reasonKind": "host-operation-not-permitted",
        "failureLayer": "asp-cli-startup-or-ipc-boundary",
        "osError": "EPERM",
        "message": "Host denied the verified Runtime connection.",
        "recovery": "Repair Host transport permission.",
        "originalError": "Operation not permitted (os error 1)",
    }


def test_cli_failure_schema_accepts_each_typed_pre_frame_terminal() -> None:
    VALIDATOR.validate(host_eperm())
    VALIDATOR.validate(
        {
            "schemaId": "agent.semantic-protocols.cli-failure",
            "schemaVersion": 1,
            "state": "blocked",
            "reasonKind": "transport-unavailable",
            "failureLayer": "runtime-transport-capability",
            "message": "Serving identity could not be proven.",
            "originalError": "Runtime serving endpoint requires an applied activation",
        }
    )
    verified_host_transport = host_eperm()
    verified_host_transport["failureLayer"] = "runtime-verified-endpoint-transport"
    VALIDATOR.validate(verified_host_transport)
    VALIDATOR.validate(
        {
            "schemaId": "agent.semantic-protocols.cli-failure",
            "schemaVersion": 1,
            "state": "blocked",
            "reasonKind": "runtime-status-observation-failed",
            "failureLayer": "runtime-status-observation",
            "message": "Status identity could not be verified.",
            "originalError": "early eof",
        }
    )


@pytest.mark.parametrize(
    "mutate",
    [
        lambda receipt: receipt.pop("osError"),
        lambda receipt: receipt.__setitem__("failureLayer", "runtime-transport-capability"),
    ],
)
def test_cli_failure_schema_rejects_crossed_or_incomplete_host_eperm(
    mutate,
) -> None:
    receipt = host_eperm()
    mutate(receipt)
    with pytest.raises(ValidationError):
        VALIDATOR.validate(receipt)
