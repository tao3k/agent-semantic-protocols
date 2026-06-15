"""Build turn-level quality details from visible agent-session events."""

from __future__ import annotations

from typing import Any

from .agent_observation_asp import normalize_command
from .direct_read_shape import direct_source_read_shape
from .utils import dict_value, list_value, optional_int, require_str


def agent_session_turn_details(
    receipt: dict[str, Any],
    events: list[dict[str, Any]],
    findings: list[dict[str, Any]],
) -> list[dict[str, Any]]:
    commands = list_value(receipt.get("commands"))
    repeated_command_ids = _repeated_command_ids(commands)
    finding_ids = {str(item.get("id")) for item in findings if isinstance(item, dict)}
    by_id = _commands_by_id(commands)
    details = (
        _event_turn_details(receipt, events, by_id, repeated_command_ids, finding_ids)
        if events
        else _receipt_turn_details(receipt, commands, repeated_command_ids, finding_ids)
    )
    return _renumber(details)


def agent_session_turn_summary(
    turn_details: list[dict[str, Any]],
) -> dict[str, Any]:
    phase_counts: dict[str, int] = {}
    signal_counts: dict[str, int] = {}
    finding_linked = 0
    for detail in turn_details:
        phase = str(detail.get("phase", "unknown"))
        phase_counts[phase] = phase_counts.get(phase, 0) + 1
        for signal in list_value(detail.get("qualitySignals")):
            key = str(signal)
            signal_counts[key] = signal_counts.get(key, 0) + 1
        if list_value(detail.get("findingIds")):
            finding_linked += 1
    return {
        "totalTurns": len(turn_details),
        "phaseCounts": dict(sorted(phase_counts.items())),
        "qualitySignalCounts": dict(sorted(signal_counts.items())),
        "findingLinkedTurns": finding_linked,
    }


def agent_session_round_details(
    turn_details: list[dict[str, Any]],
) -> list[dict[str, Any]]:
    grouped = _group_command_turns(turn_details)
    rounds = [
        _round_detail(command_id, turns)
        for command_id, turns in sorted(
            grouped.items(),
            key=lambda item: _min_ordinal(item[1]),
        )
    ]
    for index, item in enumerate(rounds):
        item["ordinal"] = index
    return rounds


def agent_session_round_summary(
    round_details: list[dict[str, Any]],
) -> dict[str, Any]:
    return {
        "totalRounds": len(round_details),
        "commandKindCounts": _count_round_command_kinds(round_details),
        "qualitySignalCounts": _count_round_quality_signals(round_details),
        "findingLinkedRounds": _count_finding_linked_rounds(round_details),
        "deniedRounds": _count_rounds_with_status(round_details, "denied"),
        "riskRounds": _count_rounds_with_signal(round_details, "direct-read-risk"),
        "repeatedRounds": _count_rounds_with_signal(
            round_details,
            "repeated-command",
        ),
    }


def _event_turn_details(
    receipt: dict[str, Any],
    events: list[dict[str, Any]],
    commands_by_id: dict[str, dict[str, Any]],
    repeated_command_ids: set[str],
    finding_ids: set[str],
) -> list[dict[str, Any]]:
    details = []
    has_answer = False
    for event in events:
        detail = _detail_from_event(
            receipt,
            event,
            commands_by_id,
            repeated_command_ids,
            finding_ids,
        )
        if detail:
            has_answer = has_answer or detail["phase"] == "answer"
            details.append(detail)
    if not has_answer:
        details.append(_answer_detail(receipt, finding_ids, ordinal=len(details)))
    return details


def _receipt_turn_details(
    receipt: dict[str, Any],
    commands: list[Any],
    repeated_command_ids: set[str],
    finding_ids: set[str],
) -> list[dict[str, Any]]:
    details = []
    for command in commands:
        if isinstance(command, dict):
            details.append(
                _command_detail(
                    command,
                    repeated_command_ids,
                    finding_ids,
                    ordinal=len(details),
                )
            )
    details.append(_answer_detail(receipt, finding_ids, ordinal=len(details)))
    return details


