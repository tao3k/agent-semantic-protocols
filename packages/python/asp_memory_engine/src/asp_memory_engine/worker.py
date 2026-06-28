"""Resident JSONL worker for ASP memory ranking."""

from __future__ import annotations

import json
import os
import socket
import sys
from dataclasses import dataclass
from pathlib import Path
from typing import Any, TextIO

from .checkpoint import Checkpoint
from .plan_rank import rank_plan_candidates
from .store import EpisodeStore, StoreConfig


@dataclass(frozen=True)
class _StoreKey:
    path: str
    embedding_dim: int
    mtime_ns: int


class _StoreCache:
    def __init__(self) -> None:
        self._stores: dict[_StoreKey, EpisodeStore] = {}

    def load(self, path: str, embedding_dim: int) -> EpisodeStore:
        state_path = Path(path)
        mtime_ns = state_path.stat().st_mtime_ns if state_path.exists() else 0
        key = _StoreKey(str(state_path), embedding_dim, mtime_ns)
        store = self._stores.get(key)
        if store is not None:
            return store
        store = EpisodeStore(
            StoreConfig(path=str(state_path), embedding_dim=embedding_dim)
        )
        store.load_state(str(state_path))
        self._stores[key] = store
        return store


def serve_memory_worker(
    *,
    input_stream: TextIO = sys.stdin,
    output_stream: TextIO = sys.stdout,
    default_state: str = StoreConfig().path,
    default_embedding_dim: int = 384,
) -> int:
    cache = _StoreCache()
    for line in input_stream:
        if not line.strip():
            continue
        try:
            response = _handle_request(
                json.loads(line),
                cache=cache,
                default_state=default_state,
                default_embedding_dim=default_embedding_dim,
            )
        except Exception as error:  # pragma: no cover - defensive worker boundary
            response = {
                "schemaId": "agent.semantic-protocols.memory-worker-error",
                "schemaVersion": "1",
                "ok": False,
                "error": str(error),
            }
        output_stream.write(json.dumps(response, sort_keys=True) + "\n")
        output_stream.flush()
    return 0


def serve_memory_worker_socket(
    socket_path: str,
    *,
    default_state: str = StoreConfig().path,
    default_embedding_dim: int = 384,
) -> int:
    if os.path.exists(socket_path):
        os.unlink(socket_path)
    server = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
    try:
        server.bind(socket_path)
        server.listen()
        while True:
            conn, _ = server.accept()
            with conn:
                reader = conn.makefile("r", encoding="utf-8")
                writer = conn.makefile("w", encoding="utf-8")
                serve_memory_worker(
                    input_stream=reader,
                    output_stream=writer,
                    default_state=default_state,
                    default_embedding_dim=default_embedding_dim,
                )
    finally:
        server.close()
        if os.path.exists(socket_path):
            os.unlink(socket_path)


def _handle_request(
    request: dict[str, Any],
    *,
    cache: _StoreCache,
    default_state: str,
    default_embedding_dim: int,
) -> dict[str, Any]:
    command = request.get("command", "rank-plans")
    if command not in {"rank-plans", "checkpoint-put", "checkpoint-list"}:
        return {
            "schemaId": "agent.semantic-protocols.memory-worker-error",
            "schemaVersion": "1",
            "ok": False,
            "error": f"unsupported command: {command}",
        }
    embedding_dim = int(request.get("embeddingDim", default_embedding_dim))
    state = str(request.get("state", default_state))
    store = cache.load(state, embedding_dim)
    if command == "checkpoint-put":
        checkpoint = Checkpoint.from_mapping(request.get("payload", {}))
        store.store_checkpoint(checkpoint)
        store.save_state(state)
        response = {
            "schemaId": "agent.semantic-protocols.memory-checkpoint-receipt",
            "schemaVersion": "1",
            "ok": True,
            "checkpoint": checkpoint.to_mapping(),
            "worker": "resident-jsonl",
        }
        request_id = request.get("id")
        if request_id is not None:
            response["id"] = request_id
        return response
    if command == "checkpoint-list":
        top_k = request.get("topK")
        checkpoints = store.list_checkpoints(
            project_id=request.get("project"),
            session_id=request.get("session"),
            plan_id=request.get("plan"),
            branch_id=request.get("branch"),
            status=request.get("status"),
            top_k=int(top_k) if top_k is not None else None,
        )
        response = {
            "schemaId": "agent.semantic-protocols.memory-checkpoint-list",
            "schemaVersion": "1",
            "checkpoints": [checkpoint.to_mapping() for checkpoint in checkpoints],
            "worker": "resident-jsonl",
        }
        request_id = request.get("id")
        if request_id is not None:
            response["id"] = request_id
        return response
    payload = request.get("payload", {"plans": request.get("plans", [])})
    response = rank_plan_candidates(
        payload,
        store=store,
        project=str(request.get("project", "_global_project")),
        session=request.get("session"),
        branch=request.get("branch"),
        top_k=int(request.get("topK", 5)),
    )
    request_id = request.get("id")
    if request_id is not None:
        response["id"] = request_id
    response["worker"] = "resident-jsonl"
    return response
