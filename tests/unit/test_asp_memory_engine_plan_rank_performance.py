"""ASP memory engine plan-rank hot-path tests."""

from __future__ import annotations

from asp_memory_engine import EpisodeStore, StoreConfig
from asp_memory_engine import plan_rank as plan_rank_module


def test_rank_plans_tokenizes_intent_once_for_large_candidate_set(
    tmp_path, monkeypatch
) -> None:
    state_path = tmp_path / "memory-state.json"
    store = EpisodeStore(StoreConfig(path=str(state_path), embedding_dim=8))
    intent = "optimize memory engine plan recall worker ranking"
    original_tokens = plan_rank_module.tokens
    intent_token_calls = 0

    def counting_tokens(value: object) -> set[str]:
        nonlocal intent_token_calls
        if value == intent:
            intent_token_calls += 1
        return original_tokens(value)

    monkeypatch.setattr(plan_rank_module, "tokens", counting_tokens)
    payload = plan_rank_module.rank_plan_candidates(
        {"plans": [_noise_plan(index) for index in range(1_000)] + [_target_plan()]},
        store=store,
        intent=intent,
        project="repo",
        top_k=1,
    )

    assert payload["plans"][0]["id"] == "target-plan"
    assert intent_token_calls == 1


def _noise_plan(index: int) -> dict[str, object]:
    return {
        "id": f"noise-plan-{index}",
        "path": f"flow/plans/noise-plan-{index}.org",
        "title": f"Noise plan {index}",
        "todo": "TODO",
        "mtime": 1.0,
        "properties": {
            "CONTRACT_ORG": "agent.plan.v1",
            "ID": f"noise-plan-{index}",
            "OBJECTIVE": "unrelated maintenance task",
        },
    }


def _target_plan() -> dict[str, object]:
    return {
        "id": "target-plan",
        "path": "flow/plans/target-plan.org",
        "title": "Memory engine plan recall worker ranking",
        "todo": "TODO",
        "mtime": 1.0,
        "properties": {
            "CONTRACT_ORG": "agent.plan.v1",
            "ID": "target-plan",
            "OBJECTIVE": "optimize memory engine plan recall worker ranking",
        },
    }