def _detail_from_event(
    receipt: dict[str, Any],
    event: dict[str, Any],
    commands_by_id: dict[str, dict[str, Any]],
    repeated_command_ids: set[str],
    finding_ids: set[str],
) -> dict[str, Any] | None:
    phase = _event_phase(event)
    if not phase:
        return None
    detail = _base_event_detail(event, phase)
    command_id = detail.get("commandId")
    command = (
        commands_by_id.get(command_id) if isinstance(command_id, str) else None
    ) or _command_from_event(event)
    if command:
        _attach_command_fields(detail, command, repeated_command_ids, finding_ids)
        if phase == "command-start":
            detail["qualitySignals"] = ["command-started"]
        elif phase != "command-result":
            detail["qualitySignals"] = _event_signals(event, receipt)
    else:
        detail["qualitySignals"] = _event_signals(event, receipt)
    if phase == "answer":
        _attach_answer_signals(detail, receipt, finding_ids)
    elif phase == "session":
        detail["qualitySignals"].extend(_session_signals(event))
    detail["findingIds"] = _finding_ids_for_signals(
        detail["qualitySignals"],
        finding_ids,
    )
    return _without_empty(detail)


def _base_event_detail(event: dict[str, Any], phase: str) -> dict[str, Any]:
    detail: dict[str, Any] = {
        "id": require_str(event, "eventId", f"turn-{event.get('ordinal', 0)}"),
        "ordinal": optional_int(event.get("ordinal")) or 0,
        "phase": phase,
        "eventIds": [require_str(event, "eventId", "")],
        "qualitySignals": [],
    }
    for source, target in (
        ("messageIndex", "messageIndex"),
        ("toolUseId", "toolUseId"),
        ("commandId", "commandId"),
        ("preview", "preview"),
    ):
        if event.get(source) is not None:
            detail[target] = event[source]
    return detail


def _command_detail(
    command: dict[str, Any],
    repeated_command_ids: set[str],
    finding_ids: set[str],
    *,
    ordinal: int,
) -> dict[str, Any]:
    detail = {
        "id": f"turn-{require_str(command, 'id', 'command')}",
        "ordinal": ordinal,
        "phase": "command-result",
        "qualitySignals": [],
    }
    if command.get("eventId"):
        detail["eventIds"] = [str(command["eventId"])]
    _attach_command_fields(detail, command, repeated_command_ids, finding_ids)
    detail["findingIds"] = _finding_ids_for_signals(
        detail["qualitySignals"],
        finding_ids,
    )
    return _without_empty(detail)


def _answer_detail(
    receipt: dict[str, Any],
    finding_ids: set[str],
    *,
    ordinal: int,
) -> dict[str, Any]:
    answer = dict_value(receipt.get("answer"))
    detail = {
        "id": "turn-answer",
        "ordinal": ordinal,
        "phase": "answer",
        "qualitySignals": [],
    }
    if answer.get("messageEventId"):
        detail["eventIds"] = [str(answer["messageEventId"])]
    if answer.get("messageIndex") is not None:
        detail["messageIndex"] = optional_int(answer.get("messageIndex")) or 0
    if answer.get("preview"):
        detail["preview"] = str(answer["preview"])
    _attach_answer_signals(detail, receipt, finding_ids)
    detail["findingIds"] = _finding_ids_for_signals(
        detail["qualitySignals"],
        finding_ids,
    )
    return _without_empty(detail)


def _attach_command_fields(
    detail: dict[str, Any],
    command: dict[str, Any],
    repeated_command_ids: set[str],
    finding_ids: set[str],
) -> None:
    command_id = require_str(command, "id", require_str(command, "commandId", ""))
    argv = [str(item) for item in list_value(command.get("argv"))]
    detail["commandId"] = command_id
    detail["commandKind"] = _command_kind(command, argv)
    if argv:
        detail["argv"] = argv
    metrics = _turn_metrics(dict_value(command.get("metrics")))
    if metrics:
        detail["metrics"] = metrics
    detail["qualitySignals"] = _command_signals(
        command,
        argv,
        repeated_command_ids,
        finding_ids,
    )


def _attach_answer_signals(
    detail: dict[str, Any],
    receipt: dict[str, Any],
    finding_ids: set[str],
) -> None:
    answer = dict_value(receipt.get("answer"))
    signals = detail["qualitySignals"]
    if not answer.get("present"):
        signals.append("answer-missing")
    elif answer.get("groundingStatus") == "grounded":
        signals.append("answer-grounded")
    else:
        signals.append("answer-weak")
    if "answer.missing" in finding_ids and "answer-missing" not in signals:
        signals.append("answer-missing")
    if "answer.weak-grounding" in finding_ids and "answer-weak" not in signals:
        signals.append("answer-weak")


def _event_phase(event: dict[str, Any]) -> str:
    return {
        "user.intent": "intent",
        "assistant.visible-message": "assistant-message",
        "tool.request": "tool-request",
        "tool.result": "tool-result",
        "command.start": "command-start",
        "command.result": "command-result",
        "answer.final": "answer",
        "session.start": "session",
        "session.stop": "session",
    }.get(str(event.get("kind")), "")


