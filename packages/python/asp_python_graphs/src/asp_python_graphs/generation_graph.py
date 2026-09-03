"""Construct one immutable Search generation graph from typed provider facts."""

from __future__ import annotations

import json
from collections.abc import Mapping
from dataclasses import dataclass
from typing import Any

import blake3

from .generation_graph_validation import (
    canonical_digest as _canonical_digest,
    canonical_json as _canonical_json,
    canonical_strings as _canonical_strings,
    exact_keys as _exact_keys,
    mapping as _mapping,
    string as _string,
    validate_identity as _validate_identity,
    validate_source_snapshot as _validate_source_snapshot,
    validate_workspace_generation as _validate_workspace_generation,
)


REQUEST_SCHEMA_ID = "agent.semantic-protocols.search-generation-graph-request"
RECEIPT_SCHEMA_ID = "agent.semantic-protocols.search-generation-graph-receipt"


@dataclass(frozen=True, slots=True)
class CompiledGenerationGraph:
    """One content-bound graph plus its public compilation receipt."""

    graph: Mapping[str, object]
    receipt: Mapping[str, object]


def build_generation_graph(payload: Mapping[str, Any]) -> dict[str, object]:
    """Return the content-bound Python graph stage receipt for one candidate."""

    return dict(compile_generation_graph(payload).receipt)


def compile_generation_graph(payload: Mapping[str, Any]) -> CompiledGenerationGraph:
    """Compile the immutable graph retained by the service generation table."""

    _exact_keys(
        payload,
        {
            "schemaId",
            "schemaVersion",
            "identity",
            "sourceSnapshot",
            "workspaceGeneration",
            "ownerPaths",
            "relations",
        },
    )
    if (
        payload.get("schemaId") != REQUEST_SCHEMA_ID
        or payload.get("schemaVersion") != "1"
    ):
        raise ValueError("search generation graph request schema mismatch")
    identity = _mapping(payload, "identity")
    source_snapshot = _mapping(payload, "sourceSnapshot")
    workspace_generation = _mapping(payload, "workspaceGeneration")
    _validate_identity(identity)
    _validate_source_snapshot(source_snapshot)
    _validate_workspace_generation(workspace_generation)
    owner_paths = _canonical_strings(payload, "ownerPaths")
    if source_snapshot.get("rootDigest") != workspace_generation.get("rootDigest"):
        raise ValueError("search generation graph source identity drift")
    if identity.get("sourceRootDigest") != _canonical_digest(
        source_snapshot.get("rootDigest")
    ):
        raise ValueError("search generation graph identity root drift")
    if identity.get("providerDigest") != _canonical_digest(
        source_snapshot.get("providerDigest")
    ):
        raise ValueError("search generation graph identity provider drift")
    if source_snapshot.get("leafCount") != workspace_generation.get("leafCount"):
        raise ValueError("search generation graph leaf count drift")
    if workspace_generation.get("ownerCount") != len(owner_paths):
        raise ValueError("search generation graph owner count drift")

    nodes: dict[str, dict[str, object]] = {}
    edges: set[tuple[str, str, str]] = set()
    owners = set(owner_paths)
    for owner_path in owner_paths:
        _insert_owner(nodes, owner_path)
    relations = payload.get("relations")
    if not isinstance(relations, list):
        raise ValueError("search generation graph relations must be an array")
    relation_keys = [_canonical_json(relation) for relation in relations]
    if relation_keys != sorted(set(relation_keys)):
        raise ValueError("search generation graph relations must be a canonical set")
    for relation in relations:
        if not isinstance(relation, Mapping):
            raise ValueError("search generation graph relation must be an object")
        _exact_keys(relation, {"from", "kind", "to"})
        relation_kind = _string(relation, "kind")
        source = _endpoint(relation, "from", owners, nodes)
        target = _endpoint(relation, "to", owners, nodes)
        edge = (source, target, relation_kind)
        if edge in edges:
            raise ValueError("search generation graph contains a duplicate relation")
        edges.add(edge)

    graph = {
        "nodes": [nodes[node_id] for node_id in sorted(nodes)],
        "edges": [
            {"source": source, "target": target, "relation": relation}
            for source, target, relation in sorted(edges)
        ],
    }
    open_payload = {
        "graph": graph,
        "sourceSnapshot": dict(source_snapshot),
        "workspaceGeneration": dict(workspace_generation),
    }
    digest_bytes = json.dumps(
        open_payload, ensure_ascii=False, separators=(",", ":"), sort_keys=True
    ).encode("utf-8")
    receipt = {
        "schemaId": RECEIPT_SCHEMA_ID,
        "schemaVersion": "1",
        "identity": dict(identity),
        "entryOwnerIds": owner_paths,
        "entryNodeIds": [_stable_node_id("owner", owner) for owner in owner_paths],
        "candidateOwnerIds": owner_paths,
        "artifactDigest": f"blake3-256:{blake3.blake3(digest_bytes).hexdigest()}",
        "complete": True,
    }
    return CompiledGenerationGraph(graph=graph, receipt=receipt)


def _endpoint(
    relation: Mapping[str, Any],
    field: str,
    owners: set[str],
    nodes: dict[str, dict[str, object]],
) -> str:
    endpoint = _mapping(relation, field)
    _exact_keys(endpoint, {"kind", "id"})
    kind = _string(endpoint, "kind")
    value = _string(endpoint, "id")
    if kind == "owner":
        if value not in owners:
            raise ValueError(
                f"search generation graph references unadmitted owner: {value}"
            )
        return _insert_owner(nodes, value)
    node_id = _stable_node_id(kind, value)
    action = "code" if kind == "item" else "tests" if kind == "test" else "topology"
    nodes.setdefault(
        node_id,
        {
            "id": node_id,
            "kind": kind,
            "role": kind,
            "value": value,
            "action": action,
            "confidence": "parser",
        },
    )
    return node_id


def _insert_owner(nodes: dict[str, dict[str, object]], owner_path: str) -> str:
    node_id = _stable_node_id("owner", owner_path)
    nodes.setdefault(
        node_id,
        {
            "id": node_id,
            "kind": "owner",
            "role": "path",
            "value": owner_path,
            "action": "owner",
            "path": owner_path,
            "ownerPath": owner_path,
            "confidence": "exact",
        },
    )
    return node_id


def _stable_node_id(kind: str, value: str) -> str:
    rendered = "".join(
        character.lower()
        if character.isascii() and character.isalnum()
        else character
        if character in "_-/ .".replace(" ", "")
        else "-"
        for character in value
    ).rstrip("-")
    return f"{kind}:{rendered or 'node'}"
