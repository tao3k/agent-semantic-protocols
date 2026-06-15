"""Build and analyze agent-session observability artifacts."""

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
from tools.semantic_sandtable.agent_session_algorithm_feedback import (
    build_graph_turbo_algorithm_feedback,
)
from tools.semantic_sandtable.agent_session_improvements import (
    build_agent_session_improvement_report,
)
from tools.semantic_sandtable.agent_session_question_plan import (
    build_agent_session_question_plan,
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
    question_plan = build_agent_session_question_plan(
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
    _validate_schema("semantic-agent-session-question-plan.v1.schema.json", question_plan)
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
    assert question_plan["questions"][0]["question"] == "Explain Tokio IO readiness."
    assert question_plan["questions"][0]["finalAnswer"]["present"] is True
    assert question_plan["questions"][0]["analyzerJudgment"]["status"] == "pass"
    assert question_plan["questions"][0]["humanReview"]["status"] == "pending"
    assert question_plan["rollup"]["pendingHumanReviews"] == 1
    assert question_plan["rollup"]["languageCounts"] == {"rust": 1}
    assert question_plan["rollup"]["projectCounts"] == {"tokio": 1}


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


def test_agent_session_analyzer_reports_search_flow_path_drift(
    tmp_path: Path,
) -> None:
    session_root = tmp_path / "session"
    write_agent_session_from_messages(
        _gerbil_path_drift_messages(),
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
    algorithm_feedback = build_graph_turbo_algorithm_feedback(
        improvement,
        feedback,
        source_path="reports/improvement-report.json",
    )

    _validate_schema("semantic-agent-session-quality-report.v1.schema.json", quality)
    _validate_schema(
        "semantic-agent-session-graph-turbo-feedback.v1.schema.json",
        feedback,
    )
    finding_ids = {finding["id"] for finding in quality["findings"]}
    candidate_kinds = {candidate["kind"] for candidate in feedback["candidates"]}
    assert {
        "search.package-drift",
        "search.path-intent-lost",
        "search.finder-path-ignored",
    } <= finding_ids
    assert {
        "search-flow-drift",
        "path-intent-lost",
        "finder-path-ignored",
    } <= candidate_kinds
    path_candidate = next(
        candidate
        for candidate in feedback["candidates"]
        if candidate["kind"] == "finder-path-ignored"
    )
    assert path_candidate["matchedSelectors"] == [".data/gerbil-poo/cli.ss"]
    assert any(
        node["fields"]["effect"] == "boost"
        and node["fields"]["selector"] == ".data/gerbil-poo/cli.ss"
        for node in algorithm_feedback["graph"]["nodes"]
    )


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
    question_plan = json.loads(
        (session_root / "reports" / "question-improvement-plan.json").read_text(
            encoding="utf-8"
        )
    )
    assert question_plan["questions"][0]["question"] == "Explain Tokio IO readiness."
    assert question_plan["questions"][0]["humanReview"]["required"] is True


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


def _gerbil_path_drift_messages() -> list[dict[str, object]]:
    return [
        {
            "type": "AssistantMessage",
            "content": [
                {
                    "text": (
                        "目标是看 .data/gerbil-poo/cli.ss 的真实 CLI 组织方式。"
                    )
                },
                {
                    "id": "call_pipe",
                    "name": "Bash",
                    "input": {
                        "command": (
                            "asp gerbil-scheme search pipe "
                            "'gerbil-poo cli.ss command import main' "
                            "--workspace . --view seeds"
                        ),
                    },
                },
            ],
        },
        {
            "type": "UserMessage",
            "content": [
                {
                    "tool_use_id": "call_pipe",
                    "content": _gerbil_path_drift_pipe_output(),
                    "is_error": False,
                }
            ],
        },
        {
            "type": "AssistantMessage",
            "content": [
                {
                    "id": "call_fd",
                    "name": "Bash",
                    "input": {
                        "command": "asp fd -query 'cli.ss gerbil-poo' .data --view seeds",
                    },
                }
            ],
        },
        {
            "type": "UserMessage",
            "content": [
                {
                    "tool_use_id": "call_fd",
                    "content": _gerbil_path_drift_fd_output(),
                    "is_error": False,
                }
            ],
        },
        {
            "type": "ResultMessage",
            "result": "The search flow found the path drift problem.",
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


def _gerbil_path_drift_pipe_output() -> str:
    return "\n".join(
        [
            "[search-pipe] lang=gerbil-scheme view=seeds",
            "queryQuality=low reason=package-drift",
            "risk=package-drift",
            (
                "ownerCoverage=bestOwner="
                "languages/gerbil-scheme-language-project-harness/src/cli.ss "
                "matched=gerbil,cli,ss missing=poo,data"
            ),
            "pathCoverage=matched=- missing=-",
            "recommendedNext=A1.rg-query",
            "nextCommand=asp rg -query 'gerbil|poo|cli|ss' .",
        ]
    )


def _gerbil_path_drift_fd_output() -> str:
    return "\n".join(
        [
            "[search-fd] view=seeds querySet=2",
            "queryQuality=medium reason=path-candidate",
            "ownerCandidates=.data/gerbil-poo/cli.ss",
            (
                "nextCommand=asp gerbil-scheme search owner "
                "languages/gerbil-scheme-language-project-harness/src/parser/brace.ss "
                "items --query 'main|command' --view seeds ."
            ),
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
