"""CLI tests for the ASP memory engine."""

from __future__ import annotations

import io
import json
import os
import shutil
import socket
import subprocess
import tempfile
import time
from pathlib import Path

from asp_memory_engine import Episode, EpisodeDraft, EpisodeStore, PlanMemoryContext, StoreConfig
from asp_memory_engine.cli import main as memory_engine_main

BINARY_RANK_PLANS_MAX_MS = 500.0
WORKER_RANK_PLANS_MAX_MS = 100.0


def test_cli_checkpoint_put_and_list_filters_by_session(
    tmp_path, capsys, monkeypatch
) -> None:
    state_path = tmp_path / "state.db"
    checkpoint_payload = {
        "sessionId": "session-a",
        "projectId": "repo",
        "planId": "plan-a",
        "title": "persist checkpoint from cli",
        "sourceLocator": "plans/current.org:9:9",
    }
    monkeypatch.setattr("sys.stdin", io.StringIO(json.dumps(checkpoint_payload)))

    put_status = memory_engine_main(
        [
            "checkpoint-put",
            "--state",
            str(state_path),
            "--embedding-dim",
            "8",
        ]
    )
    put_stdout = capsys.readouterr().out

    assert put_status == 0
    put_payload = json.loads(put_stdout)
    assert put_payload["ok"] is True
    assert put_payload["checkpoint"]["session_id"] == "session-a"

    list_status = memory_engine_main(
        [
            "checkpoint-list",
            "--state",
            str(state_path),
            "--embedding-dim",
            "8",
            "--project",
            "repo",
            "--session",
            "session-a",
        ]
    )
    list_stdout = capsys.readouterr().out

    assert list_status == 0
    list_payload = json.loads(list_stdout)
    assert [item["title"] for item in list_payload["checkpoints"]] == [
        "persist checkpoint from cli"
    ]


def test_cli_build_binary_creates_executable_rank_plans_artifact(
    tmp_path, capsys
) -> None:
    binary = tmp_path / "asp-memory-engine"

    status = memory_engine_main(["build-binary", "--output", str(binary)])

    stdout = capsys.readouterr().out
    shebang = binary.read_bytes().splitlines()[0]
    assert status == 0
    assert "[build-binary] engine=asp-memory-engine kind=zipapp" in stdout
    assert "compressed=false" in stdout
    assert os.access(binary, os.X_OK)
    assert shebang == b"#!/usr/bin/env -S python3 -S"
    assert b".venv" not in shebang

    state_path = tmp_path / "state.json"
    store = EpisodeStore(StoreConfig(path=str(state_path), embedding_dim=8))
    context = PlanMemoryContext(project_id="repo", plan_id="binary-plan")
    store.store(
        Episode.new(
            EpisodeDraft(
                id="binary-plan-episode",
                intent="binary memory engine performance",
                intent_embedding=store.encoder.encode(
                    "binary memory engine performance"
                ),
                experience="rank plans through the executable artifact",
                outcome="pending",
            ).with_plan_context(context, sharing="plan")
        )
    )
    store.save_state(state_path)
    payload = {
        "plans": [
            {
                "id": "binary-plan",
                "path": "flow/plans/agent-plan-binary-plan.org",
                "title": "Binary memory engine performance",
                "todo": "TODO",
                "mtime": 1.0,
                "properties": {
                    "CONTRACT_ORG": "agent.plan.v1",
                    "ID": "binary-plan",
                    "OBJECTIVE": "Binary memory engine performance",
                },
            }
        ]
    }

    runs = [
        _run_binary_rank_plans(binary, state_path, payload),
        _run_binary_rank_plans(binary, state_path, payload),
    ]
    fastest_ms, result = min(runs, key=lambda run: run[0])

    assert result.returncode == 0, result.stderr
    ranked = json.loads(result.stdout)
    assert ranked["plans"][0]["id"] == "binary-plan"
    assert fastest_ms < BINARY_RANK_PLANS_MAX_MS


