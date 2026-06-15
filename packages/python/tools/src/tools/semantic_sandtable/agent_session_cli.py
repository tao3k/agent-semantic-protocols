"""CLI adapter for agent-session observability artifacts."""

from __future__ import annotations

import argparse
import subprocess
import sys
from pathlib import Path
from typing import Any

from .agent_observation_json import load_stdout_messages
from .agent_session import (
    AgentSessionConfig,
    load_agent_messages,
    write_agent_session_from_messages,
    write_agent_session_receipt,
)
from .agent_session_analyzer import write_agent_session_analysis
from .output import emit, emit_json
from .utils import resolve_path


def add_agent_session_arguments(parser: argparse.ArgumentParser) -> None:
    _add_recording_arguments(parser)
    _add_live_runner_arguments(parser)
    _add_session_metadata_arguments(parser)
    _add_analysis_arguments(parser)


def _add_recording_arguments(parser: argparse.ArgumentParser) -> None:
    parser.add_argument(
        "--record-agent-session",
        action="store_true",
        help="Run the Claude SDK sandtable runner and record a live agent session.",
    )
    parser.add_argument(
        "--record-agent-session-from-messages",
        metavar="MESSAGES",
        help="Build a raw agent-session artifact from Claude SDK JSON/JSONL messages.",
    )
    parser.add_argument(
        "--agent-session-root",
        help="Output root for agent-session record commands.",
    )


def _add_live_runner_arguments(parser: argparse.ArgumentParser) -> None:
    parser.add_argument(
        "--prompt",
        help="Prompt for --record-agent-session live Claude SDK runs.",
    )
    parser.add_argument(
        "--agent-output-format",
        "--output-format",
        dest="agent_output_format",
        default="stream-json",
        choices=("text", "json", "stream-json", "summary-json"),
        help="Claude SDK runner output format for --record-agent-session.",
    )
    parser.add_argument("--include-partial-messages", action="store_true")
    parser.add_argument("--include-hook-events", action="store_true")
    parser.add_argument("--verbose", action="store_true")
    parser.add_argument(
        "--permission-mode",
        choices=("default", "acceptEdits", "plan", "bypassPermissions"),
    )
    parser.add_argument("--allowed-tool", action="append", dest="allowed_tools")
    parser.add_argument("--disallowed-tool", action="append", dest="disallowed_tools")
    parser.add_argument("--require-asp-bash-commands", action="store_true")
    parser.add_argument("--max-asp-bash-commands", type=int)
    parser.add_argument("--settings")
    parser.add_argument("--claude-cwd")
    parser.add_argument("--add-dir", action="append", dest="add_dirs")
    parser.add_argument("--add-cwd-dir", action="store_true")
    parser.add_argument("--max-turns", type=int)


def _add_session_metadata_arguments(parser: argparse.ArgumentParser) -> None:
    parser.add_argument(
        "--session-id",
        default="agent-session",
        help="Session id for agent-session observability commands.",
    )
    parser.add_argument(
        "--agent",
        default="claude-sdk",
        choices=("claude-sdk", "claude-cli", "codex", "fixture", "unknown"),
        help="Agent source for agent-session receipts.",
    )
    parser.add_argument(
        "--model",
        help="Optional model id for agent-session receipts and live Claude SDK runs.",
    )


