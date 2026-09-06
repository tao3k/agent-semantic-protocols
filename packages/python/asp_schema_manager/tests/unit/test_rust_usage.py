# SPDX-FileCopyrightText: Contributors to Agent Semantic Protocols
#
# SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-only

"""Rust schema usage reachability tests."""

from pathlib import Path
import json

from asp_schema_manager.audit import audit_workspace


def _write(path: Path, value: object | str) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(value if isinstance(value, str) else json.dumps(value), encoding="utf-8")


def test_rust_direct_and_transitive_usage_are_distinct(tmp_path: Path) -> None:
    _write(
        tmp_path / "schemas/root.v1.schema.json",
        {"$id": "https://example/root.v1.schema.json", "$ref": "https://example/leaf.v1.schema.json"},
    )
    _write(tmp_path / "schemas/leaf.v1.schema.json", {"$id": "https://example/leaf.v1.schema.json", "type": "string"})
    _write(tmp_path / "schemas/orphan.v1.schema.json", {"$id": "https://example/orphan.v1.schema.json", "type": "string"})
    _write(tmp_path / "crates/demo/src/lib.rs", 'const SCHEMA: &str = include_str!("root.v1.schema.json");')
    _write(
        tmp_path / "packages/python/asp_schema_manager/asp-schema-lifecycle.v1.json",
        {"schemaId": "asp.schema-lifecycle-manifest.v1", "schemaVersion": "1", "entries": []},
    )

    report = audit_workspace(tmp_path)
    states = {item["schemaPath"]: item for item in report["schemas"]}

    assert states["schemas/root.v1.schema.json"]["rustUsage"] == "direct"
    assert states["schemas/leaf.v1.schema.json"]["rustUsage"] == "transitive"
    assert states["schemas/orphan.v1.schema.json"]["rustUsage"] == "unobserved"
    assert states["schemas/orphan.v1.schema.json"]["lifecycleStatus"] == "removal-candidate"


def test_missing_reference_fragment_fails_the_management_gate(tmp_path: Path) -> None:
    _write(
        tmp_path / "schemas/root.v1.schema.json",
        {"$id": "https://example/root", "$ref": "https://example/leaf#/$defs/missing"},
    )
    _write(
        tmp_path / "schemas/leaf.v1.schema.json",
        {"$id": "https://example/leaf", "$defs": {"present": {"type": "string"}}},
    )
    _write(
        tmp_path / "schemas/asp-schema-lifecycle-manifest.v1.schema.json",
        {"$schema": "https://json-schema.org/draft/2020-12/schema", "type": "object"},
    )
    _write(
        tmp_path / "packages/python/asp_schema_manager/asp-schema-lifecycle.v1.json",
        {"schemaId": "asp.schema-lifecycle-manifest.v1", "schemaVersion": "1", "entries": []},
    )

    report = audit_workspace(tmp_path)

    assert any(
        item["code"] == "unresolved-schema-reference-fragment"
        for item in report["diagnostics"]
    )
