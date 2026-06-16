"""Signal extraction for adaptive large-library simulation reports."""

from __future__ import annotations

import re
import shlex
from typing import Any


def _third_step_signals(results: list[dict[str, Any]]) -> dict[str, str]:
    return _step_signals(results, 2)


def _final_step_signals(results: list[dict[str, Any]]) -> dict[str, str]:
    if len(results) < 3:
        return _empty_step_signals()
    return _step_signals(results, min(len(results) - 1, 3))


def _recovery_probe_signals(results: list[dict[str, Any]]) -> dict[str, str]:
    if len(results) < 5:
        return _empty_step_signals()
    return _step_signals(results, 4)


def _step_signals(results: list[dict[str, Any]], index: int) -> dict[str, str]:
    if len(results) <= index:
        return _empty_step_signals()
    result = results[index]
    argv = [str(value) for value in result.get("argv", [])]
    stdout = str(result.get("stdout", ""))
    action = _command_action(argv)
    recovery = _owner_items_recovery(action, stdout)
    recommended_next = _line_value(stdout, "recommendedNext")
    next_command_class = _next_command_class(stdout)
    return {
        "action": action,
        "ownerItemsRecovery": recovery,
        "ownerItemsTransition": _owner_items_transition(
            action, recovery, recommended_next, next_command_class
        ),
        "selectorQuality": _selector_quality(action, recovery, argv, stdout),
        "recommendedNext": recommended_next,
        "nextCommandClass": next_command_class,
    }


def _empty_step_signals() -> dict[str, str]:
    return {
        "action": "not-run",
        "ownerItemsRecovery": "not-run",
        "ownerItemsTransition": "not-run",
        "selectorQuality": "not-selector-ready",
        "recommendedNext": "unknown",
        "nextCommandClass": "none",
    }


def _command_action(argv: list[str]) -> str:
    if _argv_contains_sequence(argv, ["search", "owner"]) and "items" in argv:
        return "owner-items"
    if len(argv) > 1 and argv[1] == "fd":
        return "fd-query"
    if len(argv) > 1 and argv[1] == "rg":
        return "rg-query"
    if _argv_contains_sequence(argv, ["query"]):
        return "query-code"
    return "other"


def _next_command_class(stdout: str) -> str:
    command = _line_payload(stdout, "nextCommand")
    if command == "unknown":
        return "none"
    try:
        return _command_action(shlex.split(command))
    except ValueError:
        return "invalid"


def _should_follow_after_third(signals: dict[str, str]) -> bool:
    if (
        signals["action"] == "owner-items"
        and signals["ownerItemsRecovery"] == "selector-ready"
    ):
        return False
    return signals["nextCommandClass"] not in {"none", "invalid", "query-code"}


def _should_probe_after_final(signals: dict[str, str]) -> bool:
    return signals["ownerItemsTransition"] == "owner-items-ready"


def _command_was_run(command: list[str], results: list[dict[str, Any]]) -> bool:
    return any(
        [str(value) for value in result.get("argv", [])] == command
        for result in results
    )


def _owner_items_recovery(action: str, stdout: str) -> str:
    if action != "owner-items":
        return "not-owner-items"
    if "reason=owner-item-selector-ready" in stdout:
        return "selector-ready"
    if "reason=no-owner-item-match" in stdout:
        if (
            "recommendedNext=scoped-rg-query" in stdout
            and "nextCommand=asp rg -query" in stdout
        ):
            return "scoped-rg-query"
        return "no-hit-unscoped"
    return "owner-items-unknown"


def _owner_items_transition(
    action: str,
    recovery: str,
    recommended_next: str,
    next_command_class: str,
) -> str:
    if recovery == "selector-ready":
        return "selector-ready"
    if (
        recommended_next in {"A1.owner-items", "owner-items"}
        or next_command_class == "owner-items"
    ):
        return "owner-items-ready"
    if action == "owner-items":
        return "owner-items-miss"
    if next_command_class in {"fd-query", "rg-query"}:
        return "search-refine"
    return "not-ready"


def _selector_quality(
    action: str,
    recovery: str,
    argv: list[str],
    stdout: str,
) -> str:
    if action != "owner-items" or recovery != "selector-ready":
        return "not-selector-ready"
    owner = _owner_arg(argv)
    if _secondary_artifact_owner(owner):
        return "secondary-artifact-selector"
    query_terms = _query_terms(_query_arg(argv))
    matched = _matched_query_terms(
        query_terms,
        owner,
        _line_payload(stdout, "nextCommand"),
        stdout,
    )
    if not matched:
        return "weak-query-axis-selector"
    missing = [term for term in query_terms if term not in set(matched)]
    if len(missing) > len(matched):
        return "partial-query-axis-selector"
    return "source-selector"


def _argv_contains_sequence(argv: list[str], sequence: list[str]) -> bool:
    if not sequence:
        return True
    return any(argv[index : index + len(sequence)] == sequence for index in range(len(argv)))


def _owner_arg(argv: list[str]) -> str:
    try:
        search_index = argv.index("search")
        if argv[search_index + 1] == "owner":
            return argv[search_index + 2]
    except (ValueError, IndexError):
        pass
    return "unknown"


def _query_arg(argv: list[str]) -> str:
    try:
        return argv[argv.index("--query") + 1]
    except (ValueError, IndexError):
        return "unknown"


def _selector_arg(command: str) -> str:
    try:
        argv = shlex.split(command)
        return argv[argv.index("--selector") + 1]
    except (ValueError, IndexError):
        return "unknown"


def _query_terms(query: str) -> list[str]:
    terms: list[str] = []
    for raw in re.split(r"[|,\s]+", query):
        term = raw.strip().lower()
        if term and term not in terms:
            terms.append(term)
    return terms


def _matched_query_terms(
    query_terms: list[str],
    owner: str,
    next_command: str,
    stdout: str,
) -> list[str]:
    evidence = "\n".join([owner, next_command, _selector_evidence(stdout)]).lower()
    return [term for term in query_terms if term in evidence]


def _selector_evidence(stdout: str) -> str:
    return "\n".join(
        line
        for line in stdout.splitlines()
        if line.startswith("syntax ") or " selector=" in line or " pattern=" in line
    )


def _secondary_artifact_owner(owner: str) -> bool:
    return any(
        _secondary_artifact_token(part.lower()) for part in re.split(r"[/\\._-]+", owner)
    )


def _secondary_artifact_token(token: str) -> bool:
    return token in {
        "test",
        "tests",
        "spec",
        "specs",
        "fixture",
        "fixtures",
        "baseline",
        "baselines",
        "case",
        "cases",
        "template",
        "templates",
        "example",
        "examples",
        "sample",
        "samples",
        "demo",
        "demos",
        "bench",
        "benches",
        "benchmark",
        "benchmarks",
        "unittest",
        "unittests",
    }


def _line_value(text: str, key: str) -> str:
    match = re.search(rf"^{re.escape(key)}=([^\s]+)", text, flags=re.MULTILINE)
    return match.group(1) if match else "unknown"


def _line_payload(text: str, key: str) -> str:
    for line in text.splitlines():
        if line.startswith(f"{key}="):
            return line.split("=", 1)[1].strip()
    return "unknown"
