"""Load and reconcile explicit cross-family reference decisions."""

from __future__ import annotations

import json
from hashlib import sha256
from pathlib import Path
from typing import Any

from jsonschema import Draft202012Validator


def load_reference_decisions(
    registry_path: Path,
    contract_path: Path | None = None,
) -> tuple[list[dict[str, Any]], list[dict[str, Any]]]:
    diagnostics: list[dict[str, Any]] = []
    if not registry_path.is_file():
        return [], [
            {
                "severity": "error",
                "code": "reference-decision-registry-missing",
                "message": f"reference decision registry does not exist: {registry_path}",
            }
        ]
    try:
        value = json.loads(registry_path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as error:
        return [], [
            {
                "severity": "error",
                "code": "invalid-reference-decision-registry",
                "message": str(error),
            }
        ]
    if contract_path is not None:
        diagnostics.extend(_contract_diagnostics(value, contract_path))
    if (
        not isinstance(value, dict)
        or value.get("schemaId") != "asp.schema-reference-decision-registry.v1"
        or value.get("schemaVersion") != "1"
    ):
        return [], [
            {
                "severity": "error",
                "code": "invalid-reference-decision-registry-identity",
                "message": (
                    "reference decision registry must use "
                    "asp.schema-reference-decision-registry.v1 version 1"
                ),
            }
        ]
    entries: list[dict[str, Any]] = []
    identities: set[tuple[str, str, tuple[str, ...]]] = set()
    for entry in value.get("entries", []):
        if not isinstance(entry, dict):
            diagnostics.append(
                {
                    "severity": "error",
                    "code": "invalid-reference-decision-entry",
                    "message": "reference decision entry must be an object",
                }
            )
            continue
        identity = decision_identity(entry)
        if identity in identities:
            diagnostics.append(
                {
                    "severity": "error",
                    "code": "duplicate-reference-decision-entry",
                    "message": f"duplicate reference decision entry: {entry.get('fingerprint')}",
                }
            )
        identities.add(identity)
        entries.append(entry)
    return entries, diagnostics


def decision_identity(
    value: dict[str, Any],
) -> tuple[str, str, tuple[str, ...]]:
    occurrence_set_digest = value.get("occurrenceSetDigest")
    if not isinstance(occurrence_set_digest, str):
        occurrences = sorted(
            (item.get("schemaPath", ""), item.get("jsonPointer", ""))
            for item in value.get("occurrences", [])
            if isinstance(item, dict)
        )
        encoded = json.dumps(occurrences, separators=(",", ":")).encode("utf-8")
        occurrence_set_digest = "sha256:" + sha256(encoded).hexdigest()
    family_ids = tuple(
        sorted(item for item in value.get("familyIds", []) if isinstance(item, str))
    )
    return str(value.get("fingerprint", "")), occurrence_set_digest, family_ids


def reconcile_reference_decisions(
    opportunities: list[dict[str, Any]],
    entries: list[dict[str, Any]],
) -> list[dict[str, Any]]:
    diagnostics: list[dict[str, Any]] = []
    current = {
        decision_identity(item): item
        for item in opportunities
        if item.get("familyScope") == "cross-family"
    }
    decisions = {decision_identity(item): item for item in entries}
    for identity, opportunity in current.items():
        entry = decisions.get(identity)
        if entry is None or entry.get("reviewState") != "accepted":
            diagnostics.append(
                {
                    "severity": "warning",
                    "code": "cross-family-reference-decision-missing",
                    "message": "cross-family reference opportunity lacks an accepted decision",
                "fingerprint": opportunity["fingerprint"],
                "familyIds": opportunity["familyIds"],
                }
            )
    for identity, entry in decisions.items():
        if identity not in current and entry.get("reviewState") != "superseded":
            diagnostics.append(
                {
                    "severity": "warning",
                    "code": "stale-reference-decision",
                    "message": "reference decision no longer matches a current cross-family opportunity",
                "fingerprint": entry.get("fingerprint"),
                }
            )
    return diagnostics


def _contract_diagnostics(
    value: object, contract_path: Path
) -> list[dict[str, Any]]:
    try:
        contract = json.loads(contract_path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as error:
        return [
            {
                "severity": "error",
                "code": "reference-decision-contract-unavailable",
                "message": str(error),
            }
        ]
    validator = Draft202012Validator(contract)
    return [
        {
            "severity": "error",
            "code": "invalid-reference-decision-registry-contract",
            "message": error.message,
            "jsonPointer": "/" + "/".join(str(part) for part in error.path),
        }
        for error in sorted(validator.iter_errors(value), key=lambda item: list(item.path))
    ]
