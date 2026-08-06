"""Combine explicit schema lifecycle policy with observed repository evidence."""

from __future__ import annotations

import json
from pathlib import Path
from typing import Any

from jsonschema import Draft202012Validator


def load_lifecycle_manifest(
    manifest_path: Path,
    contract_path: Path | None = None,
) -> tuple[dict[str, dict[str, Any]], list[dict[str, Any]]]:
    diagnostics: list[dict[str, Any]] = []
    if not manifest_path.is_file():
        return {}, [
            {
                "severity": "error",
                "code": "lifecycle-manifest-missing",
                "message": f"lifecycle manifest does not exist: {manifest_path}",
            }
        ]
    try:
        value = json.loads(manifest_path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as error:
        return {}, [
            {
                "severity": "error",
                "code": "invalid-lifecycle-manifest",
                "message": str(error),
            }
        ]
    if contract_path is not None:
        diagnostics.extend(_manifest_contract_diagnostics(value, contract_path))
    if not isinstance(value, dict) or value.get("schemaId") != "asp.schema-lifecycle-manifest.v1" or value.get("schemaVersion") != "1":
        return {}, [
            {
                "severity": "error",
                "code": "invalid-lifecycle-manifest-identity",
                "message": "lifecycle manifest must use asp.schema-lifecycle-manifest.v1 version 1",
            }
        ]
    entries: dict[str, dict[str, Any]] = {}
    for entry in value.get("entries", []):
        if not isinstance(entry, dict) or not isinstance(entry.get("schemaPath"), str):
            diagnostics.append(
                {
                    "severity": "error",
                    "code": "invalid-lifecycle-entry",
                    "message": "lifecycle entry requires schemaPath",
                }
            )
            continue
        path = entry["schemaPath"]
        if path in entries:
            diagnostics.append(
                {
                    "severity": "error",
                    "code": "duplicate-lifecycle-entry",
                    "message": f"duplicate lifecycle entry: {path}",
                    "schemaPath": path,
                }
            )
        entries[path] = entry
    return entries, diagnostics


def _manifest_contract_diagnostics(
    value: object, contract_path: Path
) -> list[dict[str, Any]]:
    try:
        contract = json.loads(contract_path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as error:
        return [
            {
                "severity": "error",
                "code": "lifecycle-contract-unavailable",
                "message": str(error),
            }
        ]
    validator = Draft202012Validator(contract)
    return [
        {
            "severity": "error",
            "code": "invalid-lifecycle-manifest-contract",
            "message": error.message,
            "jsonPointer": "/" + "/".join(str(part) for part in error.path),
        }
        for error in sorted(validator.iter_errors(value), key=lambda item: list(item.path))
    ]


def lifecycle_state(
    schema_path: str,
    rust_state: str,
    inbound_count: int,
    manifest_entries: dict[str, dict[str, Any]],
) -> dict[str, Any]:
    entry = manifest_entries.get(schema_path)
    if entry is not None:
        return {
            "lifecycleStatus": entry.get("status", "active"),
            "lifecycleSource": "manifest",
            "owner": entry.get("owner"),
            "rationale": entry.get("rationale"),
            "replacementRef": entry.get("replacementRef"),
            "sunsetAfter": entry.get("sunsetAfter"),
        }
    if rust_state in {"direct", "transitive"}:
        status = "active"
    elif inbound_count:
        status = "review"
    else:
        status = "removal-candidate"
    return {
        "lifecycleStatus": status,
        "lifecycleSource": "inferred",
        "owner": None,
        "rationale": None,
        "replacementRef": None,
        "sunsetAfter": None,
    }
