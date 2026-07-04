"""Semantic WhereFrame, DynamicTopology, and HowFrame schema tests."""

from __future__ import annotations

from copy import deepcopy
from pathlib import Path
from typing import Any

from ..schema_validation import schema_validator_for


_REPO_ROOT = Path(__file__).resolve().parents[3]


def _errors(schema_name: str, packet: dict[str, Any]) -> list[str]:
    validator = schema_validator_for(_REPO_ROOT / "schemas" / schema_name)
    return sorted(error.message for error in validator.iter_errors(packet))


def _where_frame() -> dict[str, Any]:
    return {
        "schemaId": "semantic-where-frame.v1",
        "schemaVersion": "1",
        "protocolId": "agent.semantic-protocols.where-how",
        "protocolVersion": "1",
        "frameId": "where-frame-1",
        "topologyId": "topology-1",
        "intent": {
            "summary": "preserve search-stage receipt contract while moving to async DB search",
            "terms": ["search-stage", "receipt", "async"],
        },
        "recommendedNext": {
            "kind": "stop-and-synthesize-how",
            "reason": "owners, schema, scenario, and tests are localized",
        },
        "confirmed": {
            "owners": [
                {
                    "id": "owner:agent-semantic-search",
                    "kind": "crate",
                    "ownerPath": "crates/agent-semantic-search",
                }
            ],
            "selectors": [
                {
                    "id": "selector:SearchStageReceipt",
                    "kind": "schema",
                    "selector": "SearchStageReceipt",
                }
            ],
            "tests": [
                {
                    "id": "test:search-stage-receipt-schema",
                    "kind": "test",
                }
            ],
            "schemas": [
                {
                    "id": "schema:semantic-search-stage-receipt.v1",
                    "kind": "schema",
                }
            ],
        },
        "missingEvidence": [],
        "avoid": [
            {
                "action": "line-range identity",
                "reason": "selectors and schema ids are the durable identity",
            }
        ],
        "searchPlan": [
            {
                "step": 1,
                "kind": "policy-query",
                "purpose": "attach branch legality and scenario facts before editing",
            }
        ],
        "evidence": [
            {
                "id": "scenario:search-stage-receipt.warm-path",
                "kind": "scenario",
            }
        ],
    }


def _dynamic_topology() -> dict[str, Any]:
    return {
        "schemaId": "semantic-dynamic-topology.v1",
        "schemaVersion": "1",
        "protocolId": "agent.semantic-protocols.where-how",
        "protocolVersion": "1",
        "topologyId": "topology-1",
        "project": {
            "root": "/repo",
            "repoId": "repo-1",
            "workspaceId": "workspace-1",
        },
        "scope": {
            "kind": "feature",
            "featureId": "feature-search-stage-receipt",
            "evidenceGeneration": "generation-1",
        },
        "target": {
            "role": "invariant-owner",
            "selector": "SearchStageReceipt",
            "ownerPath": "schemas/semantic-search-stage-receipt.v1.schema.json",
            "languageId": "json",
            "summary": "receipt schema is the invariant owner for downstream render and graph evidence",
        },
        "ownership": [
            {
                "kind": "schema",
                "id": "schema:semantic-search-stage-receipt.v1",
                "selector": "SearchStageReceipt",
                "confidence": 1,
            }
        ],
        "structuralEdges": [
            {
                "from": "schema:semantic-search-stage-receipt.v1",
                "to": "renderer:compact-graph",
                "kind": "renders",
                "evidenceId": "snapshot:compact-render-contract",
            }
        ],
        "trustBoundary": "internal-contract",
        "pressures": ["hot-path", "contract-stability", "prompt-critical-render"],
        "proofs": [
            {
                "kind": "scenario",
                "id": "scenario:search-stage-receipt.warm-path",
                "status": "active",
            }
        ],
        "staleFacts": [],
        "branchLegality": {
            "legalBranches": [
                {
                    "branch": "producer-fix",
                    "reason": "schema owner must preserve the receipt invariant",
                    "evidenceId": "scenario:search-stage-receipt.warm-path",
                }
            ],
            "illegalBranches": [
                {
                    "branch": "consumer-side-fallback",
                    "reason": "missing writer proof must route to producer/schema/test evidence",
                }
            ],
            "unknownBranches": [],
        },
        "policyRefs": ["policy:rust.async-boundary.no-blocking-search-loop"],
    }


def _how_frame() -> dict[str, Any]:
    return {
        "schemaId": "semantic-how-frame.v1",
        "schemaVersion": "1",
        "protocolId": "agent.semantic-protocols.where-how",
        "protocolVersion": "1",
        "frameId": "how-frame-1",
        "whereFrameId": "where-frame-1",
        "topologyId": "topology-1",
        "decision": {
            "changeShape": "edit async search stage without adding a blocking scan",
            "recommendedNext": "compare candidate plans under branch legality",
        },
        "why": [
            {
                "id": "policy:rust.async-boundary.no-blocking-search-loop",
                "kind": "policy",
                "summary": "warm-path search must not add a blocking scan",
            },
            {
                "id": "scenario:search-stage-receipt.warm-path",
                "kind": "scenario",
                "summary": "scenario receipt proves the expected high-performance path",
            },
        ],
        "illegal": [
            {
                "branch": "consumer-side fallback",
                "reason": "receipt writer proof belongs at the producer/schema/test boundary",
            }
        ],
        "validate": [
            {
                "kind": "scenario",
                "commandOrId": "scenario:search-stage-receipt.warm-path",
            },
            {
                "kind": "test",
                "commandOrId": "test:search-stage-receipt-schema",
            },
        ],
        "evidence": [
            {
                "id": "benchmark:source-index-search-pipe.warm-path",
                "kind": "benchmark",
            }
        ],
        "branchLegality": {
            "targetRole": "invariant-owner",
            "trustBoundary": "internal-contract",
            "invariantOwner": "schema:semantic-search-stage-receipt.v1",
            "allowedRecoveries": ["route-to-owner", "refresh-evidence"],
            "prunedBranches": [
                {
                    "branch": "consumer-side fallback",
                    "reason": "target is an internal contract owner",
                }
            ],
            "evidenceGaps": [],
            "validation": ["schema", "scenario", "test"],
        },
    }


def test_where_topology_how_frames_are_valid() -> None:
    assert _errors("semantic-where-frame.v1.schema.json", _where_frame()) == []
    assert (
        _errors("semantic-dynamic-topology.v1.schema.json", _dynamic_topology())
        == []
    )
    assert _errors("semantic-how-frame.v1.schema.json", _how_frame()) == []


def test_where_frame_rejects_line_range_as_selector_identity() -> None:
    packet = deepcopy(_where_frame())
    packet["confirmed"]["selectors"][0]["lineRange"] = "12:18"

    errors = _errors("semantic-where-frame.v1.schema.json", packet)

    assert any("Additional properties are not allowed" in error for error in errors)


def test_dynamic_topology_requires_branch_legality() -> None:
    packet = deepcopy(_dynamic_topology())
    del packet["branchLegality"]

    errors = _errors("semantic-dynamic-topology.v1.schema.json", packet)

    assert "'branchLegality' is a required property" in errors
