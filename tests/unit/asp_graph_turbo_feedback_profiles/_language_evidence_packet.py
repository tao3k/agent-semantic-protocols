"""Language-neutral evidence-quality graph-turbo fixtures."""

from __future__ import annotations


def language_evidence_graph_turbo_request(
    *,
    language_id: str,
    provider_id: str,
    namespace: str,
    owner_path: str,
) -> dict[str, object]:
    prefix = language_id
    return {
        "schemaId": "agent.semantic-protocols.semantic-graph-turbo-request",
        "schemaVersion": "1",
        "protocolId": "agent.semantic-protocols.semantic-language",
        "protocolVersion": "1",
        "packetKind": "graph-turbo-request",
        "requestId": f"{prefix}.evidence.analysis.test",
        "surface": "evidence-analyze",
        "queryTerms": [f"{language_id} evidence quality"],
        "profile": "evidence-quality",
        "algorithm": "typed-ppr-diverse",
        "seedIds": [f"{prefix}:owner"],
        "budget": 8,
        "producer": {
            "languageId": language_id,
            "providerId": provider_id,
            "namespace": namespace,
        },
        "project": {"root": ".", "package": None, "fields": {}},
        "summary": {
            "graphs": 1,
            "nodes": 3,
            "edges": 2,
            "owners": 1,
            "claims": 1,
            "staleItems": 0,
            "gaps": 1,
        },
        "graphs": [
            {
                "graphId": f"{prefix}.evidence.graph.test",
                "summary": {
                    "nodes": 3,
                    "edges": 2,
                    "owners": 1,
                    "claims": 1,
                    "staleItems": 0,
                    "gaps": 1,
                },
                "nodes": [
                    {
                        "id": f"{prefix}:owner",
                        "kind": "owner",
                        "role": "path",
                        "value": owner_path,
                        "path": owner_path,
                        "ownerPath": owner_path,
                    },
                    {
                        "id": f"{prefix}:invariant",
                        "kind": "invariant-candidate",
                        "role": "claim",
                        "value": f"{language_id} API behavior needs receipt evidence",
                        "path": owner_path,
                        "ownerPath": owner_path,
                        "locator": f"{owner_path}:10:10",
                        "startLine": 10,
                        "endLine": 10,
                    },
                    {
                        "id": f"{prefix}:receipt",
                        "kind": "verification-receipt",
                        "role": "receipt",
                        "value": f"{language_id} unit gate passed",
                    },
                ],
                "edges": [
                    {
                        "source": f"{prefix}:owner",
                        "target": f"{prefix}:invariant",
                        "relation": "supports-claim",
                    },
                    {
                        "source": f"{prefix}:invariant",
                        "target": f"{prefix}:receipt",
                        "relation": "verified-by",
                    },
                ],
                "gaps": [
                    {
                        "gapId": f"{prefix}:gap:receipt",
                        "ownerPath": owner_path,
                        "summary": f"{language_id} integration receipt is missing",
                        "severity": "warning",
                    }
                ],
            }
        ],
        "fields": {"next": "pipe JSON to asp graph render"},
    }
