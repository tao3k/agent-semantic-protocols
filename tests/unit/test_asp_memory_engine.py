"""Tests for the ASP memory engine adaptation."""

from __future__ import annotations

from asp_memory_engine import (
    Episode,
    EpisodeDraft,
    EpisodeStore,
    InferredMemoryObjectKind,
    IntentEncoder,
    LocalMemoryStateStore,
    MemoryGateDecision,
    MemoryGatePolicy,
    MemoryUtilityLedger,
    PlanMemoryContext,
    PlanRecallComputation,
    RecallFeedbackOutcome,
    RecallPlanTuning,
    StoreConfig,
    TwoPhaseSearch,
    apply_feedback_to_plan_tuning,
    infer_memory_object_from_property,
    update_feedback_bias,
)
from asp_memory_engine.graph_turbo_memory import read_memory_projection


def test_two_phase_search_reranks_with_q_value() -> None:
    encoder = IntentEncoder(embedding_dim=32)
    first = Episode.new(
        EpisodeDraft(
            id="ep-1",
            intent="org contract memory recall",
            intent_embedding=encoder.encode("org contract memory recall"),
            experience="contract record",
            outcome="success",
        )
    )
    second = Episode.new(
        EpisodeDraft(
            id="ep-2",
            intent="unrelated cache path",
            intent_embedding=encoder.encode("unrelated cache path"),
            experience="cache record",
            outcome="success",
        )
    )
    store = EpisodeStore(StoreConfig(embedding_dim=32))
    store.store(first)
    store.store(second)
    store.q_table.set_q("ep-1", 0.9)
    store.q_table.set_q("ep-2", 0.1)

    result = TwoPhaseSearch(store.q_table, encoder).search(
        store.episodes,
        "memory contract recall",
        k1=2,
        k2=1,
        lambda_=0.4,
    )

    assert result[0][0].id == "ep-1"


def test_feedback_tuning_expands_after_failure_bias() -> None:
    bias = update_feedback_bias(-0.5, RecallFeedbackOutcome.FAILURE)
    plan = apply_feedback_to_plan_tuning(
        RecallPlanTuning(
            k1=9,
            k2=3,
            lambda_=0.3,
            min_score=0.2,
            max_context_chars=800,
        ),
        bias,
    )

    assert plan.k1 > 9
    assert plan.k2 > 3
    assert plan.lambda_ < 0.3


def test_inference_classifies_org_memory_properties() -> None:
    inferred = infer_memory_object_from_property("EVIDENCE_REF", "docs/plan.org#proof")

    assert inferred is not None
    assert inferred.kind is InferredMemoryObjectKind.EVIDENCE


def test_gate_promotes_high_utility_episode() -> None:
    episode = Episode.new(
        EpisodeDraft(
            id="ep-promote",
            intent="stable preference",
            intent_embedding=(1.0,),
            experience="keep this",
            outcome="success",
        )
    )
    for _ in range(4):
        episode.mark_success()
    episode.q_value = 0.95
    ledger = MemoryUtilityLedger.from_episode(episode, 0.95, 0.9, 0.9)

    assert MemoryGatePolicy().decide(ledger) is MemoryGateDecision.PROMOTE


def test_local_backend_round_trips_snapshot(tmp_path) -> None:
    store = EpisodeStore(StoreConfig(path=str(tmp_path / "state.json"), embedding_dim=8))
    episode = Episode.new(
        EpisodeDraft(
            id="ep-state",
            intent="persist memory",
            intent_embedding=store.encoder.encode("persist memory"),
            experience="stored",
            outcome="success",
            project_id="repo",
            session_id="session-a",
            plan_id="plan-a",
            branch_id="feature-x",
            plan_sharing="branch",
        )
    )
    store.store(episode)
    LocalMemoryStateStore(tmp_path / "state.json").save_snapshot(store.snapshot())

    loaded = LocalMemoryStateStore(tmp_path / "state.json").load_snapshot()

    assert loaded.episodes[0].id == "ep-state"
    assert loaded.episodes[0].project_id == "repo"
    assert loaded.episodes[0].session_id == "session-a"
    assert loaded.episodes[0].plan_id == "plan-a"
    assert loaded.episodes[0].branch_id == "feature-x"
    assert loaded.episodes[0].plan_sharing == "branch"
    assert loaded.q_values["ep-state"] == 0.5


