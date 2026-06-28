"""Checkpoint-backed plan ranking tests for query-selected Org tasks."""

from __future__ import annotations

import json

from asp_memory_engine import EpisodeStore, StoreConfig
from asp_memory_engine import cli as memory_cli
from asp_memory_engine.checkpoint import Checkpoint


def test_rank_plans_uses_query_selected_task_checkpoint_fields(
    tmp_path, capsys, monkeypatch
) -> None:
    state_path = tmp_path / "memory-state.json"
    store = EpisodeStore(StoreConfig(path=str(state_path), embedding_dim=8))
    store.store_checkpoint(_checkpoint())
    store.save_state(state_path)
    monkeypatch.setattr("sys.stdin", _Stdin(_rank_payload()))

    status = memory_cli.main(
        [
            "rank-plans",
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

    payload = json.loads(capsys.readouterr().out)
    assert status == 0
    assert payload["plans"][0]["id"] == "checkpoint-sync-for-codex-session-recall"
    scores = {plan["id"]: plan for plan in payload["plans"]}
    assert (
        scores["checkpoint-sync-for-codex-session-recall"]["memoryScore"]
        > scores["agent-session-env-detection-without-generic-fallback"]["memoryScore"]
    )


def _checkpoint() -> Checkpoint:
    return Checkpoint(
        id="checkpoint-1",
        session_id="session-a",
        title="Work checkpoints are current",
        status="open",
        kind="checklist",
        project_id="repo",
        plan_id="checkpoint-sync-for-codex-session-recall",
        source_locator="flow/plans/checkpoint-sync.org:42-42",
        resume_command=(
            "asp org query --selector flow/plans/checkpoint-sync.org:42-42 --content"
        ),
        metadata={
            "planPath": "flow/plans/checkpoint-sync.org",
            "taskStatus": "[ ]",
            "taskSection": "Next",
            "taskSourceLine": "42",
        },
    )


def _rank_payload() -> dict[str, object]:
    return {
        "plans": [
            _plan_candidate(
                plan_id="agent-session-env-detection-without-generic-fallback",
                path="flow/plans/session-env.org",
                task_title="Session env detection stays exact",
                task_line=12,
            ),
            _plan_candidate(
                plan_id="checkpoint-sync-for-codex-session-recall",
                path="flow/plans/checkpoint-sync.org",
                task_title="Work checkpoints are current",
                task_line=42,
            ),
        ]
    }


def _plan_candidate(
    *,
    plan_id: str,
    path: str,
    task_title: str,
    task_line: int,
) -> dict[str, object]:
    return {
        "id": plan_id,
        "path": path,
        "title": "Agent session plan",
        "todo": "TODO",
        "mtime": 1.0,
        "properties": {
            "CONTRACT_ORG": "agent.plan.v1",
            "ID": plan_id,
            "SESSION_ID": "session-a",
        },
        "taskCandidates": [
            {
                "kind": "checklist",
                "status": "[ ]",
                "title": task_title,
                "section": "Next",
                "sourceLine": task_line,
            }
        ],
    }


class _Stdin:
    def __init__(self, payload: object) -> None:
        self._payload = json.dumps(payload)

    def read(self) -> str:
        return self._payload
