"""Compose schema catalog, reference, Rust usage, and lifecycle audit evidence."""

from __future__ import annotations

from pathlib import Path
from typing import Any

from .catalog import definition_usage, load_catalog, schema_edges
from .families import (
    classify_schema_families,
    definition_visibility_diagnostics,
    load_family_registry,
)
from .lifecycle import lifecycle_state, load_lifecycle_manifest
from .reference_decisions import (
    load_reference_decisions,
    reconcile_reference_decisions,
)
from .references import discover_reference_opportunities
from .rust_usage import rust_usage


DEFAULT_MANIFEST = Path("packages/python/asp_schema_manager/asp-schema-lifecycle.v1.json")
DEFAULT_FAMILY_REGISTRY = Path(
    "packages/python/asp_schema_manager/asp-schema-families.v1.json"
)
DEFAULT_REFERENCE_DECISIONS = Path(
    "packages/python/asp_schema_manager/asp-schema-reference-decisions.v1.json"
)


def audit_workspace(
    workspace_root: Path,
    *,
    manifest_path: Path | None = None,
    family_registry_path: Path | None = None,
    reference_decisions_path: Path | None = None,
    minimum_reference_bytes: int = 120,
    reference_limit: int = 200,
) -> dict[str, Any]:
    root = workspace_root.resolve()
    documents, diagnostics = load_catalog(root)
    edges, inbound, edge_diagnostics = schema_edges(documents)
    diagnostics.extend(edge_diagnostics)
    resolved_family_registry = family_registry_path or (root / DEFAULT_FAMILY_REGISTRY)
    families, family_contract_diagnostics = load_family_registry(
        resolved_family_registry,
        root / "schemas/asp-schema-family-registry.v1.schema.json",
    )
    family_assignments, family_diagnostics = classify_schema_families(
        documents, families
    )
    diagnostics.extend(family_contract_diagnostics)
    diagnostics.extend(family_diagnostics)
    diagnostics.extend(
        definition_visibility_diagnostics(edges, family_assignments, families)
    )
    used_definitions, unused_definitions = definition_usage(documents)
    opportunities = discover_reference_opportunities(
        documents,
        family_assignments=family_assignments,
        used_definitions=used_definitions,
        minimum_bytes=minimum_reference_bytes,
        limit=reference_limit,
    )
    resolved_reference_decisions = reference_decisions_path or (
        root / DEFAULT_REFERENCE_DECISIONS
    )
    reference_decisions, reference_decision_diagnostics = load_reference_decisions(
        resolved_reference_decisions,
        root / "schemas/asp-schema-reference-decision-registry.v1.schema.json",
    )
    diagnostics.extend(reference_decision_diagnostics)
    diagnostics.extend(
        reconcile_reference_decisions(opportunities, reference_decisions)
    )
    rust_states, rust_evidence = rust_usage(root, documents, edges)
    resolved_manifest = manifest_path or (root / DEFAULT_MANIFEST)
    entries, lifecycle_diagnostics = load_lifecycle_manifest(
        resolved_manifest,
        root / "schemas/asp-schema-lifecycle-manifest.v1.schema.json",
    )
    diagnostics.extend(lifecycle_diagnostics)
    known_paths = {document.relative_path for document in documents}
    for path in sorted(entries.keys() - known_paths):
        diagnostics.append(
            {
                "severity": "error",
                "code": "lifecycle-schema-missing",
                "message": "lifecycle entry names a missing schema",
                "schemaPath": path,
            }
        )

    schema_states: list[dict[str, Any]] = []
    for document in documents:
        path = document.relative_path
        lifecycle = lifecycle_state(path, rust_states[path], inbound[path], entries)
        if lifecycle["lifecycleStatus"] in {"retired", "removal-candidate"} and (
            rust_states[path] != "unobserved" or inbound[path] > 0
        ):
            diagnostics.append(
                {
                    "severity": "error",
                    "code": "unsafe-lifecycle-retirement",
                    "message": "retired/removal-candidate schema still has Rust or inbound reference evidence",
                    "schemaPath": path,
                }
            )
        schema_states.append(
            {
                "schemaPath": path,
                "schemaIdentifier": document.schema_identifier,
                "hasReferences": bool(document.references),
                "inboundReferenceCount": inbound[path],
                "rustUsage": rust_states[path],
                "rustEvidence": rust_evidence[path],
                "unusedDefinitionPointers": unused_definitions[path],
                **family_assignments[path],
                **lifecycle,
            }
        )

    diagnostics.sort(
        key=lambda item: (
            item["severity"],
            item["code"],
            item.get("schemaPath", ""),
            item["message"],
        )
    )
    counts = {state: sum(item["rustUsage"] == state for item in schema_states) for state in ("direct", "transitive", "unobserved")}
    total_schema_bytes = sum(document.byte_length for document in documents)
    estimated_reducible_bytes = sum(item["estimatedReducibleBytes"] for item in opportunities)
    opportunity_scope_counts = {
        scope: sum(item["familyScope"] == scope for item in opportunities)
        for scope in ("family-local", "cross-family", "mixed", "unclassified")
    }
    return {
        "schemaId": "asp.schema-management-report.v1",
        "schemaVersion": "1",
        "summary": {
            "schemaCount": len(documents),
            "schemaWithRefCount": sum(bool(document.references) for document in documents),
            "totalSchemaBytes": total_schema_bytes,
            "referenceOpportunityCount": len(opportunities),
            "estimatedReducibleBytes": estimated_reducible_bytes,
            "estimatedReductionBasisPoints": (
                estimated_reducible_bytes * 10_000 // total_schema_bytes
                if total_schema_bytes
                else 0
            ),
            "directRustCount": counts["direct"],
            "transitiveRustCount": counts["transitive"],
            "unobservedRustCount": counts["unobserved"],
            "removalCandidateCount": sum(item["lifecycleStatus"] == "removal-candidate" for item in schema_states),
            "familyCount": len(families),
            "classifiedSchemaCount": sum(item["familyId"] is not None for item in schema_states),
            "unclassifiedSchemaCount": sum(item["familyId"] is None for item in schema_states),
            "referenceOpportunityScopeCounts": opportunity_scope_counts,
            "unusedDefinitionCount": sum(
                len(pointers) for pointers in unused_definitions.values()
            ),
        },
        "diagnostics": diagnostics,
        "referenceOpportunities": opportunities,
        "schemas": schema_states,
    }