def test_cli_binary_worker_ranks_warm_request_in_milliseconds(tmp_path) -> None:
    binary = tmp_path / "asp-memory-engine"
    status = memory_engine_main(["build-binary", "--output", str(binary)])
    assert status == 0
    state_path = tmp_path / "state.json"
    store = EpisodeStore(StoreConfig(path=str(state_path), embedding_dim=8))
    context = PlanMemoryContext(project_id="repo", plan_id="worker-plan")
    store.store(
        Episode.new(
            EpisodeDraft(
                id="worker-plan-episode",
                intent="worker memory engine performance",
                intent_embedding=store.encoder.encode(
                    "worker memory engine performance"
                ),
                experience="rank plans through the resident worker",
                outcome="pending",
            ).with_plan_context(context, sharing="plan")
        )
    )
    store.save_state(state_path)
    request = {
        "id": "rank-1",
        "command": "rank-plans",
        "state": str(state_path),
        "embeddingDim": 8,
        "project": "repo",
        "intent": "worker memory engine performance",
        "topK": 1,
        "payload": {
            "plans": [
                {
                    "id": "worker-plan",
                    "path": "flow/plans/agent-plan-worker-plan.org",
                    "title": "Worker memory engine performance",
                    "todo": "TODO",
                    "mtime": 1.0,
                    "properties": {
                        "CONTRACT_ORG": "agent.plan.v1",
                        "ID": "worker-plan",
                        "OBJECTIVE": "Worker memory engine performance",
                    },
                }
            ]
        },
    }
    worker = subprocess.Popen(
        [str(binary), "worker", "--state", str(state_path), "--embedding-dim", "8"],
        stdin=subprocess.PIPE,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )
    try:
        first = _send_worker_request(worker, request)
        assert first["plans"][0]["id"] == "worker-plan"
        request["id"] = "rank-2"
        start = time.perf_counter()
        second = _send_worker_request(worker, request)
        elapsed_ms = (time.perf_counter() - start) * 1000.0
    finally:
        worker.terminate()
        worker.wait(timeout=5)

    assert second["id"] == "rank-2"
    assert second["plans"][0]["id"] == "worker-plan"
    assert elapsed_ms < WORKER_RANK_PLANS_MAX_MS


def test_cli_binary_socket_worker_ranks_request(tmp_path) -> None:
    binary = tmp_path / "asp-memory-engine"
    status = memory_engine_main(["build-binary", "--output", str(binary)])
    assert status == 0
    state_path = tmp_path / "state.json"
    store = EpisodeStore(StoreConfig(path=str(state_path), embedding_dim=8))
    context = PlanMemoryContext(project_id="repo", plan_id="socket-worker-plan")
    store.store(
        Episode.new(
            EpisodeDraft(
                id="socket-worker-plan-episode",
                intent="socket worker memory engine performance",
                intent_embedding=store.encoder.encode(
                    "socket worker memory engine performance"
                ),
                experience="rank plans through the socket worker",
                outcome="pending",
            ).with_plan_context(context, sharing="plan")
        )
    )
    store.save_state(state_path)
    request = {
        "id": "socket-rank",
        "command": "rank-plans",
        "state": str(state_path),
        "embeddingDim": 8,
        "project": "repo",
        "intent": "socket worker memory engine performance",
        "topK": 1,
        "payload": {
            "plans": [
                {
                    "id": "socket-worker-plan",
                    "path": "candidate-socket-worker-plan",
                    "title": "Socket worker memory engine performance",
                    "todo": "TODO",
                    "mtime": 1.0,
                    "properties": {
                        "CONTRACT_ORG": "agent.plan.v1",
                        "ID": "socket-worker-plan",
                        "OBJECTIVE": "Socket worker memory engine performance",
                    },
                }
            ]
        },
    }
    socket_dir = Path(tempfile.mkdtemp(prefix="asp-mem-", dir="/tmp"))
    socket_path = socket_dir / "m.sock"
    worker = subprocess.Popen(
        [str(binary), "worker", "--socket", str(socket_path)],
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )
    try:
        _wait_for_socket(socket_path)
        with socket.socket(socket.AF_UNIX, socket.SOCK_STREAM) as client:
            client.connect(str(socket_path))
            client.sendall((json.dumps(request) + "\n").encode("utf-8"))
            response = _read_socket_line(client)
    finally:
        worker.terminate()
        worker.wait(timeout=5)
        shutil.rmtree(socket_dir, ignore_errors=True)

    ranked = json.loads(response)
    assert ranked["id"] == "socket-rank"
    assert ranked["worker"] == "resident-jsonl"
    assert ranked["plans"][0]["id"] == "socket-worker-plan"


def _run_binary_rank_plans(binary, state_path, payload):
    start = time.perf_counter()
    result = subprocess.run(
        [
            str(binary),
            "rank-plans",
            "--state",
            str(state_path),
            "--embedding-dim",
            "8",
            "--project",
            "repo",
        ],
        input=json.dumps(payload),
        text=True,
        capture_output=True,
        check=False,
    )
    return (time.perf_counter() - start) * 1000.0, result


def _send_worker_request(worker, request):
    assert worker.stdin is not None
    assert worker.stdout is not None
    worker.stdin.write(json.dumps(request) + "\n")
    worker.stdin.flush()
    line = worker.stdout.readline()
    assert line
    return json.loads(line)


def _wait_for_socket(socket_path) -> None:
    deadline = time.monotonic() + 5.0
    while time.monotonic() < deadline:
        if socket_path.exists():
            return
        time.sleep(0.01)
    raise AssertionError(f"socket did not appear: {socket_path}")


def _read_socket_line(client: socket.socket) -> str:
    chunks = []
    while True:
        chunk = client.recv(4096)
        assert chunk
        chunks.append(chunk)
        if b"\n" in chunk:
            return b"".join(chunks).split(b"\n", 1)[0].decode("utf-8")
