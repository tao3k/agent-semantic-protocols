"""Build and aggregate agent-session question improvement plans."""

from __future__ import annotations

import json
from pathlib import Path

from jsonschema import Draft202012Validator

from tools.semantic_sandtable.agent_session import (
    AgentSessionConfig,
    build_agent_session_receipt,
    write_agent_session_from_messages,
)
from tools.semantic_sandtable.agent_session_analyzer import (
    analyze_agent_session_receipt,
    graph_turbo_feedback_from_analysis,
)
from tools.semantic_sandtable.agent_session_improvements import (
    build_agent_session_improvement_report,
)
from tools.semantic_sandtable.agent_session_question_plan import (
    aggregate_agent_session_question_plans,
    build_agent_session_question_plan,
)
from tools.semantic_sandtable.cli import semantic_sandtable_main as main


_ROOT = Path(__file__).resolve().parents[3]


def test_agent_session_question_plan_aggregate_rolls_up_multiple_questions(
    tmp_path: Path,
) -> None:
    rust_plan = _question_plan_for_messages(
        tmp_path / "rust-session",
        session_id="rust-question",
        scenario_id="rust.tokio-question",
        language="rust",
        project_name="tokio",
        intent="Explain Tokio IO readiness.",
    )
    typescript_plan = _question_plan_for_messages(
        tmp_path / "typescript-session",
        session_id="typescript-question",
        scenario_id="typescript.effect-question",
        language="typescript",
        project_name="effect",
        intent="Explain Effect service dependency tags.",
        messages=_messages_with_repeated_search(),
    )

    aggregate = aggregate_agent_session_question_plans(
        [rust_plan, typescript_plan],
        source_paths=[
            "rust/reports/question-improvement-plan.json",
            "typescript/reports/question-improvement-plan.json",
        ],
    )

    _validate_schema("semantic-agent-session-question-plan.v1.schema.json", aggregate)
    assert aggregate["rollup"]["questionCount"] == 2
    assert aggregate["rollup"]["pendingHumanReviews"] == 2
    assert aggregate["rollup"]["analyzerStatusCounts"] == {"pass": 1, "review": 1}
    assert aggregate["rollup"]["answerGroundingCounts"] == {"grounded": 2}
    assert aggregate["rollup"]["languageCounts"] == {"rust": 1, "typescript": 1}
    assert aggregate["rollup"]["projectCounts"] == {"effect": 1, "tokio": 1}
    assert aggregate["rollup"]["findingCounts"] == {"command.repeated": 1}
    assert aggregate["rollup"]["graphTurboCandidateKindCounts"] == {
        "repeated-query-group": 1
    }
    assert aggregate["rollup"]["revealedSignalCounts"] == {
        "mentions-asp-flow": 2,
        "mentions-evidence": 2,
        "mentions-final-claim": 2,
    }
    assert aggregate["rollup"]["planItemCount"] == 4
    assert aggregate["rollup"]["planItemIdCounts"] == {
        "question-plan.human-review.final-answer": 2,
        "question-plan.improve.command.repeated": 1,
        "question-plan.improve.gt.command.repeated": 1,
    }
    assert aggregate["rollup"]["planItemCategoryCounts"] == {
        "answer-review": 2,
        "command-efficiency": 1,
        "graph-turbo": 1,
    }
    assert aggregate["rollup"]["planItemSeverityCounts"] == {
        "info": 2,
        "warning": 2,
    }
    assert aggregate["rollup"]["planItemSourceCounts"] == {
        "analyzer": 2,
        "human-review": 2,
    }
    assert len(aggregate["questions"]) == 2
    first_question = aggregate["questions"][0]
    assert first_question["sourceSession"] == {
        "sessionId": "rust-question",
        "scenarioId": "rust.tokio-question",
    }
    assert first_question["sourceArtifacts"]["receiptPath"] == (
        "receipts/agent-session-receipt.json"
    )
    assert first_question["analysisMetrics"]["totalRounds"] >= 1


