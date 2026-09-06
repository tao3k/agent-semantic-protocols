"""Semantic handoff from one admitted Search settlement to Query Playbook."""

from __future__ import annotations

from ._topology_admission import require_topology


_BINDING_FIELDS = (
    "workspaceIdentity",
    "sourceGenerationDigest",
    "providerCatalogDigest",
    "topologyLibraryDigest",
    "topologyGenerationDigest",
    "structuralTopologyDigest",
    "semanticTopologyDigest",
    "inferenceProgramDigest",
)


def validate_query_playbook_handoff(request: dict, settlement: dict) -> None:
    """Require an exact, replay-safe projection of Search MaterializationSet."""

    require_topology(
        all(
            request["settlementBinding"].get(field) == settlement["binding"].get(field)
            for field in _BINDING_FIELDS
        ),
        "query-playbook-binding-mismatch",
    )
    materialization = settlement["materializationSet"]
    require_topology(
        request["searchRequestId"] == materialization["requestId"],
        "query-playbook-request-mismatch",
    )
    require_topology(
        request["materializationSetDigest"] == materialization["digest"]
        and request["selectors"] == materialization["selectors"]
        and request["proofDependencies"] == materialization["proofDependencies"],
        "query-playbook-materialization-mismatch",
    )
