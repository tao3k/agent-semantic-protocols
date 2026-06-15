"""Build and analyze agent-session observability artifacts."""

from __future__ import annotations

import json
from pathlib import Path

from jsonschema import Draft202012Validator

from tools.semantic_sandtable.agent_session import (
    AgentSessionConfig,
    build_agent_session_receipt,
    write_agent_session_receipt,
    write_agent_session_from_messages,
)
from tools.semantic_sandtable.agent_session_analyzer import (
    analyze_agent_session_receipt,
    graph_turbo_feedback_from_analysis,
    write_agent_session_analysis,
)
from tools.semantic_sandtable.agent_session_algorithm_feedback import (
    build_graph_turbo_algorithm_feedback,
    build_graph_turbo_calibration_proposal,
)
from tools.semantic_sandtable.agent_session_improvements import (
    build_agent_session_improvement_report,
)
from tools.semantic_sandtable.cli import semantic_sandtable_main as main


_ROOT = Path(__file__).resolve().parents[3]


def test_agent_session_artifact_receipt_and_analysis(tmp_path: Path) -> None:
    session_root = tmp_path / "session"
    config = _config(session_root)

    manifest = write_agent_session_from_messages(
        _messages(),
        session_root,
        config=config,
    )
    receipt = build_agent_session_receipt(session_root, config=config)
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

    _validate_schema("semantic-agent-session-receipt.v1.schema.json", receipt)
    _validate_schema("semantic-agent-session-quality-report.v1.schema.json", quality)
    _validate_schema(
        "semantic-agent-session-improvement-report.v1.schema.json",
        improvement,
    )
    _validate_schema(
        "semantic-agent-session-graph-turbo-feedback.v1.schema.json",
        feedback,
    )
    assert manifest["eventCount"] >= 6
    assert (session_root / "events.jsonl").is_file()
    assert (session_root / "messages.jsonl").is_file()
    assert (session_root / "outputs" / "command-call_1.stdout").read_text(
        encoding="utf-8"
    ) == _search_output()
    assert receipt["summary"]["commandCount"] == 1
    assert receipt["summary"]["searchPrimeCommands"] == 1
    assert receipt["answer"]["present"] is True
    assert receipt["answer"]["groundingStatus"] == "grounded"
    assert quality["turnSummary"]["phaseCounts"]["command-result"] == 1
    assert quality["turnSummary"]["qualitySignalCounts"]["answer-grounded"] == 1
    assert quality["roundSummary"]["totalRounds"] == 1
    assert quality["roundSummary"]["commandKindCounts"]["search"] == 1
    assert quality["roundDetails"][0]["resultStatus"] == "complete"
    assert {turn["phase"] for turn in quality["turnDetails"]} >= {
        "command-result",
        "answer",
    }
    assert quality["findings"] == []
    assert feedback["candidates"] == []
    assert improvement["metrics"]["totalRounds"] == 1
    assert improvement["improvementPoints"] == []


def test_agent_session_analyzer_reports_repeated_search_feedback(
    tmp_path: Path,
) -> None:
    session_root = tmp_path / "session"
    messages = _messages() + _repeat_search_messages()
    write_agent_session_from_messages(messages, session_root, config=_config(session_root))
    receipt = build_agent_session_receipt(session_root)

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

    finding_ids = {finding["id"] for finding in quality["findings"]}
    candidate_kinds = {candidate["kind"] for candidate in feedback["candidates"]}
    improvement_ids = {point["id"] for point in improvement["improvementPoints"]}
    turn_signals = {
        signal
        for turn in quality["turnDetails"]
        for signal in turn["qualitySignals"]
    }
    assert "command.repeated" in finding_ids
    assert "repeated-query-group" in candidate_kinds
    assert "improve.command.repeated" in improvement_ids
    assert "repeated-command" in turn_signals
    assert quality["turnSummary"]["qualitySignalCounts"]["repeated-command"] == 2
    assert quality["roundSummary"]["repeatedRounds"] == 2
    assert improvement["metrics"]["repeatedCommands"] == 1
    assert {
        round_detail["resultStatus"] for round_detail in quality["roundDetails"]
    } == {"warning"}


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

    receipt_path = session_root / "receipts" / "agent-session-receipt.json"
    write_agent_session_receipt(session_root, receipt_path, config=_config(session_root))
    write_agent_session_analysis(
        receipt_path,
        session_root / "reports" / "quality-report.json",
        session_root / "reports" / "graph-turbo-feedback.json",
        session_root / "reports" / "improvement-report.json",
        session_root / "reports" / "algorithm-graph-feedback.json",
        session_root / "reports" / "algorithm-calibration.json",
    )
    written_calibration = json.loads(
        (session_root / "reports" / "algorithm-calibration.json").read_text(
            encoding="utf-8"
        )
    )
    assert written_calibration["metrics"]["kindDeltaCount"] >= 1


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
    assert "risk=flat-query,owner-drift" in candidate["reason"]


