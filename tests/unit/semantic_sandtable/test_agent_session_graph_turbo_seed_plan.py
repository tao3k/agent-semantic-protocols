"""Analyze graph-turbo seed-plan evidence in agent sessions."""

from __future__ import annotations

import json
from pathlib import Path

from jsonschema import Draft202012Validator

from tools.semantic_sandtable.agent_session import (
    AgentSessionConfig,
    build_agent_session_receipt,
    write_agent_session_from_messages,
)
from tools.semantic_sandtable.agent_session_algorithm_feedback import (
    build_graph_turbo_algorithm_feedback,
    build_graph_turbo_calibration_proposal,
)
from tools.semantic_sandtable.agent_session_analyzer import (
    analyze_agent_session_receipt,
    graph_turbo_feedback_from_analysis,
)
from tools.semantic_sandtable.agent_session_improvements import (
    build_agent_session_improvement_report,
)


_ROOT = Path(__file__).resolve().parents[3]


def test_agent_session_analyzer_reports_graph_turbo_seed_plan_feedback(
    tmp_path: Path,
) -> None:
    session_root = tmp_path / "session"
    write_agent_session_from_messages(
        _graph_turbo_seed_plan_messages(),
        session_root,
        config=_config(session_root),
    )
    receipt = build_agent_session_receipt(session_root)
    events = _read_jsonl(session_root / "events.jsonl")

    quality = analyze_agent_session_receipt(receipt, events=events)
    feedback = graph_turbo_feedback_from_analysis(
        receipt,
        quality,
        source_receipt_path="receipts/agent-session-receipt.json",
        events=events,
    )
    improvement = build_agent_session_improvement_report(
        quality,
        feedback,
        source_quality_report_path="reports/quality-report.json",
        source_graph_turbo_feedback_path="reports/graph-turbo-feedback.json",
    )

    _validate_schema(
        "semantic-agent-session-graph-turbo-feedback.v1.schema.json",
        feedback,
    )
    candidate = next(
        candidate
        for candidate in feedback["candidates"]
        if candidate["kind"] == "seed-plan-quality"
    )
    assert candidate["packetNodeIds"] == ["owner:src/lib.rs"]
    assert candidate["expectedChange"] == "query-seed-present"
    assert candidate["seedQuality"] == "review"
    assert candidate["riskFactors"] == ["fallback-owner", "query-seed-missing"]
    assert candidate["recommendedActions"] == [
        "replace-fallback-owner-seed",
        "propagate-query-seed",
    ]
    assert "fallbackOwnerSeedCount=1" in candidate["reason"]
    assert any(
        point.get("sourceCandidateIds") == [candidate["id"]]
        for point in improvement["improvementPoints"]
    )
    algorithm_feedback = build_graph_turbo_algorithm_feedback(
        improvement,
        feedback,
        source_path="reports/improvement-report.json",
    )
    _validate_schema("semantic-graph-turbo-feedback.v1.schema.json", algorithm_feedback)
    assert algorithm_feedback["metrics"]["penaltyCount"] >= 1
    assert any(
        node["fields"]["reason"] == "seed-plan-quality"
        for node in algorithm_feedback["graph"]["nodes"]
    )
    calibration = build_graph_turbo_calibration_proposal(
        algorithm_feedback,
        request_packet=json.loads(_graph_turbo_seed_plan_output()),
        profile="owner-query",
    )
    _validate_schema("semantic-graph-turbo-calibration.v1.schema.json", calibration)
    assert calibration["packetKind"] == "graph-turbo-calibration"
    assert calibration["metrics"]["kindDeltaCount"] >= 1
    assert calibration["metrics"]["relationDeltaCount"] >= 1

def test_agent_session_analyzer_reports_seed_plan_risk_feedback(
    tmp_path: Path,
) -> None:
    session_root = tmp_path / "session"
    write_agent_session_from_messages(
        _graph_turbo_seed_plan_risk_messages(),
        session_root,
        config=_config(session_root),
    )
    receipt = build_agent_session_receipt(session_root)
    events = _read_jsonl(session_root / "events.jsonl")

    quality = analyze_agent_session_receipt(receipt, events=events)
    feedback = graph_turbo_feedback_from_analysis(
        receipt,
        quality,
        source_receipt_path="receipts/agent-session-receipt.json",
        events=events,
    )

    candidate = next(
        candidate
        for candidate in feedback["candidates"]
        if candidate["kind"] == "seed-plan-quality"
    )
    assert candidate["expectedChange"] == "split-query-pack"
    assert candidate["riskFactors"] == ["flat-query", "owner-drift"]
    assert candidate["recommendedActions"] == ["split-query-pack", "narrow-owner-scope"]
    assert "queryOwnerSeedCount=2" in candidate["reason"]
    assert "risk=flat-query,owner-drift" in candidate["reason"]


