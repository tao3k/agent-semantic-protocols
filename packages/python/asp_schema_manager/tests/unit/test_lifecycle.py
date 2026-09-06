# SPDX-FileCopyrightText: Contributors to Agent Semantic Protocols
#
# SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-only

"""Schema lifecycle safety tests."""

from pathlib import Path
import json

from asp_schema_manager.audit import audit_workspace


def _write(path: Path, value: object | str) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(value if isinstance(value, str) else json.dumps(value), encoding="utf-8")


def test_manifest_cannot_retire_a_rust_used_schema(tmp_path: Path) -> None:
    schema_path = "schemas/live.v1.schema.json"
    _write(tmp_path / schema_path, {"$id": "https://example/live", "type": "string"})
    _write(tmp_path / "crates/demo/src/lib.rs", 'const SCHEMA: &str = "live.v1.schema.json";')
    _write(
        tmp_path / "lifecycle.json",
        {
            "schemaId": "asp.schema-lifecycle-manifest.v1",
            "schemaVersion": "1",
            "entries": [
                {
                    "schemaPath": schema_path,
                    "status": "retired",
                    "owner": "demo",
                    "rationale": "test",
                }
            ],
        },
    )

    report = audit_workspace(tmp_path, manifest_path=tmp_path / "lifecycle.json")

    assert any(item["code"] == "unsafe-lifecycle-retirement" for item in report["diagnostics"])


def test_manifest_is_validated_by_shared_contract(tmp_path: Path) -> None:
    _write(tmp_path / "schemas/live.v1.schema.json", {"type": "string"})
    _write(
        tmp_path / "schemas/asp-schema-lifecycle-manifest.v1.schema.json",
        {
            "$schema": "https://json-schema.org/draft/2020-12/schema",
            "type": "object",
            "required": ["entries"],
            "properties": {"entries": {"type": "array"}},
        },
    )
    _write(
        tmp_path / "lifecycle.json",
        {"schemaId": "asp.schema-lifecycle-manifest.v1", "schemaVersion": "1"},
    )

    report = audit_workspace(tmp_path, manifest_path=tmp_path / "lifecycle.json")

    assert any(
        item["code"] == "invalid-lifecycle-manifest-contract"
        for item in report["diagnostics"]
    )
