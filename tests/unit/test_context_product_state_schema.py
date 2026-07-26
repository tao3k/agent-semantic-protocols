from __future__ import annotations

import json
from pathlib import Path

import pytest
from jsonschema import ValidationError

from unit.schema_validation import schema_validator_for


ROOT = Path(__file__).resolve().parents[2]
SCHEMA = ROOT / "schemas" / "context-product-state.v1.schema.json"
FIXTURES = ROOT / "schemas" / "context-product-state.v1.fixtures.json"


def load_json(path: Path) -> dict[str, object]:
    return json.loads(path.read_text(encoding="utf-8"))


def fixture(name: str) -> dict[str, object]:
    fixtures = load_json(FIXTURES)["fixtures"]
    return next(item["value"] for item in fixtures if item["name"] == name)


def test_open_context_product_uses_group_aware_v1_state() -> None:
    value = fixture("valid-open-search-state")
    schema_validator_for(SCHEMA).validate(value)
    assert value["executions"] == []
    assert value["joinedExecutionGroups"] == []
    assert "execution" not in value


@pytest.mark.parametrize(
    "fixture_name",
    [
        "valid-execution-started-event-v1",
        "valid-execution-consumed-event-v1",
        "valid-execution-revoked-event-v1",
        "valid-execution-group-joined-event-v1",
    ],
)
def test_execution_events_bind_group_identity(fixture_name: str) -> None:
    validator = schema_validator_for(SCHEMA)
    event_validator = validator.evolve(
        schema=validator.schema["$defs"]["contextProductEvent"]
    )
    value = fixture(fixture_name)
    event_validator.validate(value)
    assert value["executionGroupId"] == "group-1"
    if value["eventType"] != "ExecutionGroupJoined":
        assert value["stageIds"] == ["stage-1"]


@pytest.mark.parametrize("required_field", ["executions", "joinedExecutionGroups"])
def test_context_product_rejects_missing_group_authority_field(
    required_field: str,
) -> None:
    value = fixture("valid-open-search-state")
    del value[required_field]
    with pytest.raises(ValidationError):
        schema_validator_for(SCHEMA).validate(value)