def _config(session_root: Path) -> AgentSessionConfig:
    return AgentSessionConfig(
        session_id="session-1",
        scenario_id="rust.tokio-agent-observability",
        language="rust",
        project_name="tokio",
        project_source="registry",
        project_workdir=str(session_root),
        intent="Explain Tokio IO readiness.",
        model="sonnet",
    )


def _graph_turbo_seed_plan_messages() -> list[dict[str, object]]:
    return _messages(
        call_id="call_seed",
        command="asp rust search pipe 'tokio readiness' --view graph-turbo-request .",
        output=_graph_turbo_seed_plan_output(),
        result="Tokio readiness search reached a graph-turbo seed request.",
    )


def _graph_turbo_seed_plan_risk_messages() -> list[dict[str, object]]:
    return _messages(
        call_id="call_seed_risk",
        command=(
            "asp rust search pipe 'cache runtime graph package parser' "
            "--view graph-turbo-request ."
        ),
        output=_graph_turbo_seed_plan_risk_output(),
        result="Graph-turbo seed request exposed flat-query risk.",
    )


def _messages(
    *,
    call_id: str,
    command: str,
    output: str,
    result: str,
) -> list[dict[str, object]]:
    return [
        {
            "type": "AssistantMessage",
            "content": [
                {"id": call_id, "name": "Bash", "input": {"command": command}}
            ],
        },
        {
            "type": "UserMessage",
            "content": [
                {
                    "tool_use_id": call_id,
                    "content": output,
                    "is_error": False,
                }
            ],
        },
        {
            "type": "ResultMessage",
            "result": result,
            "usage": {"input_tokens": 10, "output_tokens": 5},
            "total_cost_usd": 0.01,
        },
    ]


def _graph_turbo_seed_plan_output() -> str:
    seed_ids = ["owner:src/lib.rs"]
    return _request_output(
        seed_plan={
            "reason": "fallback-owner",
            "seedQuality": "review",
            "queryPresent": True,
            "querySeedPresent": False,
            "candidateCount": 3,
            "candidateOwnerCount": 1,
            "queryOwnerSeedCount": 0,
            "fallbackOwnerSeedCount": 1,
            "selectedSeedCount": 1,
            "seedIds": seed_ids,
            "riskFactors": ["fallback-owner", "query-seed-missing"],
            "recommendedActions": [
                "replace-fallback-owner-seed",
                "propagate-query-seed",
            ],
        },
        query_terms=[],
        seed_ids=seed_ids,
        graph_nodes=[
            {
                "id": "owner:src/lib.rs",
                "kind": "owner",
                "role": "path",
                "value": "src/lib.rs",
                "locator": "owner:src/lib.rs",
            }
        ],
    )


def _graph_turbo_seed_plan_risk_output() -> str:
    query_terms = ["cache", "runtime", "graph", "package", "parser", "owner"]
    seed_ids = ["query:cache_runtime_graph_package_parser"]
    return _request_output(
        seed_plan={
            "reason": "query",
            "seedQuality": "review",
            "queryPresent": True,
            "querySeedPresent": True,
            "candidateCount": 12,
            "candidateOwnerCount": 5,
            "queryOwnerSeedCount": 2,
            "fallbackOwnerSeedCount": 0,
            "selectedSeedCount": 1,
            "seedIds": seed_ids,
            "riskFactors": ["flat-query", "owner-drift"],
            "recommendedActions": ["split-query-pack", "narrow-owner-scope"],
        },
        query_terms=query_terms,
        seed_ids=seed_ids,
        graph_nodes=[],
    )


def _request_output(
    *,
    seed_plan: dict[str, object],
    query_terms: list[str],
    seed_ids: list[str],
    graph_nodes: list[dict[str, object]],
) -> str:
    seed_plan = {
        "phase": "seed-query",
        "algorithm": "asp-search-pipe-v2",
        **seed_plan,
    }
    return json.dumps(
        {
            "schemaId": "agent.semantic-protocols.semantic-graph-turbo-request",
            "queryTerms": query_terms,
            "seedIds": seed_ids,
            "graph": {"nodes": graph_nodes, "edges": []},
            "seedPlan": seed_plan,
        }
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
