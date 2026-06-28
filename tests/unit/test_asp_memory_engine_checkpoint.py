"""Checkpoint persistence behavior for the ASP memory engine."""

from __future__ import annotations

from asp_memory_engine.backend import LocalMemoryStateStore
from asp_memory_engine.checkpoint import Checkpoint
from asp_memory_engine.store import EpisodeStore, StoreConfig
from asp_memory_engine.worker import _handle_request, _StoreCache


def test_json_backend_round_trips_checkpoint_snapshot(tmp_path) -> None:
    store = EpisodeStore(StoreConfig(path=str(tmp_path / "state.json"), embedding_dim=8))
    store.store_checkpoint(
        Checkpoint.from_mapping(
            {
                "sessionId": "session-a",
                "projectId": "repo",
                "planId": "plan-a",
                "branchId": "feature-x",
                "title": "resume durable checkpoint",
                "status": "open",
                "sourceLocator": "plans/current.org:12:12",
                "resumeCommand": "asp org query --selector plans/current.org:12:12",
            }
        )
    )
    LocalMemoryStateStore(tmp_path / "state.json").save_snapshot(store.snapshot())

    loaded = LocalMemoryStateStore(tmp_path / "state.json").load_snapshot()

    assert loaded.checkpoints[0].session_id == "session-a"
    assert loaded.checkpoints[0].project_id == "repo"
    assert loaded.checkpoints[0].plan_id == "plan-a"
    assert loaded.checkpoints[0].branch_id == "feature-x"
    assert loaded.checkpoints[0].title == "resume durable checkpoint"
    assert loaded.checkpoints[0].source_locator == "plans/current.org:12:12"


def test_sqlite_backend_round_trips_checkpoint_snapshot(tmp_path) -> None:
    state_path = tmp_path / "state.sqlite"
    store = EpisodeStore(StoreConfig(path=str(state_path), embedding_dim=8))
    store.store_checkpoint(
        Checkpoint.from_mapping(
            {
                "session_id": "session-a",
                "project_id": "repo",
                "plan_id": "plan-a",
                "title": "sqlite durable checkpoint",
            }
        )
    )
    store.save_state(state_path)

    loaded = EpisodeStore(StoreConfig(path=str(state_path), embedding_dim=8))
    loaded.load_state(state_path)
    checkpoints = loaded.list_checkpoints(session_id="session-a", project_id="repo")

    assert len(checkpoints) == 1
    assert checkpoints[0].title == "sqlite durable checkpoint"


def test_worker_checkpoint_put_and_list_round_trip(tmp_path) -> None:
    state_path = str(tmp_path / "worker-state.sqlite")
    cache = _StoreCache()

    put_response = _handle_request(
        {
            "command": "checkpoint-put",
            "state": state_path,
            "embeddingDim": 8,
            "payload": {
                "sessionId": "session-worker",
                "projectId": "repo",
                "title": "worker durable checkpoint",
            },
        },
        cache=cache,
        default_state=state_path,
        default_embedding_dim=8,
    )

    assert put_response["ok"] is True

    list_response = _handle_request(
        {
            "command": "checkpoint-list",
            "state": state_path,
            "embeddingDim": 8,
            "project": "repo",
            "session": "session-worker",
        },
        cache=cache,
        default_state=state_path,
        default_embedding_dim=8,
    )

    assert [item["title"] for item in list_response["checkpoints"]] == [
        "worker durable checkpoint"
    ]
