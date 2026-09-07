# SPDX-FileCopyrightText: 2026 tao3k team and Contributors
#
# SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

"""Load canonical JSON Schema documents from the repository registry."""

from __future__ import annotations

from dataclasses import dataclass
import hashlib
import json
from pathlib import Path
from typing import Any, Iterator

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


@dataclass(frozen=True)
class ProtoContract:
    """A canonical protobuf contract owned by a runtime package.

    Protobuf sources are intentionally not discovered from ``schemas/``.  The
    runtime-server source is the owner; this record only projects its stable
    identity and declared consumers for read-only catalog receipts.
    """

    relative_path: str
    owner_package: str
    consumers: tuple[str, ...]
    source_digest: str


_PROVIDER_STREAM_PROTO = (
    "crates/agent-semantic-runtime-server/proto/asp-provider-stream.proto"
)
_PROVIDER_STREAM_OWNER = "agent-semantic-runtime-server"
_PROVIDER_STREAM_CONSUMERS = (
    "runtime-server:server-binding",
    "provider-transport:client-binding",
)


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


def load_proto_contracts(
    workspace_root: Path,
) -> tuple[list[ProtoContract], list[dict[str, Any]]]:
    """Load the runtime-server-owned protobuf contract receipt.

    This is deliberately an explicit owner lookup rather than a recursive
    ``*.proto`` scan.  Build consumers may reference this file from their own
    package trees, but those references do not create additional sources or
    owners.  The digest covers the exact source bytes so formatting changes
    are visible to downstream publication checks.
    """

    root = workspace_root.resolve()
    path = root / _PROVIDER_STREAM_PROTO
    if not path.is_file():
        return [], [
            {
                "severity": "error",
                "code": "missing-proto-contract",
                "message": "canonical runtime-server protobuf contract is missing",
                "protoPath": _PROVIDER_STREAM_PROTO,
            }
        ]
    try:
        source = path.read_bytes()
    except OSError as error:
        return [], [
            {
                "severity": "error",
                "code": "unreadable-proto-contract",
                "message": str(error),
                "protoPath": _PROVIDER_STREAM_PROTO,
            }
        ]

    contract = ProtoContract(
        relative_path=_PROVIDER_STREAM_PROTO,
        owner_package=_PROVIDER_STREAM_OWNER,
        consumers=_PROVIDER_STREAM_CONSUMERS,
        source_digest=f"sha256:{hashlib.sha256(source).hexdigest()}",
    )
    return [contract], []
