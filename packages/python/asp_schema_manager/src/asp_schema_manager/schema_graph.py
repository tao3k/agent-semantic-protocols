# SPDX-FileCopyrightText: 2026 tao3k team and Contributors
#
# SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

"""Resolve cross-Schema references and definition reachability."""

from __future__ import annotations

from pathlib import Path
from typing import Any, Iterator
from urllib.parse import unquote, urldefrag, urlparse

from .catalog import SchemaDocument


def _reference_occurrences(
    value: Any, pointer: str = ""
) -> Iterator[tuple[str, str]]:
    if isinstance(value, dict):
        for keyword in ("$ref", "$dynamicRef"):
            reference = value.get(keyword)
            if isinstance(reference, str):
                yield pointer, reference
        for key, child in value.items():
            child_pointer = f"{pointer}/{_escape_pointer(str(key))}"
            yield from _reference_occurrences(child, child_pointer)
    elif isinstance(value, list):
        for index, child in enumerate(value):
            yield from _reference_occurrences(child, f"{pointer}/{index}")


def _escape_pointer(token: str) -> str:
    return token.replace("~", "~0").replace("/", "~1")


def schema_edges(
    documents: list[SchemaDocument],
) -> tuple[dict[str, set[str]], dict[str, int], list[dict[str, Any]]]:
    by_identifier = {
        document.schema_identifier: document.relative_path
        for document in documents
        if document.schema_identifier
    }
    by_filename = {document.path.name: document.relative_path for document in documents}
    by_path = {document.relative_path: document for document in documents}
    edges = {document.relative_path: set() for document in documents}
    inbound = {document.relative_path: 0 for document in documents}
    diagnostics = _duplicate_identifier_diagnostics(documents)

    for document in documents:
        for reference in document.references:
            target_uri, fragment = urldefrag(reference)
            target = document.relative_path if not target_uri else by_identifier.get(target_uri)
            if target is None and target_uri:
                target = by_filename.get(Path(urlparse(target_uri).path).name)
            if target is None:
                if urlparse(target_uri).hostname in {
                    "json-schema.org",
                    "www.json-schema.org",
                }:
                    continue
                diagnostics.append(
                    {
                        "severity": "warning",
                        "code": "unresolved-schema-reference",
                        "message": f"unresolved $ref {reference}",
                        "schemaPath": document.relative_path,
                    }
                )
                continue
            if fragment and not _fragment_exists(by_path[target].value, unquote(fragment)):
                diagnostics.append(
                    {
                        "severity": "error",
                        "code": "unresolved-schema-reference-fragment",
                        "message": f"unresolved $ref fragment {reference}",
                        "schemaPath": document.relative_path,
                    }
                )
                continue
            if target not in edges[document.relative_path]:
                edges[document.relative_path].add(target)
                inbound[target] += 1
    return edges, inbound, diagnostics


def _duplicate_identifier_diagnostics(
    documents: list[SchemaDocument],
) -> list[dict[str, Any]]:
    identifiers: dict[str, list[str]] = {}
    for document in documents:
        if document.schema_identifier:
            identifiers.setdefault(document.schema_identifier, []).append(
                document.relative_path
            )
    return [
        {
            "severity": "error",
            "code": "duplicate-schema-identifier",
            "message": f"duplicate $id {identifier}: {', '.join(paths)}",
        }
        for identifier, paths in sorted(identifiers.items())
        if len(paths) > 1
    ]


def _fragment_exists(value: Any, fragment: str) -> bool:
    if fragment.startswith("/"):
        return _json_pointer_exists(value, fragment)
    return _anchor_pointer(value, fragment) is not None


def _json_pointer_exists(value: Any, pointer: str) -> bool:
    current = value
    for encoded_token in pointer.removeprefix("/").split("/"):
        token = encoded_token.replace("~1", "/").replace("~0", "~")
        if isinstance(current, dict) and token in current:
            current = current[token]
            continue
        if isinstance(current, list) and token.isdigit() and int(token) < len(current):
            current = current[int(token)]
            continue
        return False
    return True


def definition_usage(
    documents: list[SchemaDocument],
) -> tuple[set[tuple[str, str]], dict[str, list[str]]]:
    by_identifier = {
        document.schema_identifier: document.relative_path
        for document in documents
        if document.schema_identifier
    }
    by_filename = {document.path.name: document.relative_path for document in documents}
    by_path = {document.relative_path: document for document in documents}
    occurrences = {
        document.relative_path: list(_reference_occurrences(document.value))
        for document in documents
    }
    used: set[tuple[str, str]] = set()
    changed = True
    while changed:
        changed = False
        for source_path, references in occurrences.items():
            for origin_pointer, reference in references:
                source_definition = _definition_root(origin_pointer)
                if source_definition is not None and (
                    source_path,
                    source_definition,
                ) not in used:
                    continue
                target_definition = _referenced_definition(
                    source_path,
                    reference,
                    by_identifier,
                    by_filename,
                    by_path,
                )
                if target_definition is None or target_definition in used:
                    continue
                used.add(target_definition)
                changed = True
    unused = {
        document.relative_path: [
            f"/$defs/{_escape_pointer(str(name))}"
            for name in document.value.get("$defs", {})
            if (
                document.relative_path,
                f"/$defs/{_escape_pointer(str(name))}",
            )
            not in used
        ]
        for document in documents
        if isinstance(document.value.get("$defs", {}), dict)
    }
    return used, unused


def _referenced_definition(
    source_path: str,
    reference: str,
    by_identifier: dict[str, str],
    by_filename: dict[str, str],
    by_path: dict[str, SchemaDocument],
) -> tuple[str, str] | None:
    target_uri, fragment = urldefrag(reference)
    target_path = source_path if not target_uri else by_identifier.get(target_uri)
    if target_path is None and target_uri:
        target_path = by_filename.get(Path(urlparse(target_uri).path).name)
    if target_path is None or not fragment:
        return None
    decoded_fragment = unquote(fragment)
    target_definition = _definition_root(decoded_fragment)
    if target_definition is None and not decoded_fragment.startswith("/"):
        anchor_pointer = _anchor_pointer(by_path[target_path].value, decoded_fragment)
        target_definition = (
            _definition_root(anchor_pointer) if anchor_pointer is not None else None
        )
    return (
        (target_path, target_definition)
        if target_definition is not None
        else None
    )


def _definition_root(pointer: str) -> str | None:
    if not pointer.startswith("/$defs/"):
        return None
    return "/".join(pointer.split("/")[:3])


def _anchor_pointer(value: Any, anchor: str, pointer: str = "") -> str | None:
    if isinstance(value, dict):
        if value.get("$anchor") == anchor or value.get("$dynamicAnchor") == anchor:
            return pointer
        for key, child in value.items():
            found = _anchor_pointer(
                child, anchor, f"{pointer}/{_escape_pointer(str(key))}"
            )
            if found is not None:
                return found
    elif isinstance(value, list):
        for index, child in enumerate(value):
            found = _anchor_pointer(child, anchor, f"{pointer}/{index}")
            if found is not None:
                return found
    return None
