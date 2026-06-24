"""ASP memory engine plan-rank tests."""

from __future__ import annotations

import json

from asp_memory_engine import Episode, EpisodeDraft, EpisodeStore, PlanMemoryContext, StoreConfig
from asp_memory_engine import cli as memory_cli


def test_rank_plans_sorts_rust_owned_candidates_with_memory_score(
    tmp_path, capsys, monkeypatch
) -> None:
    state_path = tmp_path / "memory-state.json"
    store = EpisodeStore(StoreConfig(path=str(state_path), embedding_dim=8))
    context = PlanMemoryContext(project_id="repo", plan_id="tighten-asp-org-recall-flow")
    store.store(
        Episode.new(
            EpisodeDraft(
                id="recall-flow-episode",
                intent="tighten org recall flow",
                intent_embedding=store.encoder.encode("tighten org recall flow"),
                experience="continue the recall implementation",
                outcome="pending",
            ).with_plan_context(context, sharing="project")
        )
    )
    store.save_state(state_path)
    monkeypatch.setattr(
        "sys.stdin",
        _Stdin(
            {
                "plans": [
                    {
                        "id": "unrelated-plan",
                        "path": "flow/plans/agent-plan-unrelated.org",
                        "title": "Unrelated plan",
                        "todo": "TODO",
                        "mtime": 1.0,
                        "properties": {
                            "CONTRACT_ORG": "agent.plan.v1",
                            "ID": "unrelated-plan",
                            "OBJECTIVE": "Unrelated plan",
                        },
                    },
                    {
                        "id": "tighten-asp-org-recall-flow",
                        "path": "flow/plans/agent-plan-tighten-asp-org-recall-flow.org",
                        "title": "Tighten ASP org recall flow [1/8] [12%]",
                        "todo": "TODO",
                        "mtime": 1.0,
                        "properties": {
                            "CONTRACT_ORG": "agent.plan.v1",
                            "ID": "tighten-asp-org-recall-flow",
                            "OBJECTIVE": "Tighten ASP org recall flow",
                            "NEXT_ACTION": "continue the recall implementation",
                        },
                    },
                ]
            }
        ),
    )

    status = memory_cli.main(
        [
            "rank-plans",
            "--state",
            str(state_path),
            "--embedding-dim",
            "8",
            "--project",
            "repo",
            "--intent",
            "tighten org recall flow",
        ]
    )

    stdout = capsys.readouterr().out
    payload = json.loads(stdout)
    assert status == 0
    assert payload["engine"] == "asp-memory-engine"
    assert payload["ranker"] == "memory-engine"
    assert payload["plans"][0]["id"] == "tighten-asp-org-recall-flow"
    assert payload["plans"][0]["memoryScore"] > 0.0
    assert payload["plans"][0]["textScore"] > 0.0
    scores = {plan["id"]: plan for plan in payload["plans"]}
    assert (
        scores["tighten-asp-org-recall-flow"]["memoryScore"]
        > scores["unrelated-plan"]["memoryScore"]
    )


def test_rank_plans_keeps_exact_plan_memory_above_shared_memory(
    tmp_path, capsys, monkeypatch
) -> None:
    state_path = tmp_path / "memory-state.json"
    store = EpisodeStore(StoreConfig(path=str(state_path), embedding_dim=8))
    session_context = PlanMemoryContext(project_id="repo", session_id="session-a")
    target_context = PlanMemoryContext(
        project_id="repo",
        session_id="session-a",
        plan_id="memory-engine-hot-path",
    )
    for index in range(30):
        store.store(
            Episode.new(
                EpisodeDraft(
                    id=f"shared-episode-{index}",
                    intent="stabilize memory engine recall flow",
                    intent_embedding=store.encoder.encode(
                        "stabilize memory engine recall flow"
                    ),
                    experience="broad session memory",
                    outcome="pending",
                ).with_plan_context(session_context, sharing="session")
            )
        )
    store.store(
        Episode.new(
            EpisodeDraft(
                id="exact-plan-episode",
                intent="stabilize memory engine recall flow exact plan",
                intent_embedding=store.encoder.encode(
                    "stabilize memory engine recall flow exact plan"
                ),
                experience="target plan memory",
                outcome="pending",
            ).with_plan_context(target_context, sharing="plan")
        )
    )
    store.save_state(state_path)
    monkeypatch.setattr(
        "sys.stdin",
        _Stdin(
            {
                "plans": [
                    {
                        "id": "unrelated-plan",
                        "path": "flow/plans/agent-plan-unrelated.org",
                        "title": "Unrelated plan",
                        "todo": "TODO",
                        "mtime": 1.0,
                        "properties": {
                            "CONTRACT_ORG": "agent.plan.v1",
                            "ID": "unrelated-plan",
                            "PLAN_SESSION": "session-a",
                            "OBJECTIVE": "stabilize memory engine recall flow",
                        },
                    },
                    {
                        "id": "memory-engine-hot-path",
                        "path": "flow/plans/agent-plan-memory-engine-hot-path.org",
                        "title": "Memory engine hot path [1/8] [12%]",
                        "todo": "TODO",
                        "mtime": 1.0,
                        "properties": {
                            "CONTRACT_ORG": "agent.plan.v1",
                            "ID": "memory-engine-hot-path",
                            "PLAN_SESSION": "session-a",
                            "OBJECTIVE": "stabilize memory engine recall flow",
                        },
                    },
                ]
            }
        ),
    )

    status = memory_cli.main(
        [
            "rank-plans",
            "--state",
            str(state_path),
            "--embedding-dim",
            "8",
            "--project",
            "repo",
            "--intent",
            "stabilize memory engine recall flow",
        ]
    )

    stdout = capsys.readouterr().out
    payload = json.loads(stdout)
    assert status == 0
    scores = {plan["id"]: plan for plan in payload["plans"]}
    assert (
        scores["memory-engine-hot-path"]["memoryScore"]
        > scores["unrelated-plan"]["memoryScore"]
    )


class _Stdin:
    def __init__(self, payload: object) -> None:
        self._payload = json.dumps(payload)

    def read(self) -> str:
        return self._payload
