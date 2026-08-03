import json
from pathlib import Path

import jsonschema
import pytest


ROOT = Path(__file__).resolve().parents[2]
SCHEMA = json.loads((ROOT / "schemas/agent-route-registry.v1.schema.json").read_text())
FIXTURES = ROOT / "schemas/fixtures/agent-route-registry"


def test_route_only_registry_is_valid() -> None:
    document = json.loads((FIXTURES / "valid-route-only.v1.json").read_text())
    jsonschema.Draft202012Validator(SCHEMA).validate(document)


def test_manager_projection_fields_are_not_representable() -> None:
    document = json.loads((FIXTURES / "invalid-manager-projection.v1.json").read_text())
    with pytest.raises(jsonschema.ValidationError):
        jsonschema.Draft202012Validator(SCHEMA).validate(document)
