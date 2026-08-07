"""Reference candidate discovery tests."""

from pathlib import Path
import json

from asp_schema_manager.audit import audit_workspace


SHAPE = {
    "type": "object",
    "additionalProperties": False,
    "required": ["schemaId", "schemaVersion", "value"],
    "properties": {
        "schemaId": {"type": "string", "minLength": 1},
        "schemaVersion": {"const": "1"},
        "value": {"type": "string", "minLength": 1},
    },
}


def _write(path: Path, value: object) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(value), encoding="utf-8")


def test_repeated_validation_shape_becomes_reference_opportunity(tmp_path: Path) -> None:
    _write(
        tmp_path / "schemas/a.v1.schema.json",
        {
            "$id": "https://example/a",
            "$defs": {"shared": {**SHAPE, "description": "A"}},
            "properties": {"payload": {"$ref": "#/$defs/shared"}},
        },
    )
    _write(tmp_path / "schemas/b.v1.schema.json", {"$id": "https://example/b", "properties": {"payload": {**SHAPE, "description": "B"}}})
    _write(
        tmp_path / "packages/python/asp_schema_manager/asp-schema-lifecycle.v1.json",
        {"schemaId": "asp.schema-lifecycle-manifest.v1", "schemaVersion": "1", "entries": []},
    )

    report = audit_workspace(tmp_path, minimum_reference_bytes=80)

    assert report["summary"]["referenceOpportunityCount"] >= 1
    candidate = report["referenceOpportunities"][0]
    assert len(candidate["occurrences"]) == 2
    assert candidate["recommendation"] == "classify-before-extraction"
    assert candidate["estimatedReducibleBytes"] > 0


def test_local_definition_can_own_repeated_shape(tmp_path: Path) -> None:
    _write(
        tmp_path / "schemas/a.v1.schema.json",
        {
            "$id": "https://example/a",
            "$defs": {"shared": SHAPE},
            "properties": {
                "anchor": {"$ref": "#/$defs/shared"},
                "payload": SHAPE,
            },
        },
    )
    _write(
        tmp_path / "packages/python/asp_schema_manager/asp-schema-lifecycle.v1.json",
        {"schemaId": "asp.schema-lifecycle-manifest.v1", "schemaVersion": "1", "entries": []},
    )

    report = audit_workspace(tmp_path, minimum_reference_bytes=80)

    candidate = report["referenceOpportunities"][0]
    assert candidate["recommendation"] == "ref:#/$defs/shared"


def test_same_local_ref_different_targets_do_not_merge(tmp_path: Path) -> None:
    _write(
        tmp_path / "schemas/a.v1.schema.json",
        {
            "$id": "https://example/a",
            "$defs": {"shared": {"type": "string", "minLength": 1}},
            "properties": {
                "payload": {
                    "type": "object",
                    "additionalProperties": False,
                    "required": ["source", "target", "relation"],
                    "properties": {
                        "source": {"$ref": "#/$defs/shared"},
                        "target": {"$ref": "#/$defs/shared"},
                        "relation": {"type": "string", "pattern": "^[a-z][a-z0-9_-]*$"},
                        "weight": {"type": "number"},
                        "fields": {"type": "object"},
                    },
                }
            },
        },
    )
    _write(
        tmp_path / "schemas/b.v1.schema.json",
        {
            "$id": "https://example/b",
            "$defs": {"shared": {"type": "integer", "minimum": 0}},
            "properties": {
                "payload": {
                    "type": "object",
                    "additionalProperties": False,
                    "required": ["source", "target", "relation"],
                    "properties": {
                        "source": {"$ref": "#/$defs/shared"},
                        "target": {"$ref": "#/$defs/shared"},
                        "relation": {"type": "string", "pattern": "^[a-z][a-z0-9_-]*$"},
                        "weight": {"type": "number"},
                        "fields": {"type": "object"},
                    },
                }
            },
        },
    )
    _write(
        tmp_path / "packages/python/asp_schema_manager/asp-schema-lifecycle.v1.json",
        {"schemaId": "asp.schema-lifecycle-manifest.v1", "schemaVersion": "1", "entries": []},
    )

    report = audit_workspace(tmp_path, minimum_reference_bytes=1)

    merged = [
        item
        for item in report["referenceOpportunities"]
        if len(item["occurrences"]) > 1
        and {occurrence["schemaPath"] for occurrence in item["occurrences"]}
        == {"schemas/a.v1.schema.json", "schemas/b.v1.schema.json"}
    ]
    assert merged == []


def test_local_ref_target_and_reference_site_do_not_merge(tmp_path: Path) -> None:
    shared = {
        "type": "object",
        "additionalProperties": False,
        "required": ["source", "target", "relation"],
        "properties": {
            "source": {"type": "string", "minLength": 1},
            "target": {"type": "string", "minLength": 1},
            "relation": {"type": "string", "pattern": "^[a-z][a-z0-9_-]*$"},
            "weight": {"type": "number"},
            "fields": {"type": "object"},
        },
    }
    _write(
        tmp_path / "schemas/a.v1.schema.json",
        {
            "$id": "https://example/a",
            "$defs": {"shared": shared},
            "properties": {"payload": {"$ref": "#/$defs/shared"}},
        },
    )
    _write(
        tmp_path / "packages/python/asp_schema_manager/asp-schema-lifecycle.v1.json",
        {"schemaId": "asp.schema-lifecycle-manifest.v1", "schemaVersion": "1", "entries": []},
    )

    report = audit_workspace(tmp_path, minimum_reference_bytes=1)

    assert not any(
        {
            "/$defs/shared",
            "/properties/payload",
        }.issubset({occurrence["jsonPointer"] for occurrence in item["occurrences"]})
        for item in report["referenceOpportunities"]
    )


def test_unreachable_definitions_are_reported_not_extracted(tmp_path: Path) -> None:
    _write(
        tmp_path / "schemas/a.v1.schema.json",
        {"$id": "https://example/a", "$defs": {"dead": SHAPE}},
    )
    _write(
        tmp_path / "schemas/b.v1.schema.json",
        {"$id": "https://example/b", "$defs": {"dead": SHAPE}},
    )
    _write(
        tmp_path / "packages/python/asp_schema_manager/asp-schema-lifecycle.v1.json",
        {"schemaId": "asp.schema-lifecycle-manifest.v1", "schemaVersion": "1", "entries": []},
    )

    report = audit_workspace(tmp_path, minimum_reference_bytes=80)

    assert report["summary"]["referenceOpportunityCount"] == 0
    assert report["summary"]["unusedDefinitionCount"] == 2
