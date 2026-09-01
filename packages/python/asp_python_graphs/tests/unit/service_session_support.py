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
        "seedIds": copy.deepcopy(packet["seedIds"]),
        "kindBudgets": copy.deepcopy(packet["kindBudgets"]),
        "windowMerge": copy.deepcopy(packet["windowMerge"]),
        "pathBudget": packet["pathBudget"],
        "pathMaxHops": packet["pathMaxHops"],
        "cache": copy.deepcopy(packet["cache"]),
        "queryClauses": copy.deepcopy(packet.get("queryClauses", [])),
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
        "generationToken": 1,
        "messageKind": kind,
        "payloadSchemaId": "agent.semantic-protocols.asp-python-graphs-test",
        "payload": {},
        **fields,
    }
