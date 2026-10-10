# SPDX-FileCopyrightText: 2026 tao3k team and Contributors
#
# SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

from __future__ import annotations

import copy
import json
from pathlib import Path

import pytest
from jsonschema import Draft202012Validator


ROOT = Path(__file__).resolve().parents[2]
SCHEMAS = ROOT / "schemas"


def _load(path: Path) -> dict:
    return json.loads(path.read_text(encoding="utf-8"))


def _validator(name: str) -> Draft202012Validator:
    from unit.schema_validation import schema_validator_for

    return schema_validator_for(SCHEMAS / name)


def _assert_valid(validator: Draft202012Validator, instance: dict) -> None:
    errors = sorted(validator.iter_errors(instance), key=lambda error: list(error.path))
    assert not errors, "\n".join(error.message for error in errors)


def resident_binding() -> dict[str, object]:
    return {
        "schemaId": "agent.semantic-protocols.agent-session-host-binding",
        "schemaVersion": "1",
        "bindingId": "binding-1",
        "registrationId": "registration-1",
        "rootSessionId": "root-1",
        "hostChildId": "child-1",
        "hostTaskName": "asp_testing",
        "agentInstanceId": "agent-1",
        "residentId": "asp_testing",
        "routeKey": "testing",
        "profileId": "profile-1",
        "profileDigest": "blake3:profile",
        "modelId": "gpt-test",
        "modelDigest": "blake3:model",
        "sandboxMode": "workspace-write",
        "sessionLifetime": "resident",
        "generation": 1,
        "lifecycleState": "live",
        "routable": True,
        "evidenceSequence": 1,
        "temporaryAuthority": None,
    }


def test_temporary_binding_requires_explicit_authority() -> None:
    validator = _validator("agent-session-host-binding.v1.schema.json")
    binding = resident_binding()
    _assert_valid(validator, binding)
    temporary = copy.deepcopy(binding)
    temporary["sessionLifetime"] = "temporary"
    temporary.pop("temporaryAuthority")

    assert list(validator.iter_errors(temporary))

    temporary["temporaryAuthority"] = {
        "reason": "explicit bounded parallel qualification",
        "parentBindingId": binding["bindingId"],
        "issuedAtUtc": "2026-08-06T00:00:00Z",
        "expiresAtUtc": "2026-08-06T00:05:00Z",
        "completionCondition": "qualification receipt published",
    }
    _assert_valid(validator, temporary)


def test_root_namespace_rejects_duplicate_configured_resident_ids() -> None:
    validator = _validator("agent-root-resident-namespace.v1.schema.json")
    namespace = {
        "schemaId": "agent.semantic-protocols.agent-root-resident-namespace",
        "schemaVersion": "1",
        "rootSessionId": "root-1",
        "configuredResidentIds": ["asp_explorer", "asp_testing"],
        "residents": [],
        "temporarySessions": [],
        "nonMatchedChildren": [],
        "generation": 0,
    }
    _assert_valid(validator, namespace)

    namespace["configuredResidentIds"] = ["asp_explorer", "asp_explorer"]
    assert list(validator.iter_errors(namespace))


@pytest.mark.parametrize("schema_name", [
    "agent-session-host-binding.v1.schema.json",
    "agent-root-resident-namespace.v1.schema.json",
])
def test_lifecycle_schema_versions_are_exactly_one(schema_name: str) -> None:
    schema = _load(SCHEMAS / schema_name)
    assert schema["properties"]["schemaVersion"]["const"] == "1"
