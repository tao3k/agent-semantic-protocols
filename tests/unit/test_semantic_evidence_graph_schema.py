"""Validate the semantic evidence graph schema contract."""

import json
from pathlib import Path

from jsonschema import Draft202012Validator


def _load_schema() -> dict:
    path = Path(__file__).resolve().parents[2] / "schemas" / "semantic-evidence-graph.v1.schema.json"
    return json.loads(path.read_text())


def test_semantic_evidence_graph_schema_is_valid() -> None:
    Draft202012Validator.check_schema(_load_schema())


def test_semantic_evidence_graph_accepts_review_evidence_graph() -> None:
    schema = _load_schema()
    value = {
        "schemaId": "agent.semantic-protocols.semantic-evidence-graph",
        "schemaVersion": "1",
        "protocolId": "agent.semantic-protocols.evidence-graph",
        "protocolVersion": "1",
        "graphId": "rust.evidence.graph",
        "producer": {
            "languageId": "rust",
            "providerId": "rs-harness",
            "namespace": "agent.semantic-protocols.languages.rust.rs-harness",
        },
        "project": {"root": "."},
        "summary": {
            "nodes": 5,
            "edges": 4,
            "owners": 1,
            "claims": 1,
            "staleItems": 1,
            "gaps": 1,
        },
        "nodes": [
            {
                "nodeId": "review-packet:rust.review.packet",
                "kind": "review-packet",
                "label": "rust.review.packet",
                "packetId": "rust.review.packet",
                "summary": "review packet",
            },
            {
                "nodeId": "owner:src.model.rs",
                "kind": "owner",
                "label": "src/model.rs",
                "ownerPath": "src/model.rs",
            },
            {
                "nodeId": "invariant:agent-r027:src.model.rs:42",
                "kind": "invariant-candidate",
                "label": "semantic fields need named type",
                "candidateId": "agent-r027:src.model.rs:42",
                "ownerPath": "src/model.rs",
                "status": "changed",
                "location": {"path": "src/model.rs", "line": 42, "column": 0},
                "summary": "public data shape should not expose stringly fields",
            },
            {
                "nodeId": "waiver:waiver.agent-r027.src-model",
                "kind": "waiver",
                "label": "waiver.agent-r027.src-model",
                "waiverId": "waiver.agent-r027.src-model",
                "status": "stale",
            },
            {
                "nodeId": "review-action:run-receipt.agent-r027.src-model.expect-test",
                "kind": "review-action",
                "label": "Run expect-test for agent-r027:src.model.rs:42",
                "actionId": "run-receipt.agent-r027.src-model.expect-test",
                "status": "missing",
            },
        ],
        "edges": [
            {
                "edgeId": "edge:review-packet.rust.review.packet:invariant.agent-r027.src.model.rs.42",
                "kind": "derived-from",
                "fromNodeId": "invariant:agent-r027:src.model.rs:42",
                "toNodeId": "review-packet:rust.review.packet",
            },
            {
                "edgeId": "edge:invariant.agent-r027.src.model.rs.42:owner.src.model.rs",
                "kind": "derived-from",
                "fromNodeId": "invariant:agent-r027:src.model.rs:42",
                "toNodeId": "owner:src.model.rs",
            },
            {
                "edgeId": "edge:invariant.agent-r027.src.model.rs.42:waiver.waiver.agent-r027.src-model",
                "kind": "waived-by",
                "fromNodeId": "invariant:agent-r027:src.model.rs:42",
                "toNodeId": "waiver:waiver.agent-r027.src-model",
            },
            {
                "edgeId": "edge:review-packet.rust.review.packet:review-action.run-receipt.agent-r027.src-model.expect-test",
                "kind": "suggests-action",
                "fromNodeId": "review-packet:rust.review.packet",
                "toNodeId": "review-action:run-receipt.agent-r027.src-model.expect-test",
            },
        ],
        "gaps": [
            {
                "gapId": "gap:agent-r027:src.model.rs:42:expect-test",
                "ownerPath": "src/model.rs",
                "summary": "no passed expect-test receipt linked to candidate",
                "severity": "warning",
            }
        ],
    }

    Draft202012Validator(schema).validate(value)


def test_semantic_evidence_graph_rejects_absolute_owner_paths() -> None:
    schema = _load_schema()
    value = {
        "schemaId": "agent.semantic-protocols.semantic-evidence-graph",
        "schemaVersion": "1",
        "protocolId": "agent.semantic-protocols.evidence-graph",
        "protocolVersion": "1",
        "graphId": "rust.evidence.graph",
        "producer": {
            "languageId": "rust",
            "providerId": "rs-harness",
            "namespace": "agent.semantic-protocols.languages.rust.rs-harness",
        },
        "project": {"root": "."},
        "summary": {
            "nodes": 1,
            "edges": 0,
            "owners": 1,
            "claims": 0,
            "staleItems": 0,
            "gaps": 0,
        },
        "nodes": [
            {
                "nodeId": "owner:src.model.rs",
                "kind": "owner",
                "label": "/tmp/src/model.rs",
                "ownerPath": "/tmp/src/model.rs",
            }
        ],
        "edges": [],
    }

    errors = list(Draft202012Validator(schema).iter_errors(value))
    assert errors
