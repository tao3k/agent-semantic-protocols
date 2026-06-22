"""CLI tests for the ASP memory engine."""

from __future__ import annotations

from asp_memory_engine import Episode, EpisodeDraft, EpisodeStore, PlanMemoryContext, StoreConfig
from asp_memory_engine.cli import main as memory_engine_main


def test_cli_recall_plan_filters_by_session_context(tmp_path, capsys) -> None:
    state_path = tmp_path / "state.json"
    store = EpisodeStore(StoreConfig(path=str(state_path), embedding_dim=8))
    visible_context = PlanMemoryContext(
        project_id="repo",
        session_id="session-a",
        plan_id="plan-a",
        branch_id="main",
    )
    hidden_context = PlanMemoryContext(
        project_id="repo",
        session_id="session-b",
        plan_id="plan-b",
        branch_id="main",
    )
    store.store(
        Episode.new(
            EpisodeDraft(
                id="visible-task",
                intent="current unfinished org task",
                intent_embedding=store.encoder.encode("current unfinished org task"),
                experience="continue the visible task",
                outcome="pending",
            ).with_plan_context(visible_context, sharing="session")
        )
    )
    store.store(
        Episode.new(
            EpisodeDraft(
                id="hidden-task",
                intent="current unfinished org task",
                intent_embedding=store.encoder.encode("current unfinished org task"),
                experience="do not show this",
                outcome="pending",
            ).with_plan_context(hidden_context, sharing="session")
        )
    )
    store.save_state(state_path)

    status = memory_engine_main(
        [
            "recall-plan",
            "--state",
            str(state_path),
            "--embedding-dim",
            "8",
            "--intent",
            "unfinished org task",
            "--project",
            "repo",
            "--session",
            "session-a",
            "--plan",
            "plan-a",
            "--branch",
            "main",
        ]
    )

    stdout = capsys.readouterr().out
    assert status == 0
    assert "[recall-plan] engine=asp-memory-engine" in stdout
    assert "visible-task" in stdout
    assert "hidden-task" not in stdout