def _add_analysis_arguments(parser: argparse.ArgumentParser) -> None:
    parser.add_argument(
        "--analyze-recorded-agent-session",
        action="store_true",
        help=(
            "After recording an agent session, also write quality and "
            "graph-turbo feedback reports."
        ),
    )
    parser.add_argument(
        "--analyzer",
        action="store_true",
        help=(
            "Alias for --analyze-recorded-agent-session on record/import runs. "
            "Writes analyzer reports without changing normal ASP search output."
        ),
    )
    parser.add_argument(
        "--build-agent-session-receipt",
        metavar="SESSION_ROOT",
        help="Build an agent-session receipt from a recorded session root.",
    )
    parser.add_argument(
        "--analyze-agent-session",
        metavar="SESSION_ROOT_OR_RECEIPT",
        help="Analyze an agent-session root or receipt JSON file.",
    )
    parser.add_argument(
        "--quality-report",
        help="Output path for --analyze-agent-session quality report.",
    )
    parser.add_argument(
        "--graph-turbo-feedback",
        help="Output path for --analyze-agent-session graph-turbo feedback.",
    )
    parser.add_argument(
        "--improvement-report",
        help="Output path for --analyze-agent-session improvement report.",
    )
    parser.add_argument(
        "--algorithm-graph-feedback",
        help=(
            "Output path for graph-turbo algorithm feedback generated from "
            "the improvement report."
        ),
    )
    parser.add_argument(
        "--algorithm-calibration",
        help=(
            "Output path for graph-turbo profile calibration proposal generated "
            "from algorithm feedback."
        ),
    )


def handle_agent_session_args(repo_root: Path, args: Any) -> int | None:
    if getattr(args, "record_agent_session", False):
        return _record_live_session(repo_root, args)
    messages_arg = getattr(args, "record_agent_session_from_messages", None)
    if messages_arg:
        return _record_from_messages(repo_root, args, messages_arg)
    session_root_arg = getattr(args, "build_agent_session_receipt", None)
    if session_root_arg:
        return _build_receipt(repo_root, args, session_root_arg)
    analyze_arg = getattr(args, "analyze_agent_session", None)
    if analyze_arg:
        return _analyze_session(repo_root, args, analyze_arg)
    return None


def _record_live_session(repo_root: Path, args: Any) -> int:
    if not args.prompt:
        raise SystemExit("--prompt is required with --record-agent-session")
    session_root = _session_root(repo_root, args)
    session_root.mkdir(parents=True, exist_ok=True)
    command = _agent_session_command(args)
    completed = _run_agent_session_command(command, cwd=repo_root)
    _write_text(session_root / "sdk-stdout.jsonl", completed.stdout)
    if completed.stderr:
        _write_text(session_root / "sdk-stderr.txt", completed.stderr)
    messages = load_stdout_messages(completed.stdout)
    config = _config(args, session_root)
    manifest = write_agent_session_from_messages(messages, session_root, config=config)
    receipt_path = session_root / "receipts" / "agent-session-receipt.json"
    receipt = write_agent_session_receipt(session_root, receipt_path, config=config)
    payload = {
        "sessionId": manifest["sessionId"],
        "eventCount": manifest["eventCount"],
        "receiptPath": str(receipt_path),
        "commandCount": receipt["summary"]["commandCount"],
        "exitCode": completed.returncode,
    }
    _attach_record_analysis(repo_root, args, receipt_path, payload)
    _emit_record_payload(args, payload)
    return completed.returncode


def _record_from_messages(repo_root: Path, args: Any, messages_arg: str) -> int:
    messages_path = _resolve_existing_path(repo_root, messages_arg)
    session_root = _session_root(repo_root, args)
    messages = load_agent_messages(messages_path)
    config = _config(args, session_root)
    manifest = write_agent_session_from_messages(messages, session_root, config=config)
    receipt_path = session_root / "receipts" / "agent-session-receipt.json"
    receipt = write_agent_session_receipt(session_root, receipt_path, config=config)
    payload = {
        "sessionId": manifest["sessionId"],
        "eventCount": manifest["eventCount"],
        "receiptPath": str(receipt_path),
        "commandCount": receipt["summary"]["commandCount"],
    }
    _attach_record_analysis(repo_root, args, receipt_path, payload)
    _emit_record_payload(args, payload)
    return 0


def _build_receipt(repo_root: Path, args: Any, session_root_arg: str) -> int:
    session_root = _resolve_existing_path(repo_root, session_root_arg)
    output_path = _output_path(
        repo_root,
        args,
        session_root / "receipts" / "agent-session-receipt.json",
    )
    receipt = write_agent_session_receipt(session_root, output_path)
    if args.json:
        emit_json(receipt)
    else:
        emit(
            "[agent-session-receipt] "
            f"session={receipt['sessionId']} commands={receipt['summary']['commandCount']} "
            f"output={output_path}"
        )
    return 0


