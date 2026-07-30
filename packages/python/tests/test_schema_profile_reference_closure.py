from __future__ import annotations

import json
from pathlib import Path

from tools.schema_reference_closure import (
    schema_profile_contract_closure,
    schema_reference_closure,
)


def _write_schema(schema_dir: Path, name: str, document: dict[str, object]) -> None:
    schema_dir.mkdir(parents=True, exist_ok=True)
    (schema_dir / name).write_text(json.dumps(document), encoding="utf-8")


def test_schema_reference_closure_follows_local_and_canonical_refs(
    tmp_path: Path,
) -> None:
    schema_dir = tmp_path / "schemas"
    _write_schema(
        schema_dir,
        "root.schema.json",
        {
            "$defs": {
                "local": {"$ref": "local.schema.json"},
                "canonical": {
                    "$ref": (
                        "https://example.invalid/schemas/"
                        "canonical.schema.json#/$defs/value"
                    )
                },
                "contract": {
                    "const": "https://example.invalid/schemas/contract.schema.json"
                },
            }
        },
    )
    _write_schema(
        schema_dir,
        "local.schema.json",
        {"$ref": "transitive.schema.json"},
    )
    _write_schema(schema_dir, "canonical.schema.json", {"type": "object"})
    _write_schema(schema_dir, "contract.schema.json", {"type": "object"})
    _write_schema(schema_dir, "transitive.schema.json", {"type": "string"})

    assert schema_reference_closure(tmp_path, {"root.schema.json"}) == {
        "root.schema.json",
        "local.schema.json",
        "canonical.schema.json",
        "contract.schema.json",
        "transitive.schema.json",
    }


def test_schema_reference_closure_ignores_non_schema_and_missing_refs(
    tmp_path: Path,
) -> None:
    schema_dir = tmp_path / "schemas"
    _write_schema(
        schema_dir,
        "root.schema.json",
        {
            "examples": [
                {"$ref": "#/$defs/local"},
                {"$ref": "missing.schema.json"},
                {"$ref": "notes.txt"},
            ]
        },
    )

    assert schema_reference_closure(tmp_path, {"root.schema.json"}) == {
        "root.schema.json"
    }


def test_schema_profile_contract_closure_seeds_provider_manifest(
    tmp_path: Path,
) -> None:
    schema_dir = tmp_path / "schemas"
    _write_schema(
        schema_dir,
        "provider-manifest.v1.schema.json",
        {"$ref": "provider-project-resolution-descriptor.v1.schema.json"},
    )
    _write_schema(
        schema_dir,
        "provider-project-resolution-descriptor.v1.schema.json",
        {"type": "object"},
    )
    assert schema_profile_contract_closure(
        tmp_path,
        (),
    ) == {
        "provider-manifest.v1.schema.json",
        "provider-project-resolution-descriptor.v1.schema.json",
    }