def test_plan_context_recall_respects_session_branch_and_sharing() -> None:
    store = EpisodeStore(StoreConfig(embedding_dim=8))
    context = PlanMemoryContext(
        project_id="repo",
        session_id="session-a",
        plan_id="plan-a",
        branch_id="feature-x",
    )
    visible_specs = [
        ("isolated-plan-a", context, "isolated"),
        ("session-shared", context, "session"),
        ("branch-shared", context, "branch"),
        ("project-shared", context, "project"),
    ]
    hidden_context = PlanMemoryContext(
        project_id="repo",
        session_id="session-b",
        plan_id="plan-b",
        branch_id="feature-y",
    )
    specs = visible_specs + [("other-session", hidden_context, "session")]
    for episode_id, episode_context, sharing in specs:
        store.store(
            Episode.new(
                EpisodeDraft(
                    id=episode_id,
                    intent="org plan evidence",
                    intent_embedding=store.encoder.encode("org plan evidence"),
                    experience=episode_id,
                    outcome="success",
                ).with_plan_context(episode_context, sharing=sharing)
            )
        )

    visible_ids = {
        episode.id
        for episode, _score in store.recall_for_plan(
            "org plan evidence",
            context,
            top_k=8,
        )
    }

    assert visible_ids == {
        "isolated-plan-a",
        "session-shared",
        "branch-shared",
        "project-shared",
    }


def test_plan_context_can_be_built_from_org_properties() -> None:
    context = PlanMemoryContext.from_org_properties(
        {
            "PLAN_PROJECT": "repo",
            "PLAN_SESSION": "session-a",
            "PLAN_ID": "plan-a",
            "PLAN_BRANCH": "feature-x",
        }
    )

    assert context.project_id == "repo"
    assert context.session_id == "session-a"
    assert context.plan_id == "plan-a"
    assert context.branch_id == "feature-x"


def test_plan_recall_computation_is_built_from_org_properties() -> None:
    computation = PlanRecallComputation.from_org_properties(
        {
            "MEMORY_SCOPE": "project=repo;session=session-a;plan=plan-a;branch=main",
            "MEMORY_RECALL_K1": "9",
            "MEMORY_RECALL_K2": "3",
            "MEMORY_RECALL_LAMBDA": "0.30",
            "MEMORY_MIN_SCORE": "0.20",
            "MEMORY_MAX_CONTEXT_CHARS": "800",
            "MEMORY_FEEDBACK_BIAS": "-0.5",
        }
    )

    assert computation.scope_key == "project=repo;session=session-a;plan=plan-a;branch=main"
    assert computation.feedback_bias == -0.5
    assert computation.tuning.k1 > 9
    assert computation.tuning.k2 > 3
    assert computation.tuning.lambda_ < 0.30
    assert computation.tuning.min_score < 0.20
    assert computation.tuning.max_context_chars > 800


def test_plan_recall_uses_org_computation_tuning() -> None:
    store = EpisodeStore(StoreConfig(embedding_dim=8))
    context = PlanMemoryContext(
        project_id="repo",
        session_id="session-a",
        plan_id="plan-a",
        branch_id="main",
    )
    for episode_id in ("first", "second"):
        store.store(
            Episode.new(
                EpisodeDraft(
                    id=episode_id,
                    intent="org memory recall",
                    intent_embedding=store.encoder.encode("org memory recall"),
                    experience=f"{episode_id} result",
                    outcome="success",
                ).with_plan_context(context, sharing="session")
            )
        )
    computation = PlanRecallComputation.from_org_properties(
        {
            "MEMORY_SCOPE": "project=repo;session=session-a;plan=plan-a;branch=main",
            "MEMORY_RECALL_K1": "2",
            "MEMORY_RECALL_K2": "1",
            "MEMORY_RECALL_LAMBDA": "0.30",
            "MEMORY_MIN_SCORE": "0.0",
            "MEMORY_MAX_CONTEXT_CHARS": "1000",
            "MEMORY_FEEDBACK_BIAS": "0.0",
        }
    )

    results = store.recall_for_plan(
        "org memory recall",
        context,
        computation=computation,
    )

    assert len(results) == 1


def test_graph_turbo_read_memory_projection_suppresses_adjacent_selectors() -> None:
    projection = read_memory_projection(
        ["src/cli.py:10:20", "src/cli.py:18:26", "src/other.py:1:4"],
        ["src/cli.py:10:20"],
        max_gap_lines=8,
    )

    assert projection.seen_selectors == ("src/cli.py:10:20",)
    assert projection.suppressed_selectors == (
        "src/cli.py:10:20",
        "src/cli.py:18:26",
    )
