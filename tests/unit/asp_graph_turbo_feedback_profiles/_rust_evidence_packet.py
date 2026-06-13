"""Rust evidence graph-turbo request fixture."""

from __future__ import annotations


def rust_evidence_graph_turbo_request() -> dict[str, object]:
    return {
        "schemaId": "agent.semantic-protocols.semantic-graph-turbo-request",
        "schemaVersion": "1",
        "protocolId": "agent.semantic-protocols.semantic-language",
        "protocolVersion": "1",
        "packetKind": "graph-turbo-request",
        "requestId": "rust.evidence.analysis.test",
        "surface": "evidence-analyze",
        "queryTerms": ["rust evidence quality"],
        "profile": "rust-evidence-quality",
        "algorithm": "typed-ppr-diverse",
        "seedIds": ["owner:src/model.rs"],
        "budget": 8,
        "producer": {
            "languageId": "rust",
            "providerId": "rs-harness",
            "namespace": "agent.semantic-protocols.languages.rust.rs-harness",
        },
        "project": {"root": ".", "package": None, "fields": {}},
        "summary": {
            "graphs": 1,
            "nodes": 7,
            "edges": 6,
            "owners": 1,
            "claims": 1,
            "staleItems": 1,
            "gaps": 1,
        },
        "graphs": [
            {
                "graphId": "rust.evidence.graph.test",
                "summary": {
                    "nodes": 7,
                    "edges": 6,
                    "owners": 1,
                    "claims": 1,
                    "staleItems": 1,
                    "gaps": 1,
                },
                "nodes": [
                    {
                        "id": "owner:src/model.rs",
                        "kind": "owner",
                        "role": "path",
                        "value": "src/model.rs",
                        "path": "src/model.rs",
                        "ownerPath": "src/model.rs",
                    },
                    {
                        "id": "invariant:agent-r027",
                        "kind": "invariant-candidate",
                        "role": "claim",
                        "value": "semantic fields need named type",
                        "path": "src/model.rs",
                        "ownerPath": "src/model.rs",
                        "locator": "src/model.rs:42:42",
                        "startLine": 42,
                        "endLine": 42,
                    },
                    {
                        "id": "receipt:cargo-check",
                        "kind": "verification-receipt",
                        "role": "receipt",
                        "value": "cargo check passed",
                    },
                    {
                        "id": "snapshot:src-model",
                        "kind": "behavior-snapshot",
                        "role": "snapshot",
                        "value": "expect-test output changed",
                    },
                    {
                        "id": "readiness:project",
                        "kind": "determinism-readiness",
                        "role": "readiness",
                        "value": "needs injection",
                    },
                    {
                        "id": "review:packet",
                        "kind": "review-packet",
                        "role": "packet",
                        "value": "rust.review.packet",
                    },
                    {
                        "id": "action:run-receipt",
                        "kind": "review-action",
                        "role": "action",
                        "value": "run expect-test receipt",
                    },
                ],
                "edges": [
                    {
                        "source": "owner:src/model.rs",
                        "target": "invariant:agent-r027",
                        "relation": "supports-claim",
                    },
                    {
                        "source": "invariant:agent-r027",
                        "target": "receipt:cargo-check",
                        "relation": "verified-by",
                    },
                    {
                        "source": "invariant:agent-r027",
                        "target": "snapshot:src-model",
                        "relation": "observed-by",
                    },
                    {
                        "source": "invariant:agent-r027",
                        "target": "readiness:project",
                        "relation": "requires-evidence",
                    },
                    {
                        "source": "review:packet",
                        "target": "invariant:agent-r027",
                        "relation": "reviewed-by",
                    },
                    {
                        "source": "invariant:agent-r027",
                        "target": "action:run-receipt",
                        "relation": "suggests-action",
                    },
                ],
                "gaps": [
                    {
                        "gapId": "gap:receipt",
                        "ownerPath": "src/model.rs",
                        "summary": "missing expect-test receipt",
                        "severity": "warning",
                    }
                ],
            }
        ],
        "fields": {"next": "pipe JSON to asp graph render"},
    }
