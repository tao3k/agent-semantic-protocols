# SPDX-FileCopyrightText: 2026 tao3k team and Contributors
#
# SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

"""Shared protocol fixtures for ASP Python Graphs service-session tests."""

from __future__ import annotations

import copy
import json
from pathlib import Path


ROOT = Path(__file__).resolve().parents[5]
DIGEST_A = f"blake3-256:{'a' * 64}"
DIGEST_B = f"blake3-256:{'b' * 64}"


def rank_payload() -> dict[str, object]:
    packet = json.loads(
        (ROOT / "sandtables/fixtures/asp/graph-turbo-owner-query.json").read_text()
    )
    return {
        "graph": copy.deepcopy(packet["graph"]),
        "entryNodeIds": copy.deepcopy(packet["entryNodeIds"]),
        "kindBudgets": copy.deepcopy(packet["kindBudgets"]),
        "windowMerge": copy.deepcopy(packet["windowMerge"]),
        "pathBudget": packet["pathBudget"],
        "pathMaxHops": packet["pathMaxHops"],
        "cache": copy.deepcopy(packet["cache"]),
        "queryClauses": copy.deepcopy(packet.get("queryClauses", [])),
    }


def generation_payload() -> dict[str, object]:
    return {
        "schemaId": "agent.semantic-protocols.search-generation-graph-request",
        "schemaVersion": "1",
        "identity": {
            "projectId": "project-test",
            "workspaceId": "workspace-test",
            "sourceRootDigest": DIGEST_A,
            "providerDigest": DIGEST_B,
            "schemaDigest": DIGEST_A,
            "generationCandidateDigest": DIGEST_B,
        },
        "sourceSnapshot": {
            "schemaId": "asp.source-snapshot.v1",
            "algorithm": "blake3-merkle-v1",
            "rootDigest": DIGEST_A.removeprefix("blake3-256:"),
            "sourceKind": "filesystem",
            "leafCount": 2,
            "providerDigest": DIGEST_B.removeprefix("blake3-256:"),
        },
        "workspaceGeneration": {
            "rootDigest": DIGEST_A.removeprefix("blake3-256:"),
            "rootDepth": 1,
            "leafCount": 2,
            "ownerCount": 2,
        },
        "ownerPaths": ["src/a.py", "src/b.py"],
        "relations": [
            {
                "from": {"kind": "owner", "id": "src/a.py"},
                "kind": "imports",
                "to": {"kind": "owner", "id": "src/b.py"},
            }
        ],
    }


def resident_evaluation_payload() -> dict[str, object]:
    return {
        "schemaId": "agent.semantic-protocols.semantic-graph-resident-evaluation-request",
        "schemaVersion": "1",
        "protocolId": "agent.semantic-protocols.search",
        "protocolVersion": "1",
        "packetKind": "resident-graph-evaluation-request",
        "languageId": "python",
        "surface": "search-playbook",
        "queryTerms": [],
        "queryClauses": ["imports|depends-on"],
        "profile": "dependency",
        "entryNodeIds": ["owner:src/a.py"],
        "candidateNodeIds": ["owner:src/a.py"],
        "budget": {"maxDepth": 4, "maxNodes": 64, "maxEdges": 128, "maxResults": 8},
    }


def message(kind: str, request_id: str, **fields: object) -> dict[str, object]:
    sequence_by_request = {
        "hello-1": 1,
        "open-1": 2,
        "open-2": 3,
        "open-3": 4,
        "evaluate-1": 3,
        "evaluate-stale": 2,
        "cancel-1": 3,
        "evaluate-cancelled": 4,
        "eval-1": 4,
    }
    return {
        "schemaId": "agent.semantic-protocols.asp-python-graphs-session",
        "schemaVersion": "1",
        "sessionId": "session-a",
        "serviceEpoch": "epoch-a",
        "requestId": request_id,
        "sequence": sequence_by_request.get(request_id, 1),
        "messageKind": kind,
        "payloadSchemaId": "agent.semantic-protocols.asp-python-graphs-test",
        "payload": {},
        **fields,
    }
