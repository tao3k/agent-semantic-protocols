# SPDX-FileCopyrightText: 2026 tao3k team and Contributors
#
# SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

from __future__ import annotations

import copy
import json
from pathlib import Path

import pytest
from jsonschema import Draft202012Validator
from referencing import Registry, Resource


ROOT = Path(__file__).resolve().parents[2]
SCHEMAS = ROOT / "schemas"
FIXTURES = SCHEMAS / "fixtures" / "multi-agent-session-control-plane-pane"


def _load(path: Path) -> dict:
    return json.loads(path.read_text(encoding="utf-8"))


def _registry() -> Registry:
    registry = Registry()
    for path in SCHEMAS.glob("*.schema.json"):
        document = _load(path)
        if schema_id := document.get("$id"):
            registry = registry.with_resource(
                schema_id, Resource.from_contents(document)
            )
    return registry


def _validator(name: str) -> Draft202012Validator:
    return Draft202012Validator(_load(SCHEMAS / name), registry=_registry())


def _assert_valid(validator: Draft202012Validator, instance: dict) -> None:
    errors = sorted(validator.iter_errors(instance), key=lambda error: list(error.path))
    assert not errors, "\n".join(error.message for error in errors)


def test_control_plane_registered_and_registration_required_fixtures_are_bound() -> None:
    validator = _validator("multi-agent-session-control-plane-pane.v1.schema.json")
    registered = _load(FIXTURES / "valid-registered.v1.json")
    registration_required = _load(FIXTURES / "valid-registration-required.v1.json")

    _assert_valid(validator, registered)
    _assert_valid(validator, registration_required)
    assert registered["agent"]["binding"]["hostChildId"]
    assert registered["agent"]["binding"]["lifecycleState"] == "live"
    assert registration_required["agent"]["binding"] is None


def test_registered_node_rejects_missing_or_terminal_binding() -> None:
    validator = _validator("multi-agent-session-control-plane-pane.v1.schema.json")
    registered = _load(FIXTURES / "valid-registered.v1.json")

    missing = copy.deepcopy(registered)
    missing["agent"]["binding"] = None
    assert list(validator.iter_errors(missing))

    terminal = copy.deepcopy(registered)
    terminal["agent"]["binding"]["lifecycleState"] = "completed"
    terminal["agent"]["binding"]["routable"] = False
    assert list(validator.iter_errors(terminal))


def test_temporary_binding_requires_explicit_authority() -> None:
    validator = _validator("agent-session-host-binding.v1.schema.json")
    binding = _load(FIXTURES / "valid-registered.v1.json")["agent"]["binding"]
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
    "multi-agent-session-control-plane-pane.v1.schema.json",
])
def test_lifecycle_schema_versions_are_exactly_one(schema_name: str) -> None:
    schema = _load(SCHEMAS / schema_name)
    assert schema["properties"]["schemaVersion"]["const"] == "1"