def _event_signals(event: dict[str, Any], receipt: dict[str, Any]) -> list[str]:
    kind = event.get("kind")
    if kind == "user.intent":
        return ["intent-recorded"]
    if kind == "assistant.visible-message":
        return ["assistant-visible"]
    if kind == "tool.request":
        return ["tool-requested"]
    if kind == "tool.result":
        return ["tool-result-observed"]
    if kind == "answer.final":
        answer = dict_value(receipt.get("answer"))
        return (
            ["answer-grounded"]
            if answer.get("groundingStatus") == "grounded"
            else ["answer-weak"]
        )
    return []


def _session_signals(event: dict[str, Any]) -> list[str]:
    fields = dict_value(event.get("fields"))
    status = fields.get("status")
    if status == "complete":
        return ["session-complete"]
    if status == "partial":
        return ["session-partial"]
    return []


def _command_signals(
    command: dict[str, Any],
    argv: list[str],
    repeated_command_ids: set[str],
    finding_ids: set[str],
) -> list[str]:
    signals = ["command-recorded"]
    kind = _command_kind(command, argv)
    if kind == "search" and "prime" in argv:
        signals.append("search-prime")
    elif kind == "search":
        signals.append("search-followup")
    elif kind == "query":
        signals.append("query-selector")
    elif kind == "check":
        signals.append("check-command")
    elif kind == "guide":
        signals.append("guide-command")
    if command.get("denied") or kind == "hook-deny":
        signals.append("hook-denied")
    command_id = require_str(command, "id", require_str(command, "commandId", ""))
    if command_id in repeated_command_ids:
        signals.append("repeated-command")
    if _is_direct_read_risk(argv):
        signals.append("direct-read-risk")
    if "search.missing-prime" in finding_ids and kind == "search":
        signals.append("search-followup")
    return _dedupe(signals)


def _finding_ids_for_signals(
    signals: list[str],
    finding_ids: set[str],
) -> list[str]:
    mapping = {
        "repeated-command": "command.repeated",
        "direct-read-risk": "read.direct-risk",
        "hook-denied": "hook.denied",
        "answer-missing": "answer.missing",
        "answer-weak": "answer.weak-grounding",
        "search-followup": "search.missing-prime",
    }
    return [
        finding_id
        for signal, finding_id in mapping.items()
        if signal in signals and finding_id in finding_ids
    ]


def _commands_by_id(commands: list[Any]) -> dict[str, dict[str, Any]]:
    by_id = {}
    for command in commands:
        if isinstance(command, dict):
            by_id[require_str(command, "id", "")] = command
    return by_id


def _command_from_event(event: dict[str, Any]) -> dict[str, Any]:
    fields = dict_value(event.get("fields"))
    argv = [str(item) for item in list_value(fields.get("argv"))]
    if not event.get("commandId") and not argv:
        return {}
    command = {
        "id": require_str(event, "commandId", require_str(event, "eventId", "")),
        "argv": argv,
        "metrics": _event_metrics(fields),
    }
    if fields.get("denied"):
        command["denied"] = True
    return command


def _event_metrics(fields: dict[str, Any]) -> dict[str, int]:
    return {
        "elapsedMs": optional_int(fields.get("elapsedMs")) or 0,
        "stdoutBytes": optional_int(fields.get("stdoutBytes")) or 0,
        "stderrBytes": optional_int(fields.get("stderrBytes")) or 0,
        "stdoutLines": optional_int(fields.get("stdoutLines")) or 0,
        "stderrLines": optional_int(fields.get("stderrLines")) or 0,
    }


def _turn_metrics(metrics: dict[str, Any]) -> dict[str, int]:
    result = {}
    for key in ("stdoutBytes", "stderrBytes", "stdoutLines", "stderrLines", "elapsedMs"):
        value = optional_int(metrics.get(key))
        if value is not None:
            result[key] = value
    return result


def _command_kind(command: dict[str, Any], argv: list[str]) -> str:
    kind = command.get("kind")
    if isinstance(kind, str) and kind:
        return kind
    for surface in ("search", "query", "check", "guide"):
        if surface in argv:
            return surface
    if command.get("denied"):
        return "hook-deny"
    return "other"


def _repeated_command_ids(commands: list[Any]) -> set[str]:
    normalized_by_id = {}
    counts: dict[str, int] = {}
    for command in commands:
        if not isinstance(command, dict):
            continue
        command_id = require_str(command, "id", "")
        normalized = normalize_command(
            " ".join(str(item) for item in list_value(command.get("argv")))
        )
        normalized_by_id[command_id] = normalized
        counts[normalized] = counts.get(normalized, 0) + 1
    return {
        command_id
        for command_id, normalized in normalized_by_id.items()
        if counts.get(normalized, 0) > 1
    }