def test_agent_session_turn_details_report_denied_direct_read_and_missing_answer(
    tmp_path: Path,
) -> None:
    session_root = tmp_path / "session"
    write_agent_session_from_messages(
        _denied_direct_read_messages(),
        session_root,
        config=_config(session_root),
    )
    receipt = build_agent_session_receipt(session_root)

    quality = analyze_agent_session_receipt(receipt)
    turn_signals = {
        signal
        for turn in quality["turnDetails"]
        for signal in turn["qualitySignals"]
    }
    finding_ids = {finding["id"] for finding in quality["findings"]}

    assert {"read.direct-risk", "hook.denied", "answer.missing"} <= finding_ids
    assert {
        "direct-read-risk",
        "hook-denied",
        "answer-missing",
    } <= turn_signals
    assert quality["turnSummary"]["qualitySignalCounts"]["direct-read-risk"] == 1
    assert quality["turnSummary"]["qualitySignalCounts"]["hook-denied"] == 1
    assert quality["turnSummary"]["qualitySignalCounts"]["answer-missing"] == 1
    assert quality["roundSummary"]["deniedRounds"] == 1
    assert quality["roundSummary"]["riskRounds"] == 1
    assert quality["roundDetails"][0]["resultStatus"] == "denied"


def test_cli_records_and_analyzes_agent_session_from_messages(
    tmp_path: Path,
) -> None:
    messages_path = tmp_path / "messages.jsonl"
    messages_path.write_text(
        "\n".join(json.dumps(message) for message in _messages()) + "\n",
        encoding="utf-8",
    )
    session_root = tmp_path / "session"

    assert (
        main(
            [
                "--repo-root",
                str(_ROOT),
                "--record-agent-session-from-messages",
                str(messages_path),
                "--agent-session-root",
                str(session_root),
                "--session-id",
                "cli-session",
                "--scenario-id",
                "rust.tokio-agent-observability",
                "--language",
                "rust",
                "--project-name",
                "tokio",
                "--project-source",
                "registry",
                "--intent",
                "Explain Tokio IO readiness.",
                "--analyzer",
            ]
        )
        == 0
    )
    assert (session_root / "receipts" / "agent-session-receipt.json").is_file()
    quality = json.loads(
        (session_root / "reports" / "quality-report.json").read_text(
            encoding="utf-8"
        )
    )
    improvement = json.loads(
        (session_root / "reports" / "improvement-report.json").read_text(
            encoding="utf-8"
        )
    )
    assert any(
        turn["phase"] == "command-result"
        and "command-recorded" in turn["qualitySignals"]
        for turn in quality["turnDetails"]
    )
    assert quality["roundSummary"]["totalRounds"] == 1
    assert quality["roundDetails"][0]["commandKind"] == "search"
    assert improvement["metrics"]["commandCount"] == 1
    assert improvement["metrics"]["totalRounds"] == 1
    assert (session_root / "reports" / "graph-turbo-feedback.json").is_file()
    assert (session_root / "reports" / "algorithm-graph-feedback.json").is_file()
    assert (session_root / "reports" / "algorithm-calibration.json").is_file()


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


