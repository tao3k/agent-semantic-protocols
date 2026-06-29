"""Live agent-session CLI wrapper tests."""

from __future__ import annotations

import json
import subprocess
from pathlib import Path
from typing import Any

import pytest

from tools.semantic_sandtable.cli import semantic_sandtable_main as main


_ROOT = Path(__file__).resolve().parents[3]


def test_cli_records_live_agent_session_without_sdk(
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    captured: dict[str, Any] = {}

    def fake_run(
        command: list[str],
        *,
        cwd: Path,
    ) -> subprocess.CompletedProcess[str]:
        captured["command"] = command
        captured["cwd"] = cwd
        return subprocess.CompletedProcess(
            command,
            0,
            stdout=_stdout_messages(),
            stderr="sdk warning\n",
        )

    monkeypatch.setattr(
        "tools.semantic_sandtable.agent_session_cli._run_agent_session_command",
        fake_run,
    )
    session_root = tmp_path / "live-session"

    assert (
        main(
            [
                "--repo-root",
                str(_ROOT),
                "--record-agent-session",
                "--prompt",
                "Explain Tokio IO readiness.",
                "--agent-session-root",
                str(session_root),
                "--child-session-id",
                "live-cli-session",
                "--scenario-id",
                "rust.tokio-live-agent-observability",
                "--language",
                "rust",
                "--project-name",
                "tokio",
                "--project-source",
                "registry",
                "--intent",
                "Explain Tokio IO readiness.",
                "--model",
                "sonnet",
                "--include-hook-events",
                "--require-asp-bash-commands",
                "--max-asp-bash-commands",
                "3",
                "--add-cwd-dir",
                "--analyze-recorded-agent-session",
            ]
        )
        == 0
    )

    command = captured["command"]
    assert captured["cwd"] == _ROOT
    assert command[:3] == [
        command[0],
        "-m",
        "tools.semantic_sandtable.claude_sdk_runner",
    ]
    assert _option_value(command, "--prompt") == "Explain Tokio IO readiness."
    assert _option_value(command, "--output-format") == "stream-json"
    assert _option_value(command, "--model") == "sonnet"
    assert _option_value(command, "--max-asp-bash-commands") == "3"
    assert "--include-hook-events" in command
    assert "--require-asp-bash-commands" in command
    assert "--add-cwd-dir" in command

    receipt_path = session_root / "receipts" / "agent-session-receipt.json"
    receipt = json.loads(receipt_path.read_text(encoding="utf-8"))
    assert receipt["sessionId"] == "live-cli-session"
    assert receipt["model"] == "sonnet"
    assert receipt["summary"]["commandCount"] == 1
    assert receipt["answer"]["present"] is True
    assert (session_root / "sdk-stdout.jsonl").read_text(
        encoding="utf-8"
    ) == _stdout_messages()
    assert (session_root / "sdk-stderr.txt").read_text(
        encoding="utf-8"
    ) == "sdk warning\n"
    quality = json.loads(
        (session_root / "reports" / "quality-report.json").read_text(
            encoding="utf-8"
        )
    )
    feedback = json.loads(
        (session_root / "reports" / "graph-turbo-feedback.json").read_text(
            encoding="utf-8"
        )
    )
    improvement = json.loads(
        (session_root / "reports" / "improvement-report.json").read_text(
            encoding="utf-8"
        )
    )
    assert quality["sessionId"] == "live-cli-session"
    assert feedback["sessionId"] == "live-cli-session"
    assert improvement["sessionId"] == "live-cli-session"
    assert quality["turnSummary"]["phaseCounts"]["command-result"] == 1
    assert quality["turnSummary"]["qualitySignalCounts"]["command-started"] == 1
    assert quality["turnSummary"]["qualitySignalCounts"]["search-prime"] == 1
    assert quality["roundSummary"]["totalRounds"] == 1
    assert quality["roundDetails"][0]["commandKind"] == "search"
    assert quality["roundDetails"][0]["resultStatus"] == "complete"
    assert improvement["metrics"]["roundStatusCounts"]["complete"] == 1
    assert improvement["improvementPoints"] == []
    assert any(
        turn["phase"] == "command-result"
        and "search-prime" in turn["qualitySignals"]
        for turn in quality["turnDetails"]
    )
    assert any(
        turn["phase"] == "answer"
        and "answer-grounded" in turn["qualitySignals"]
        for turn in quality["turnDetails"]
    )


def _stdout_messages() -> str:
    return "\n".join(json.dumps(message) for message in _messages()) + "\n"


def _messages() -> list[dict[str, object]]:
    return [
        {
            "type": "AssistantMessage",
            "content": [
                {
                    "id": "call_1",
                    "name": "Bash",
                    "input": {"command": "asp rust search prime --workspace . --view seeds"},
                }
            ],
        },
        {
            "type": "UserMessage",
            "content": [
                {
                    "tool_use_id": "call_1",
                    "content": (
                        "[search-prime] language=rust project=tokio\n"
                        "nextCommand=asp rust search pipe 'readiness' --workspace . --view seeds"
                    ),
                    "is_error": False,
                }
            ],
        },
        {
            "type": "ResultMessage",
            "result": "Tokio readiness is grounded in the ASP search frontier.",
            "usage": {"input_tokens": 10, "output_tokens": 5},
            "total_cost_usd": 0.01,
        },
    ]


def _option_value(command: list[str], option: str) -> str | None:
    try:
        index = command.index(option)
    except ValueError:
        return None
    return command[index + 1]