def test_cli_aggregates_question_plans(
    tmp_path: Path,
) -> None:
    plan_a = _question_plan_for_messages(
        tmp_path / "session-a",
        session_id="session-a",
        scenario_id="rust.tokio-question-a",
        language="rust",
        project_name="tokio",
        intent="Explain Tokio IO readiness.",
    )
    plan_b = _question_plan_for_messages(
        tmp_path / "session-b",
        session_id="session-b",
        scenario_id="typescript.effect-question-b",
        language="typescript",
        project_name="effect",
        intent="Explain Effect service dependency tags.",
    )
    plan_a_path = tmp_path / "plan-a.json"
    plan_b_path = tmp_path / "plan-b.json"
    output_path = tmp_path / "aggregate.json"
    plan_a_path.write_text(json.dumps(plan_a), encoding="utf-8")
    plan_b_path.write_text(json.dumps(plan_b), encoding="utf-8")

    assert (
        main(
            [
                "--repo-root",
                str(_ROOT),
                "--aggregate-question-plans",
                str(plan_a_path),
                str(plan_b_path),
                "--question-plan-output",
                str(output_path),
            ]
        )
        == 0
    )

    aggregate = json.loads(output_path.read_text(encoding="utf-8"))
    _validate_schema("semantic-agent-session-question-plan.v1.schema.json", aggregate)
    assert aggregate["rollup"]["questionCount"] == 2
    assert aggregate["sourceQuestionPlanPaths"] == [str(plan_a_path), str(plan_b_path)]


def _question_plan_for_messages(
    session_root: Path,
    *,
    session_id: str,
    scenario_id: str,
    language: str,
    project_name: str,
    intent: str,
    messages: list[dict[str, object]] | None = None,
) -> dict[str, object]:
    config = AgentSessionConfig(
        session_id=session_id,
        scenario_id=scenario_id,
        language=language,
        project_name=project_name,
        project_source="registry",
        project_workdir=str(session_root),
        intent=intent,
        model="sonnet",
    )
    write_agent_session_from_messages(
        messages or _messages(),
        session_root,
        config=config,
    )
    receipt = build_agent_session_receipt(session_root, config=config)
    events = _read_jsonl(session_root / "events.jsonl")
    quality = analyze_agent_session_receipt(receipt)
    feedback = graph_turbo_feedback_from_analysis(
        receipt,
        quality,
        source_receipt_path="receipts/agent-session-receipt.json",
    )
    improvement = build_agent_session_improvement_report(
        quality,
        feedback,
        source_quality_report_path="reports/quality-report.json",
        source_graph_turbo_feedback_path="reports/graph-turbo-feedback.json",
    )
    return build_agent_session_question_plan(
        receipt,
        quality,
        feedback,
        improvement,
        events,
        source_receipt_path="receipts/agent-session-receipt.json",
        source_quality_report_path="reports/quality-report.json",
        source_graph_turbo_feedback_path="reports/graph-turbo-feedback.json",
        source_improvement_report_path="reports/improvement-report.json",
    )


def _messages() -> list[dict[str, object]]:
    return [
        {
            "type": "AssistantMessage",
            "content": [
                {
                    "id": "call_1",
                    "name": "Bash",
                    "input": {
                        "command": "asp rust search prime --workspace . --view seeds",
                    },
                }
            ],
        },
        {
            "type": "UserMessage",
            "content": [
                {
                    "tool_use_id": "call_1",
                    "content": _search_output(),
                    "is_error": False,
                }
            ],
        },
        {
            "type": "ResultMessage",
            "result": (
                "Tokio IO readiness should be inspected from the runtime IO "
                "frontier selected by the ASP search output."
            ),
            "usage": {"input_tokens": 10, "output_tokens": 5},
            "total_cost_usd": 0.01,
        },
    ]


def _repeat_search_messages() -> list[dict[str, object]]:
    return [
        {
            "type": "AssistantMessage",
            "content": [
                {
                    "id": "call_2",
                    "name": "Bash",
                    "input": {
                        "command": "asp rust search prime --workspace . --view seeds",
                    },
                }
            ],
        },
        {
            "type": "UserMessage",
            "content": [
                {
                    "tool_use_id": "call_2",
                    "content": _search_output(),
                    "is_error": False,
                }
            ],
        },
    ]


def _messages_with_repeated_search() -> list[dict[str, object]]:
    base_messages = _messages()
    return base_messages[:2] + _repeat_search_messages() + base_messages[2:]


def _search_output() -> str:
    return "\n".join(
        [
            "[search-prime] language=rust project=tokio",
            "nextCommand=asp rust search pipe 'AsyncRead readiness' --workspace . --view seeds",
        ]
    )


def _read_jsonl(path: Path) -> list[dict[str, object]]:
    return [
        json.loads(line)
        for line in path.read_text(encoding="utf-8").splitlines()
        if line.strip()
    ]


def _validate_schema(schema_name: str, payload: dict[str, object]) -> None:
    schema = json.loads((_ROOT / "schemas" / schema_name).read_text())
    Draft202012Validator(schema).validate(payload)