def _messages() -> list[dict[str, object]]:
    return [
        {
            "type": "AssistantMessage",
            "content": [
                {
                    "id": "call_1",
                    "name": "Bash",
                    "input": {
                        "command": "asp rust search prime --view seeds .",
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
                        "command": "asp rust search prime --view seeds .",
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


def _graph_turbo_seed_plan_messages() -> list[dict[str, object]]:
    return [
        {
            "type": "AssistantMessage",
            "content": [
                {
                    "id": "call_seed",
                    "name": "Bash",
                    "input": {
                        "command": (
                            "asp rust search pipe 'tokio readiness' "
                            "--view graph-turbo-request ."
                        ),
                    },
                }
            ],
        },
        {
            "type": "UserMessage",
            "content": [
                {
                    "tool_use_id": "call_seed",
                    "content": _graph_turbo_seed_plan_output(),
                    "is_error": False,
                }
            ],
        },
        {
            "type": "ResultMessage",
            "result": "Tokio readiness search reached a graph-turbo seed request.",
            "usage": {"input_tokens": 10, "output_tokens": 5},
            "total_cost_usd": 0.01,
        },
    ]


def _graph_turbo_seed_plan_risk_messages() -> list[dict[str, object]]:
    return [
        {
            "type": "AssistantMessage",
            "content": [
                {
                    "id": "call_seed_risk",
                    "name": "Bash",
                    "input": {
                        "command": (
                            "asp rust search pipe 'cache runtime graph package parser' "
                            "--view graph-turbo-request ."
                        ),
                    },
                }
            ],
        },
        {
            "type": "UserMessage",
            "content": [
                {
                    "tool_use_id": "call_seed_risk",
                    "content": _graph_turbo_seed_plan_risk_output(),
                    "is_error": False,
                }
            ],
        },
        {
            "type": "ResultMessage",
            "result": "Graph-turbo seed request exposed flat-query risk.",
            "usage": {"input_tokens": 10, "output_tokens": 5},
            "total_cost_usd": 0.01,
        },
    ]


def _denied_direct_read_messages() -> list[dict[str, object]]:
    return [
        {
            "type": "AssistantMessage",
            "content": [
                {
                    "id": "call_1",
                    "name": "Bash",
                    "input": {
                        "command": "asp rust direct-source-read --code src/lib.rs",
                    },
                }
            ],
        },
        {
            "type": "UserMessage",
            "content": [
                {
                    "tool_use_id": "call_1",
                    "content": "ASP hook denied: use asp rust search prime first.",
                    "is_error": True,
                }
            ],
        },
    ]


def _search_output() -> str:
    return "\n".join(
        [
            "[search-prime] language=rust project=tokio",
            "nextCommand=asp rust search pipe 'AsyncRead readiness' --view seeds .",
        ]
    )


def _graph_turbo_seed_plan_output() -> str:
    return json.dumps(
        {
            "schemaId": "agent.semantic-protocols.semantic-graph-turbo-request",
            "queryTerms": [],
            "seedIds": ["owner:src/lib.rs"],
            "graph": {
                "nodes": [
                    {
                        "id": "owner:src/lib.rs",
                        "kind": "owner",
                        "role": "path",
                        "value": "src/lib.rs",
                        "locator": "owner:src/lib.rs",
                    }
                ],
                "edges": [],
            },
            "seedPlan": {
                "phase": "seed-query",
                "algorithm": "asp-search-pipe-v2",
                "reason": "fallback-owner",
                "queryPresent": False,
                "querySeedPresent": False,
                "candidateCount": 3,
                "candidateOwnerCount": 1,
                "fallbackOwnerSeedCount": 1,
                "selectedSeedCount": 1,
                "seedIds": ["owner:src/lib.rs"],
            },
        }
    )


def _graph_turbo_seed_plan_risk_output() -> str:
    return json.dumps(
        {
            "schemaId": "agent.semantic-protocols.semantic-graph-turbo-request",
            "queryTerms": [
                "cache",
                "runtime",
                "graph",
                "package",
                "parser",
                "owner",
            ],
            "seedIds": ["query:cache_runtime_graph_package_parser"],
            "seedPlan": {
                "phase": "seed-query",
                "algorithm": "asp-search-pipe-v2",
                "reason": "query",
                "queryPresent": True,
                "querySeedPresent": True,
                "candidateCount": 12,
                "candidateOwnerCount": 5,
                "fallbackOwnerSeedCount": 0,
                "selectedSeedCount": 1,
                "seedIds": ["query:cache_runtime_graph_package_parser"],
            },
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