def _is_direct_read_risk(argv: list[str]) -> bool:
    return "direct-source-read" in argv and direct_source_read_shape(argv) in {
        "broad",
        "unbounded",
    }


def _group_command_turns(
    turn_details: list[dict[str, Any]],
) -> dict[str, list[dict[str, Any]]]:
    grouped: dict[str, list[dict[str, Any]]] = {}
    for detail in turn_details:
        command_id = detail.get("commandId")
        if isinstance(command_id, str) and command_id:
            grouped.setdefault(command_id, []).append(detail)
    return grouped


def _count_round_command_kinds(round_details: list[dict[str, Any]]) -> dict[str, int]:
    counts: dict[str, int] = {}
    for item in round_details:
        key = str(item.get("commandKind", "other"))
        counts[key] = counts.get(key, 0) + 1
    return dict(sorted(counts.items()))


def _count_round_quality_signals(
    round_details: list[dict[str, Any]],
) -> dict[str, int]:
    counts: dict[str, int] = {}
    for item in round_details:
        for signal in list_value(item.get("qualitySignals")):
            key = str(signal)
            counts[key] = counts.get(key, 0) + 1
    return dict(sorted(counts.items()))


def _count_finding_linked_rounds(round_details: list[dict[str, Any]]) -> int:
    return sum(1 for item in round_details if list_value(item.get("findingIds")))


def _count_rounds_with_status(
    round_details: list[dict[str, Any]],
    status: str,
) -> int:
    return sum(1 for item in round_details if item.get("resultStatus") == status)


def _count_rounds_with_signal(
    round_details: list[dict[str, Any]],
    signal: str,
) -> int:
    return sum(
        1
        for item in round_details
        if signal in [str(value) for value in list_value(item.get("qualitySignals"))]
    )


def _round_detail(command_id: str, turns: list[dict[str, Any]]) -> dict[str, Any]:
    command_turn = _command_result_turn(turns) or turns[-1]
    signals = _round_values(turns, "qualitySignals")
    finding_ids = _round_values(turns, "findingIds")
    detail = {
        "id": f"round-{command_id}",
        "ordinal": _min_ordinal(turns),
        "commandId": command_id,
        "commandKind": str(command_turn.get("commandKind", "other")),
        "qualitySignals": signals,
        "resultStatus": _round_status(signals),
    }
    for key in ("argv", "metrics", "toolUseId"):
        value = command_turn.get(key)
        if value not in (None, [], {}):
            detail[key] = value
    event_ids = _round_values(turns, "eventIds")
    if event_ids:
        detail["eventIds"] = event_ids
    turn_ordinals = [
        value
        for value in (optional_int(turn.get("ordinal")) for turn in turns)
        if value is not None
    ]
    if turn_ordinals:
        detail["turnOrdinals"] = sorted(turn_ordinals)
    if finding_ids:
        detail["findingIds"] = finding_ids
    return _without_empty(detail)


def _command_result_turn(turns: list[dict[str, Any]]) -> dict[str, Any] | None:
    for turn in turns:
        if turn.get("phase") == "command-result":
            return turn
    return None


def _round_values(turns: list[dict[str, Any]], key: str) -> list[str]:
    values = []
    for turn in turns:
        for value in list_value(turn.get(key)):
            text = str(value)
            if text not in values:
                values.append(text)
    return values


def _round_status(signals: list[str]) -> str:
    if "hook-denied" in signals:
        return "denied"
    if (
        "direct-read-risk" in signals
        or "repeated-command" in signals
        or "search-followup" in signals
    ):
        return "warning"
    if "command-recorded" in signals:
        return "complete"
    return "unknown"


def _min_ordinal(turns: list[dict[str, Any]]) -> int:
    values = [
        value
        for value in (optional_int(turn.get("ordinal")) for turn in turns)
        if value is not None
    ]
    return min(values) if values else 0


def _renumber(details: list[dict[str, Any]]) -> list[dict[str, Any]]:
    for index, detail in enumerate(details):
        detail["ordinal"] = index
    return details


def _dedupe(values: list[str]) -> list[str]:
    result = []
    for value in values:
        if value not in result:
            result.append(value)
    return result


def _without_empty(detail: dict[str, Any]) -> dict[str, Any]:
    return {
        key: value
        for key, value in detail.items()
        if key == "qualitySignals" or value not in ("", [], {}, None)
    }
