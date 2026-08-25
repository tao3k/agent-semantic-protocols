"""Load explicit schema-family policy and classify catalog documents."""

from __future__ import annotations

import json
from pathlib import Path
from typing import Any

from jsonschema import Draft202012Validator

from .catalog import SchemaDocument


def load_family_registry(
    registry_path: Path,
    contract_path: Path,
) -> tuple[list[dict[str, Any]], list[dict[str, Any]]]:
    if not registry_path.is_file():
        return [], [
            {
                "severity": "warning",
                "code": "schema-family-registry-missing",
                "message": f"schema family registry does not exist: {registry_path}",
            }
        ]
    try:
        value = json.loads(registry_path.read_text(encoding="utf-8"))
        contract = json.loads(contract_path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as error:
        return [], [
            {
                "severity": "error",
                "code": "invalid-schema-family-registry",
                "message": str(error),
            }
        ]
    families = value.get("families", []) if isinstance(value, dict) else []
    family_contract_value = {
        "schemaId": "asp.schema-family-registry.v1",
        "schemaVersion": "1",
        "families": families,
    }
    diagnostics = [
        {
            "severity": "error",
            "code": "invalid-schema-family-registry-contract",
            "message": error.message,
            "jsonPointer": "/" + "/".join(str(part) for part in error.path),
        }
        for error in sorted(
            Draft202012Validator(contract).iter_errors(family_contract_value),
            key=lambda item: list(item.path),
        )
    ]
    return [item for item in families if isinstance(item, dict)], diagnostics


def classify_schema_families(
    documents: list[SchemaDocument],
    families: list[dict[str, Any]],
) -> tuple[dict[str, dict[str, str | None]], list[dict[str, Any]]]:
    diagnostics = _registry_diagnostics(documents, families)
    assignments: dict[str, dict[str, str | None]] = {}
    for document in documents:
        matches = [family for family in families if _matches(document, family)]
        if not matches:
            assignments[document.relative_path] = {
                "familyId": None,
                "familySource": "unclassified",
            }
            continue
        highest_priority = max(int(family.get("priority", 0)) for family in matches)
        winners = [
            family
            for family in matches
            if int(family.get("priority", 0)) == highest_priority
        ]
        if len(winners) > 1:
            family_ids = sorted(str(family.get("familyId")) for family in winners)
            diagnostics.append(
                {
                    "severity": "error",
                    "code": "ambiguous-schema-family",
                    "message": f"same-priority family match: {', '.join(family_ids)}",
                    "schemaPath": document.relative_path,
                }
            )
            assignments[document.relative_path] = {
                "familyId": None,
                "familySource": "ambiguous",
            }
            continue
        winner = winners[0]
        overrides = winner.get("membershipOverrides", {})
        includes = (
            overrides.get("includeSchemaPaths", [])
            if isinstance(overrides, dict)
            else []
        )
        assignments[document.relative_path] = {
            "familyId": str(winner.get("familyId")),
            "familySource": (
                "explicit-include"
                if document.relative_path in includes
                else "namespace-selector"
            ),
        }
    for family in families:
        definition_path = family.get("definitionSchemaPath")
        if definition_path is None or definition_path not in assignments:
            continue
        family_id = str(family.get("familyId"))
        if assignments[definition_path]["familyId"] != family_id:
            diagnostics.append(
                {
                    "severity": "error",
                    "code": "schema-family-definition-owner-mismatch",
                    "message": f"definition schema is not classified into {family_id}",
                    "schemaPath": str(definition_path),
                }
            )
    return assignments, diagnostics


def _matches(document: SchemaDocument, family: dict[str, Any]) -> bool:
    namespace = family.get("namespace")
    if not isinstance(namespace, dict):
        return False
    overrides = family.get("membershipOverrides", {})
    excludes = (
        overrides.get("excludeSchemaPaths", [])
        if isinstance(overrides, dict)
        else []
    )
    if document.relative_path in excludes:
        return False
    includes = (
        overrides.get("includeSchemaPaths", [])
        if isinstance(overrides, dict)
        else []
    )
    if document.relative_path in includes:
        return True
    filename_match = any(
        document.path.name.startswith(prefix)
        for prefix in namespace.get("filenamePrefixes", [])
        if isinstance(prefix, str)
    )
    identifier_match = any(
        document.schema_identifier is not None
        and document.schema_identifier.startswith(prefix)
        for prefix in namespace.get("schemaIdentifierPrefixes", [])
        if isinstance(prefix, str)
    )
    return filename_match or identifier_match


def _registry_diagnostics(
    documents: list[SchemaDocument], families: list[dict[str, Any]]
) -> list[dict[str, Any]]:
    diagnostics: list[dict[str, Any]] = []
    family_ids = [str(family.get("familyId")) for family in families]
    known_paths = {document.relative_path for document in documents}
    documents_by_path = {document.relative_path: document for document in documents}
    for family_id in sorted(set(family_ids)):
        if family_ids.count(family_id) > 1:
            diagnostics.append(
                {
                    "severity": "error",
                    "code": "duplicate-schema-family",
                    "message": f"duplicate familyId: {family_id}",
                }
            )
    for family in families:
        family_id = str(family.get("familyId"))
        parent = family.get("parentFamilyId")
        if parent is not None and parent not in family_ids:
            diagnostics.append(
                {
                    "severity": "error",
                    "code": "unknown-parent-schema-family",
                    "message": f"unknown parentFamilyId {parent}",
                }
            )
        definition_path = family.get("definitionSchemaPath")
        if definition_path is not None and definition_path not in known_paths:
            diagnostics.append(
                {
                    "severity": "error",
                    "code": "schema-family-definition-missing",
                    "message": f"family definition schema does not exist: {definition_path}",
                    "schemaPath": str(definition_path),
                }
            )
        elif definition_path is not None:
            definitions = documents_by_path[str(definition_path)].value.get("$defs")
            if not isinstance(definitions, dict) or not definitions:
                diagnostics.append(
                    {
                        "severity": "error",
                        "code": "schema-family-definition-surface-empty",
                        "message": "family definition schema must own non-empty $defs",
                        "schemaPath": str(definition_path),
                    }
                )
        overrides = family.get("membershipOverrides", {})
        if not isinstance(overrides, dict):
            continue
        named_paths = set(overrides.get("includeSchemaPaths", [])) | set(
            overrides.get("excludeSchemaPaths", [])
        )
        for schema_path in sorted(named_paths - known_paths):
            diagnostics.append(
                {
                    "severity": "error",
                    "code": "schema-family-selector-path-missing",
                    "message": f"family selector names a missing schema for {family_id}",
                    "schemaPath": schema_path,
                }
            )
    diagnostics.extend(_parent_cycle_diagnostics(families))
    return diagnostics


def _parent_cycle_diagnostics(
    families: list[dict[str, Any]],
) -> list[dict[str, Any]]:
    parents = {
        str(family.get("familyId")): str(family["parentFamilyId"])
        for family in families
        if family.get("parentFamilyId") is not None
    }
    diagnostics: list[dict[str, Any]] = []
    reported: set[tuple[str, ...]] = set()
    for family_id in sorted(parents):
        chain: list[str] = []
        current = family_id
        while current in parents and current not in chain:
            chain.append(current)
            current = parents[current]
        if current not in chain:
            continue
        cycle = tuple(sorted(chain[chain.index(current) :]))
        if cycle in reported:
            continue
        reported.add(cycle)
        diagnostics.append(
            {
                "severity": "error",
                "code": "schema-family-parent-cycle",
                "message": f"schema family parent cycle: {', '.join(cycle)}",
            }
        )
    return diagnostics


def definition_visibility_diagnostics(
    edges: dict[str, set[str]],
    assignments: dict[str, dict[str, str | None]],
    families: list[dict[str, Any]],
) -> list[dict[str, Any]]:
    by_definition_path = {
        str(family["definitionSchemaPath"]): family
        for family in families
        if family.get("definitionSchemaPath") is not None
    }
    parents = {
        str(family.get("familyId")): str(family["parentFamilyId"])
        for family in families
        if family.get("parentFamilyId") is not None
    }
    diagnostics: list[dict[str, Any]] = []
    for source_path, targets in edges.items():
        source_family = assignments.get(source_path, {}).get("familyId")
        for target_path in sorted(targets & by_definition_path.keys()):
            if source_path == target_path:
                continue
            owner = by_definition_path[target_path]
            owner_family = str(owner.get("familyId"))
            visibility = owner.get("definitionVisibility")
            if _definition_visible(
                source_family, owner_family, str(visibility), parents
            ):
                continue
            diagnostics.append(
                {
                    "severity": "error",
                    "code": "schema-family-definition-visibility-violation",
                    "message": (
                        f"{owner_family} definitions have {visibility} visibility; "
                        f"consumer family is {source_family or 'unclassified'}"
                    ),
                    "schemaPath": source_path,
                }
            )
    return diagnostics


def _definition_visible(
    source_family: str | None,
    owner_family: str,
    visibility: str,
    parents: dict[str, str],
) -> bool:
    if visibility == "public":
        return True
    if source_family == owner_family:
        return True
    if visibility != "descendants" or source_family is None:
        return False
    current = source_family
    visited: set[str] = set()
    while current in parents and current not in visited:
        visited.add(current)
        current = parents[current]
        if current == owner_family:
            return True
    return False