def _analyze_session(repo_root: Path, args: Any, analyze_arg: str) -> int:
    target = _resolve_existing_path(repo_root, analyze_arg)
    receipt_path = (
        target
        if target.is_file()
        else target / "receipts" / "agent-session-receipt.json"
    )
    (
        report_path,
        feedback_path,
        improvement_path,
        algorithm_feedback_path,
        algorithm_calibration_path,
    ) = _analysis_paths(repo_root, args, receipt_path)
    quality, feedback, improvement = write_agent_session_analysis(
        receipt_path,
        report_path,
        feedback_path,
        improvement_path,
        algorithm_feedback_path,
        algorithm_calibration_path,
    )
    if args.json:
        emit_json(
            {
                "quality": quality,
                "graphTurboFeedback": feedback,
                "improvementReport": improvement,
                "algorithmGraphFeedbackPath": str(algorithm_feedback_path),
                "algorithmCalibrationPath": str(algorithm_calibration_path),
            }
        )
    else:
        emit(
            "[agent-session-analysis] "
            f"session={quality['sessionId']} findings={len(quality['findings'])} "
            f"candidates={len(feedback['candidates'])} "
            f"improvements={len(improvement['improvementPoints'])}"
        )
    return 0


def _attach_record_analysis(
    repo_root: Path,
    args: Any,
    receipt_path: Path,
    payload: dict[str, Any],
) -> None:
    if not (args.analyze_recorded_agent_session or args.analyzer):
        return
    (
        report_path,
        feedback_path,
        improvement_path,
        algorithm_feedback_path,
        algorithm_calibration_path,
    ) = _analysis_paths(repo_root, args, receipt_path)
    quality, feedback, improvement = write_agent_session_analysis(
        receipt_path,
        report_path,
        feedback_path,
        improvement_path,
        algorithm_feedback_path,
        algorithm_calibration_path,
    )
    payload["qualityReportPath"] = str(report_path)
    payload["graphTurboFeedbackPath"] = str(feedback_path)
    payload["improvementReportPath"] = str(improvement_path)
    payload["algorithmGraphFeedbackPath"] = str(algorithm_feedback_path)
    payload["algorithmCalibrationPath"] = str(algorithm_calibration_path)
    payload["findingCount"] = len(quality["findings"])
    payload["feedbackCandidateCount"] = len(feedback["candidates"])
    payload["improvementPointCount"] = len(improvement["improvementPoints"])


def _emit_record_payload(args: Any, payload: dict[str, Any]) -> None:
    if args.json:
        emit_json(payload)
        return
    parts = [
        "[agent-session-record]",
        f"session={payload['sessionId']}",
        f"events={payload['eventCount']}",
        f"commands={payload['commandCount']}",
    ]
    if "exitCode" in payload:
        parts.append(f"exit={payload['exitCode']}")
    if "findingCount" in payload:
        parts.append(f"findings={payload['findingCount']}")
        parts.append(f"candidates={payload['feedbackCandidateCount']}")
        parts.append(f"improvements={payload['improvementPointCount']}")
    parts.append(f"receipt={payload['receiptPath']}")
    emit(" ".join(parts))


def _analysis_paths(
    repo_root: Path,
    args: Any,
    receipt_path: Path,
) -> tuple[Path, Path, Path, Path, Path]:
    report_path = _resolve_or_default(
        repo_root,
        args.quality_report,
        receipt_path.parent.parent / "reports" / "quality-report.json",
    )
    feedback_path = _resolve_or_default(
        repo_root,
        args.graph_turbo_feedback,
        receipt_path.parent.parent / "reports" / "graph-turbo-feedback.json",
    )
    improvement_path = _resolve_or_default(
        repo_root,
        args.improvement_report,
        receipt_path.parent.parent / "reports" / "improvement-report.json",
    )
    algorithm_feedback_path = _resolve_or_default(
        repo_root,
        args.algorithm_graph_feedback,
        receipt_path.parent.parent / "reports" / "algorithm-graph-feedback.json",
    )
    algorithm_calibration_path = _resolve_or_default(
        repo_root,
        args.algorithm_calibration,
        receipt_path.parent.parent / "reports" / "algorithm-calibration.json",
    )
    return (
        report_path,
        feedback_path,
        improvement_path,
        algorithm_feedback_path,
        algorithm_calibration_path,
    )


