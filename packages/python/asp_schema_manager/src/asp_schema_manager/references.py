# SPDX-FileCopyrightText: Contributors to Agent Semantic Protocols
#
# SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-only

"""Discover repeated JSON Schema validation shapes suitable for shared references."""

from __future__ import annotations

from collections import defaultdict
import hashlib
import json
from typing import Any, Iterator

from .catalog import SchemaDocument


ANNOTATION_KEYS = {"title", "description", "$comment", "examples", "default", "deprecated", "readOnly", "writeOnly"}
VALIDATION_KEYS = {
    "type",
    "properties",
    "items",
    "prefixItems",
    "required",
    "additionalProperties",
    "unevaluatedProperties",
    "enum",
    "const",
    "allOf",
    "anyOf",
    "oneOf",
    "not",
    "if",
    "then",
    "else",
    "pattern",
    "format",
    "minimum",
    "maximum",
    "minLength",
    "maxLength",
    "minItems",
    "maxItems",
    "uniqueItems",
}


def _escape_pointer(token: str) -> str:
    return token.replace("~", "~0").replace("/", "~1")


def _schema_nodes(value: Any, pointer: str = "") -> Iterator[tuple[str, dict[str, Any]]]:
    if isinstance(value, dict):
        if pointer and "$ref" not in value and VALIDATION_KEYS.intersection(value):
            yield pointer, value
        for key, child in value.items():
            child_pointer = f"{pointer}/{_escape_pointer(str(key))}"
            yield from _schema_nodes(child, child_pointer)
    elif isinstance(value, list):
        for index, child in enumerate(value):
            yield from _schema_nodes(child, f"{pointer}/{index}")


def _validation_shape(value: Any) -> Any:
    if isinstance(value, dict):
        return {
            key: _validation_shape(child)
            for key, child in sorted(value.items())
            if key not in ANNOTATION_KEYS
        }
    if isinstance(value, list):
        return [_validation_shape(child) for child in value]
    return value


def _unescape_pointer(token: str) -> str:
    return token.replace("~1", "/").replace("~0", "~")


def _resolve_local_refs(
    value: Any,
    document: SchemaDocument,
    stack: tuple[str, ...] = (),
) -> Any:
    if (
        isinstance(value, dict)
        and isinstance(value.get("$ref"), str)
        and value["$ref"].startswith("#/")
    ):
        reference = value["$ref"]
        siblings = {
            key: _resolve_local_refs(child, document, stack)
            for key, child in value.items()
            if key != "$ref"
        }
        if reference in stack:
            resolved: Any = {"$refResolution": f"cycle:{reference}"}
        else:
            target: Any = document.value
            try:
                for raw_token in reference[2:].split("/"):
                    token = _unescape_pointer(raw_token)
                    target = target[int(token)] if isinstance(target, list) else target[token]
            except (KeyError, IndexError, TypeError, ValueError):
                resolved = {"$refResolution": f"unresolved:{reference}"}
            else:
                resolved = _resolve_local_refs(target, document, (*stack, reference))
        if siblings:
            return {"$refResolution": resolved, "$refSiblings": siblings}
        return resolved
    if isinstance(value, dict):
        return {
            key: _resolve_local_refs(child, document, stack)
            for key, child in value.items()
        }
    if isinstance(value, list):
        return [_resolve_local_refs(child, document, stack) for child in value]
    return value


