from __future__ import annotations

import copy
import io
import json
import subprocess
import sys
from pathlib import Path

from asp_graph_turbo.resident_server import serve_json_lines


ROOT = Path(__file__).resolve().parents[5]
DIGEST_A = f"blake3-256:{'a' * 64}"
DIGEST_B = f"blake3-256:{'b' * 64}"


def rank_payload() -> dict[str, object]:
    packet = json.loads(
        (ROOT / "sandtables/fixtures/asp/graph-turbo-owner-query.json").read_text(
            encoding="utf-8"
        )
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


def message(kind: str, request_id: int, **fields: object) -> dict[str, object]:
    return {
        "schemaId": "agent.semantic-protocols.graph-turbo-resident-server",
        "schemaVersion": "1",
        "messageKind": kind,
        "requestId": request_id,
        **fields,
    }


def hello() -> dict[str, object]:
    return message(
        "hello",
        1,
        runtimeArtifactDigest=DIGEST_A,
        executionCommandDigest=DIGEST_B,
    )


def rank(request_id: int, generation: str = DIGEST_A) -> dict[str, object]:
    return message(
        "rank",
        request_id,
        workspaceIdentity="workspace-a",
        generationDigest=generation,
        pageRoots={"owners": DIGEST_B},
        terms=["runtime"],
        profile="owner-query",
        budget=8,
        rankPayload={key: value for key, value in rank_payload().items() if key != "graph"},
    )


def load_generation(request_id: int, generation: str = DIGEST_A) -> dict[str, object]:
    return message(
        "load-generation",
        request_id,
        workspaceIdentity="workspace-a",
        generationDigest=generation,
        pageRoots={"owners": DIGEST_B},
        generationPayload={"graph": rank_payload()["graph"]},
    )


def test_one_process_reuses_a_loaded_generation() -> None:
    messages = [hello(), load_generation(2), rank(3), rank(4), message("shutdown", 5)]
    writer = io.StringIO()
    assert serve_json_lines(io.StringIO("".join(json.dumps(item) + "\n" for item in messages)), writer) == 0
    receipts = [json.loads(line) for line in writer.getvalue().splitlines()]
    assert [receipt["state"] for receipt in receipts] == ["ready", "ready", "completed", "completed", "cancelled"]
    assert receipts[1]["generationLoads"] == 1
    assert receipts[2]["generationLoads"] == 0
    assert receipts[3]["generationLoads"] == 0
    assert receipts[3]["processSpawns"] == 0
    assert receipts[3]["generationDigest"] == DIGEST_A


def test_rank_rejects_generation_that_was_not_loaded_by_runtime() -> None:
    messages = [hello(), rank(2), message("shutdown", 3)]
    writer = io.StringIO()
    assert serve_json_lines(io.StringIO("".join(json.dumps(item) + "\n" for item in messages)), writer) == 0
    receipts = [json.loads(line) for line in writer.getvalue().splitlines()]
    assert receipts[1]["state"] == "unavailable"
    assert receipts[1]["reasonKind"] == "generation-not-loaded"


def test_rank_requires_runtime_v1_generation_identity() -> None:
    messages = [hello(), message("rank", 2, workspaceIdentity="workspace-a"), message("shutdown", 3)]
    writer = io.StringIO()
    assert serve_json_lines(io.StringIO("".join(json.dumps(item) + "\n" for item in messages)), writer) == 0
    receipts = [json.loads(line) for line in writer.getvalue().splitlines()]
    assert receipts[1]["state"] == "unavailable"
    assert receipts[1]["reasonKind"] == "invalid-identity"


def test_json_line_process_uses_only_graph_turbo_ipc_v1() -> None:
    completed = subprocess.run(
        [sys.executable, "-m", "asp_graph_turbo.resident_server"],
        input=json.dumps(hello()) + "\n" + json.dumps(message("shutdown", 2)) + "\n",
        text=True,
        capture_output=True,
        check=False,
        timeout=30,
    )
    assert completed.returncode == 0, completed.stderr
    receipts = [json.loads(line) for line in completed.stdout.splitlines()]
    assert [receipt["schemaId"] for receipt in receipts] == [
        "agent.semantic-protocols.graph-turbo-resident-server",
        "agent.semantic-protocols.graph-turbo-resident-server",
    ]
    assert [receipt["state"] for receipt in receipts] == ["ready", "cancelled"]