def _config(args: Any, session_root: Path) -> AgentSessionConfig:
    return AgentSessionConfig(
        session_id=args.session_id,
        scenario_id=args.scenario_id,
        language=args.language,
        project_name=args.project_name,
        project_source=args.project_source,
        intent=args.intent,
        agent=args.agent,
        model=args.model,
        edit_boundary=args.edit_boundary,
        project_workdir=_project_workdir(args, session_root),
    )


def _agent_session_command(args: Any) -> list[str]:
    command = [
        sys.executable,
        "-m",
        "tools.semantic_sandtable.claude_sdk_runner",
        "--prompt",
        args.prompt,
        "--output-format",
        args.agent_output_format,
    ]
    _append_flag(command, "--include-partial-messages", args.include_partial_messages)
    _append_flag(command, "--include-hook-events", args.include_hook_events)
    _append_flag(command, "--verbose", args.verbose)
    _append_flag(command, "--require-asp-bash-commands", args.require_asp_bash_commands)
    _append_flag(command, "--add-cwd-dir", args.add_cwd_dir)
    _append_option(command, "--model", args.model)
    _append_option(command, "--permission-mode", args.permission_mode)
    _append_option(command, "--max-asp-bash-commands", args.max_asp_bash_commands)
    _append_option(command, "--settings", args.settings)
    _append_option(command, "--claude-cwd", args.claude_cwd)
    _append_option(command, "--max-turns", args.max_turns)
    _append_repeated_option(command, "--allowed-tool", args.allowed_tools)
    _append_repeated_option(command, "--disallowed-tool", args.disallowed_tools)
    _append_repeated_option(command, "--add-dir", args.add_dirs)
    return command


def _run_agent_session_command(
    command: list[str],
    *,
    cwd: Path,
) -> subprocess.CompletedProcess[str]:
    return subprocess.run(
        command,
        cwd=str(cwd),
        capture_output=True,
        text=True,
        check=False,
    )


def _append_flag(command: list[str], option: str, enabled: bool) -> None:
    if enabled:
        command.append(option)


def _append_option(command: list[str], option: str, value: Any) -> None:
    if value is not None:
        command.extend([option, str(value)])


def _append_repeated_option(
    command: list[str],
    option: str,
    values: list[str] | None,
) -> None:
    for value in values or []:
        command.extend([option, str(value)])


def _project_workdir(args: Any, session_root: Path) -> str:
    if args.claude_cwd:
        return str(Path(args.claude_cwd).expanduser())
    return str(session_root.parent)


def _session_root(repo_root: Path, args: Any) -> Path:
    if args.agent_session_root:
        return _resolve_existing_or_new_path(repo_root, args.agent_session_root)
    return (
        repo_root
        / ".cache"
        / "agent-semantic-protocol"
        / "sandtable-sessions"
        / args.session_id
    ).resolve()


def _output_path(repo_root: Path, args: Any, default: Path) -> Path:
    return _resolve_or_default(repo_root, getattr(args, "output", None), default)


def _resolve_or_default(repo_root: Path, value: str | None, default: Path) -> Path:
    if value:
        return _resolve_existing_or_new_path(repo_root, value)
    return default.resolve()


def _resolve_existing_path(repo_root: Path, value: str) -> Path:
    path = resolve_path(repo_root, value)
    if path is None:
        raise SystemExit(f"invalid path: {value}")
    return path


def _resolve_existing_or_new_path(repo_root: Path, value: str) -> Path:
    path = resolve_path(repo_root, value)
    if path is None:
        raise SystemExit(f"invalid path: {value}")
    return path


def _write_text(path: Path, value: str) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(value, encoding="utf-8")