def discover_reference_opportunities(
    documents: list[SchemaDocument],
    *,
    family_assignments: dict[str, dict[str, str | None]] | None = None,
    used_definitions: set[tuple[str, str]] | None = None,
    minimum_bytes: int = 120,
    limit: int = 200,
) -> list[dict[str, Any]]:
    groups: dict[str, list[tuple[SchemaDocument, str, int]]] = defaultdict(list)
    shapes: dict[str, str] = {}
    for document in documents:
        for pointer, node in _schema_nodes(document.value):
            definition_root = _definition_root(pointer)
            if (
                used_definitions is not None
                and definition_root is not None
                and (document.relative_path, definition_root) not in used_definitions
            ):
                continue
            normalized = json.dumps(
                _validation_shape(_resolve_local_refs(node, document)),
                sort_keys=True,
                separators=(",", ":"),
                ensure_ascii=False,
            )
            validation_bytes = len(normalized.encode("utf-8"))
            if validation_bytes < minimum_bytes:
                continue
            fingerprint = hashlib.sha256(normalized.encode("utf-8")).hexdigest()
            shapes[fingerprint] = normalized
            groups[fingerprint].append((document, pointer, validation_bytes))

    opportunities: list[dict[str, Any]] = []
    for fingerprint, occurrences in groups.items():
        unique = {(document.relative_path, pointer) for document, pointer, _size in occurrences}
        if len(unique) < 2:
            continue
        ordered = sorted(occurrences, key=lambda entry: (entry[0].relative_path, entry[1]))
        validation_bytes = len(shapes[fingerprint].encode("utf-8"))
        estimated = max(0, validation_bytes * (len(ordered) - 1) - 72 * len(ordered))
        if estimated == 0:
            continue
        schema_paths = {document.relative_path for document, _pointer, _size in ordered}
        family_ids, family_scope = _family_scope(schema_paths, family_assignments)
        local_anchors = [
            (document, pointer)
            for document, pointer, _size in ordered
            if pointer.startswith("/$defs/") and document.schema_identifier
        ]
        if len(schema_paths) == 1 and local_anchors:
            _anchor_document, anchor_pointer = local_anchors[0]
            recommendation = f"ref:#{anchor_pointer}"
        elif family_scope == "family-local":
            recommendation = f"extract-family-definition:{family_ids[0]}"
        elif family_scope == "cross-family":
            recommendation = "review-cross-family-definition"
        else:
            recommendation = "classify-before-extraction"
        opportunities.append(
            {
                "fingerprint": f"sha256:{fingerprint}",
                "occurrences": [
                    {"schemaPath": document.relative_path, "jsonPointer": pointer}
                    for document, pointer, _size in ordered
                ],
                "validationBytes": validation_bytes,
                "estimatedReducibleBytes": estimated,
                "recommendation": recommendation,
                "familyIds": family_ids,
                "familyScope": family_scope,
            }
        )
    opportunities.sort(
        key=lambda item: (
            -item["estimatedReducibleBytes"],
            item["fingerprint"],
        )
    )
    return _non_overlapping_opportunities(opportunities, limit=limit)


def _non_overlapping_opportunities(
    opportunities: list[dict[str, Any]], *, limit: int
) -> list[dict[str, Any]]:
    selected: list[dict[str, Any]] = []
    occupied: dict[str, list[str]] = defaultdict(list)
    for opportunity in opportunities:
        locations = [
            (occurrence["schemaPath"], occurrence["jsonPointer"])
            for occurrence in opportunity["occurrences"]
        ]
        if any(
            _pointers_overlap(pointer, existing)
            for schema_path, pointer in locations
            for existing in occupied[schema_path]
        ):
            continue
        selected.append(opportunity)
        for schema_path, pointer in locations:
            occupied[schema_path].append(pointer)
        if len(selected) == limit:
            break
    return selected


def _pointers_overlap(left: str, right: str) -> bool:
    return left == right or left.startswith(right + "/") or right.startswith(left + "/")


def _definition_root(pointer: str) -> str | None:
    if not pointer.startswith("/$defs/"):
        return None
    return "/".join(pointer.split("/")[:3])


def _family_scope(
    schema_paths: set[str],
    assignments: dict[str, dict[str, str | None]] | None,
) -> tuple[list[str], str]:
    if assignments is None:
        return [], "unclassified"
    assigned = [assignments.get(path, {}).get("familyId") for path in schema_paths]
    family_ids = sorted({str(family_id) for family_id in assigned if family_id})
    has_unclassified = any(family_id is None for family_id in assigned)
    if not family_ids:
        return family_ids, "unclassified"
    if has_unclassified:
        return family_ids, "mixed"
    if len(family_ids) == 1:
        return family_ids, "family-local"
    return family_ids, "cross-family"
