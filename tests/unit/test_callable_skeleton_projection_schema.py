from __future__ import annotations

import json
from pathlib import Path

from jsonschema import Draft202012Validator
from referencing import Registry, Resource


ROOT = Path(__file__).resolve().parents[2]
SCHEMA_PATHS = (
    ROOT / "schemas/canonical-item-selector.v1.schema.json",
    ROOT / "schemas/exact-structural-selector.v1.schema.json",
    ROOT / "schemas/callable-skeleton-projection.v1.schema.json",
)
VALID_FIXTURE = (
    ROOT
    / "schemas/fixtures/callable-skeleton-projection/valid-rust-dispatch.v1.json"
)
INVALID_FIXTURE = (
    ROOT
    / "schemas/fixtures/callable-skeleton-projection/"
    "invalid-queryable-without-selector.v1.json"
)


def _load(path: Path) -> dict[str, object]:
    return json.loads(path.read_text())


def _validator() -> Draft202012Validator:
    schemas = [_load(path) for path in SCHEMA_PATHS]
    for schema in schemas:
        Draft202012Validator.check_schema(schema)
    registry = Registry().with_resources(
        [
            (str(schema["$id"]), Resource.from_contents(schema))
            for schema in schemas
        ]
    )
    return Draft202012Validator(schemas[-1], registry=registry)


def test_callable_skeleton_valid_fixture() -> None:
    _validator().validate(_load(VALID_FIXTURE))


def test_queryable_node_requires_exact_selector() -> None:
    errors = list(_validator().iter_errors(_load(INVALID_FIXTURE)))
    assert any(
        error.validator == "required" and "exactSelector" in error.message
        for error in errors
    )


def test_new_contract_families_use_v1() -> None:
    owned_paths = (*SCHEMA_PATHS[1:], VALID_FIXTURE, INVALID_FIXTURE)
    owned_text = "\n".join(path.read_text() for path in owned_paths)
    assert "exact-structural-selector.v2" not in owned_text
    assert '"schemaVersion": "v2"' not in owned_text
