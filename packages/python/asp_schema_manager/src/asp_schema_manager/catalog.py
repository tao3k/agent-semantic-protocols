"""Load the repository schema registry and resolve cross-document reference edges."""

from __future__ import annotations

from dataclasses import dataclass
import json
from pathlib import Path
from typing import Any, Iterator
from urllib.parse import unquote, urldefrag, urlparse

from jsonschema import Draft202012Validator
from jsonschema.exceptions import SchemaError


@dataclass(frozen=True)
class SchemaDocument:
    path: Path
    relative_path: str
    value: dict[str, Any]
    schema_identifier: str | None
    protocol_schema_id: str | None
    references: tuple[str, ...]
    byte_length: int


def _walk(value: Any) -> Iterator[Any]:
    yield value
    if isinstance(value, dict):
        for child in value.values():
            yield from _walk(child)
    elif isinstance(value, list):
        for child in value:
            yield from _walk(child)


def _references(value: dict[str, Any]) -> tuple[str, ...]:
    return tuple(
        reference
        for node in _walk(value)
        if isinstance(node, dict)
        for reference in (node.get("$ref"), node.get("$dynamicRef"))
        if isinstance(reference, str)
    )


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


def _protocol_schema_id(value: dict[str, Any]) -> str | None:
    properties = value.get("properties")
    if not isinstance(properties, dict):
        return None
    schema_id = properties.get("schemaId")
    if not isinstance(schema_id, dict):
        return None
    constant = schema_id.get("const")
    return constant if isinstance(constant, str) else None


def load_catalog(workspace_root: Path) -> tuple[list[SchemaDocument], list[dict[str, Any]]]:
    schemas_root = workspace_root / "schemas"
    documents: list[SchemaDocument] = []
    diagnostics: list[dict[str, Any]] = []
    for path in sorted(schemas_root.rglob("*.schema.json")):
        relative_path = path.relative_to(workspace_root).as_posix()
        try:
            source = path.read_text(encoding="utf-8")
            value = json.loads(source)
        except (OSError, json.JSONDecodeError) as error:
            diagnostics.append(
                {
                    "severity": "error",
                    "code": "invalid-schema-json",
                    "message": str(error),
                    "schemaPath": relative_path,
                }
            )
            continue
        if not isinstance(value, dict):
            diagnostics.append(
                {
                    "severity": "error",
                    "code": "schema-root-not-object",
                    "message": "JSON Schema root must be an object",
                    "schemaPath": relative_path,
                }
            )
            continue
        try:
            Draft202012Validator.check_schema(value)
        except SchemaError as error:
            diagnostics.append(
                {
                    "severity": "error",
                    "code": "invalid-json-schema",
                    "message": error.message,
                    "schemaPath": relative_path,
                    "jsonPointer": "/" + "/".join(str(part) for part in error.path),
                }
            )
        identifier = value.get("$id")
        documents.append(
            SchemaDocument(
                path=path,
                relative_path=relative_path,
                value=value,
                schema_identifier=identifier if isinstance(identifier, str) else None,
                protocol_schema_id=_protocol_schema_id(value),
                references=_references(value),
                byte_length=len(source.encode("utf-8")),
            )
        )
    return documents, diagnostics


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
    diagnostics: list[dict[str, Any]] = []

    identifiers: dict[str, list[str]] = {}
    for document in documents:
        if document.schema_identifier:
            identifiers.setdefault(document.schema_identifier, []).append(document.relative_path)
    for identifier, paths in sorted(identifiers.items()):
        if len(paths) > 1:
            diagnostics.append(
                {
                    "severity": "error",
                    "code": "duplicate-schema-identifier",
                    "message": f"duplicate $id {identifier}: {', '.join(paths)}",
                }
            )

    for document in documents:
        for reference in document.references:
            target_uri, fragment = urldefrag(reference)
            target = document.relative_path if not target_uri else by_identifier.get(target_uri)
            if target is None and target_uri:
                target = by_filename.get(Path(urlparse(target_uri).path).name)
            if target is None:
                host = urlparse(target_uri).hostname
                if host in {
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


def _fragment_exists(value: Any, fragment: str) -> bool:
    if fragment.startswith("/"):
        return _json_pointer_exists(value, fragment)
    return any(
        isinstance(node, dict)
        and (
            node.get("$anchor") == fragment
            or node.get("$dynamicAnchor") == fragment
        )
        for node in _walk(value)
    )


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
                target_uri, fragment = urldefrag(reference)
                target_path = source_path if not target_uri else by_identifier.get(target_uri)
                if target_path is None and target_uri:
                    target_path = by_filename.get(Path(urlparse(target_uri).path).name)
                if target_path is None or not fragment:
                    continue
                decoded_fragment = unquote(fragment)
                target_definition = _definition_root(decoded_fragment)
                if target_definition is None and not decoded_fragment.startswith("/"):
                    anchor_pointer = _anchor_pointer(
                        by_path[target_path].value, decoded_fragment
                    )
                    target_definition = (
                        _definition_root(anchor_pointer)
                        if anchor_pointer is not None
                        else None
                    )
                if target_definition is None:
                    continue
                location = (target_path, target_definition)
                if location not in used:
                    used.add(location)
                    changed = True
    unused: dict[str, list[str]] = {}
    for document in documents:
        definitions = document.value.get("$defs", {})
        names = definitions.keys() if isinstance(definitions, dict) else ()
        unused[document.relative_path] = [
            f"/$defs/{_escape_pointer(str(name))}"
            for name in names
            if (
                document.relative_path,
                f"/$defs/{_escape_pointer(str(name))}",
            )
            not in used
        ]
    return used, unused


def _definition_root(pointer: str) -> str | None:
    if not pointer.startswith("/$defs/"):
        return None
    parts = pointer.split("/")
    return "/".join(parts[:3])


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
