"""Generate deterministic, read-only semantic-proof plans from the schema catalog."""

from __future__ import annotations

import hashlib
import json
from pathlib import Path
import re
from typing import Any, Iterable

from ._logical_projection_keywords import keyword_facts
from ._logical_projection_obligations import proof_obligations
from .audit import audit_workspace
from .catalog import SchemaDocument, load_catalog, schema_edges


CONTRACT_COMPOSITION = {
    "logicalProjectionDefinition": (
        "schemas/semantic-proof-definitions.v1.schema.json#/$defs/schemaProjection"
    ),
    "obligationSchema": "schemas/semantic-proof-obligation.v1.schema.json",
    "recipeSchema": "schemas/semantic-proof-recipe.v1.schema.json",
    "proofBundleSchema": "schemas/lean-org-typst-proof-bundle-index.v1.schema.json",
    "receiptSchema": "schemas/semantic-proof-receipt.v1.schema.json",
    "checkerIds": ["axle.check", "axle.verify_proof", "lean"],
}


def generate_proof_plan(
    workspace_root: Path,
    *,
    schema_paths: Iterable[str] | None = None,
    expected_plan_digest: str | None = None,
) -> dict[str, Any]:
    root = workspace_root.resolve()
    documents, catalog_diagnostics = load_catalog(root)
    edges, _, edge_diagnostics = schema_edges(documents)
    report = audit_workspace(root)
    plan = build_proof_plan(
        documents,
        edges,
        report["schemas"],
        schema_paths=schema_paths,
        expected_plan_digest=expected_plan_digest,
    )
    relevant_paths = {item["sourceSchema"] for item in plan["schemas"]}
    plan["diagnostics"] = [
        item
        for item in catalog_diagnostics + edge_diagnostics + report["diagnostics"]
        if item.get("schemaPath") in relevant_paths or item.get("schemaPath") is None
    ]
    return plan


def build_proof_plan(
    documents: list[SchemaDocument],
    edges: dict[str, set[str]],
    schema_states: list[dict[str, Any]],
    *,
    schema_paths: Iterable[str] | None = None,
    expected_plan_digest: str | None = None,
) -> dict[str, Any]:
    by_path = {document.relative_path: document for document in documents}
    state_by_path = {item["schemaPath"]: item for item in schema_states}
    selected = _selected_paths(by_path, schema_paths)
    projections = [
        _schema_projection(by_path[path], by_path, edges, state_by_path[path])
        for path in selected
    ]
    digest_payload = {
        "contractComposition": CONTRACT_COMPOSITION,
        "schemas": projections,
    }
    plan_digest = _digest(digest_payload)
    stale = expected_plan_digest is not None and expected_plan_digest != plan_digest
    return {
        "projectionKind": "schema-logical-proof-plan",
        "projectionVersion": "1",
        "planDigest": plan_digest,
        "validity": {
            "state": "stale" if stale else "current",
            "reasonKind": (
                "schema-proof-plan-digest-mismatch" if stale else "digest-match"
            ),
            "expectedPlanDigest": expected_plan_digest,
            "observedPlanDigest": plan_digest,
        },
        **digest_payload,
    }


def _schema_projection(
    document: SchemaDocument,
    by_path: dict[str, SchemaDocument],
    edges: dict[str, set[str]],
    state: dict[str, Any],
) -> dict[str, Any]:
    source_digest = _digest(document.value)
    closure_paths = _reference_closure(document.relative_path, edges)
    closure = [
        {
            "schemaPath": path,
            "contentDigest": _digest(by_path[path].value),
        }
        for path in closure_paths
    ]
    closure_digest = _digest(closure)
    keyword_details = keyword_facts(document.value, document.relative_path)
    logical_facts = [
        {
            "id": item["id"],
            "owner": item["owner"],
            "field": item["field"],
            "role": item["role"],
        }
        for item in keyword_details
    ]
    slug = _schema_slug(document.path.name)
    projection: dict[str, Any] = {
        "sourceSchema": document.relative_path,
        "sourceSchemaIdentifier": document.schema_identifier,
        "sourceContentDigest": source_digest,
        "resolvedReferenceClosure": closure,
        "resolvedReferenceClosureDigest": closure_digest,
        "family": {
            "familyId": state["familyId"],
            "familySource": state["familySource"],
        },
        "lifecycle": {
            "status": state["lifecycleStatus"],
            "source": state["lifecycleSource"],
        },
        "logicalProjection": {
            "sourceSchema": document.relative_path,
            "formalLeanPath": f"packages/proofs/lean/ASPProof/GeneratedSchema/{slug}.lean",
            "candidateLeanPath": (
                f"packages/proofs/lean/ASPProof/GeneratedSchema/{slug}Candidate.lean"
            ),
            "facts": logical_facts,
        },
        "keywordObligations": {
            "supported": sorted(
                {
                    item["keyword"]
                    for item in keyword_details
                    if item["support"] == "supported"
                }
            ),
            "unsupported": sorted(
                {
                    item["keyword"]
                    for item in keyword_details
                    if item["support"] == "unsupported"
                }
            ),
            "facts": keyword_details,
        },
        "obligations": proof_obligations(
            schema_path=document.relative_path,
            source_digest=source_digest,
            closure_digest=closure_digest,
            facts=keyword_details,
        ),
        "recipeBinding": {
            "state": "requires-axle-materialization",
            "contract": CONTRACT_COMPOSITION["recipeSchema"],
        },
        "proofBundleBinding": {
            "state": "requires-lean-bundle-binding",
            "contract": CONTRACT_COMPOSITION["proofBundleSchema"],
        },
        "receiptBinding": {
            "state": "unverified",
            "contract": CONTRACT_COMPOSITION["receiptSchema"],
        },
    }
    projection["projectionDigest"] = _digest(projection)
    return projection


def _selected_paths(
    documents: dict[str, SchemaDocument], schema_paths: Iterable[str] | None
) -> list[str]:
    if schema_paths is None:
        return sorted(documents)
    selected = sorted(set(schema_paths))
    missing = [path for path in selected if path not in documents]
    if missing:
        raise ValueError(f"unknown registered schema: {', '.join(missing)}")
    return selected


def _reference_closure(source: str, edges: dict[str, set[str]]) -> list[str]:
    found: set[str] = set()
    frontier = list(edges.get(source, ()))
    while frontier:
        path = frontier.pop()
        if path == source or path in found:
            continue
        found.add(path)
        frontier.extend(edges.get(path, ()))
    return sorted(found)


def _schema_slug(filename: str) -> str:
    stem = filename.removesuffix(".schema.json")
    words = [word for word in re.split(r"[^a-zA-Z0-9]+", stem) if word]
    return "".join(word[:1].upper() + word[1:] for word in words)


def _digest(value: Any) -> str:
    payload = json.dumps(
        value, ensure_ascii=False, sort_keys=True, separators=(",", ":")
    ).encode()
    return f"sha256:{hashlib.sha256(payload).hexdigest()}"
