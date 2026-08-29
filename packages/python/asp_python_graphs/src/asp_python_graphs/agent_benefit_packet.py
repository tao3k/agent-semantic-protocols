"""Agent-behavior benefit report for graph turbo ranking."""

from __future__ import annotations

from collections.abc import Mapping, Sequence

from .cli import _rank_packet
from .packet import result_to_packet
from .summary_packet import result_to_summary_packet

_EVIDENCE_KINDS = frozenset({"assert", "evidence", "hot", "test"})


def build_agent_benefit_report(
    packet: Mapping[str, object],
    *,
    scenario: str | None = None,
    receipt: Mapping[str, object] | None = None,
    rank_args: object,
    quality_config: Mapping[str, object] | None = None,
) -> dict[str, object]:
    result = _rank_packet(packet, rank_args)
    result_packet = result_to_packet(result)
    summary_packet = result_to_summary_packet(result)
    receipt_metrics = _mapping(_mapping(receipt).get("metrics"))
    report = {
        "schemaId": "agent.semantic-protocols.semantic-graph-turbo-agent-benefit",
        "schemaVersion": "1",
        "protocolId": "agent.semantic-protocols.semantic-language",
        "protocolVersion": "1",
        "packetKind": "graph-turbo-agent-benefit",
        "scenario": scenario or str(packet.get("profile") or "graph-turbo"),
        "profile": result_packet.get("profile"),
        "sourceReading": _source_reading(result_packet, receipt_metrics),
        "usefulLocator": _useful_locator(summary_packet, receipt),
        "failureEvidence": _failure_evidence(summary_packet),
        "feedbackLearning": _feedback_learning(result_packet),
        "profileMatrixExplanation": _profile_matrix_explanation(summary_packet),
    }
    report["qualityGate"] = _quality_gate(report, quality_config or {})
    return report


def _source_reading(
    result_packet: Mapping[str, object],
    receipt_metrics: Mapping[str, object],
) -> dict[str, object]:
    metrics = _mapping(result_packet.get("algorithmMetrics"))
    return {
        "directCodeActionCount": _int_value(
            metrics.get("readLoopDirectCodeActionCount")
        ),
        "rawReadFallbackCount": _int_value(receipt_metrics.get("rawReadFallbackCount")),
        "duplicateSelectorCount": _int_value(
            receipt_metrics.get("duplicateSelectorCount")
        ),
        "sameOwnerScanCount": _int_value(receipt_metrics.get("sameOwnerScanCount")),
        "readMemorySuppressedCount": _int_value(
            metrics.get("readMemorySuppressedCount")
        ),
        "secondPassSuppressedCount": _int_value(
            metrics.get("readLoopSecondPassSuppressedCount")
        ),
        "duplicateSelectorSuppressedCount": _int_value(
            metrics.get("readLoopDuplicateSelectorSuppressedCount")
        ),
        "sameOwnerSuppressedCount": _int_value(
            metrics.get("readLoopSameOwnerSuppressedCount")
        ),
    }


def _useful_locator(
    summary_packet: Mapping[str, object],
    receipt: Mapping[str, object] | None,
) -> dict[str, object]:
    receipt = _mapping(receipt)
    receipt_metrics = _mapping(receipt.get("metrics"))
    selectors = _useful_selectors(receipt)
    ranked_nodes = _ranked_nodes(summary_packet)
    rank, node_id, selector = _first_locator_match(ranked_nodes, selectors)
    return {
        "found": rank is not None,
        "selector": selector,
        "nodeId": node_id,
        "rank": rank,
        "commandsToFirstUsefulLocator": receipt_metrics.get(
            "commandsToFirstUsefulLocator"
        ),
    }


def _failure_evidence(summary_packet: Mapping[str, object]) -> dict[str, object]:
    ranked_nodes = _ranked_nodes(summary_packet)
    for index, node in enumerate(ranked_nodes, start=1):
        kind = node.get("kind")
        if isinstance(kind, str) and kind in _EVIDENCE_KINDS:
            return {
                "found": True,
                "nodeId": node.get("id"),
                "kind": kind,
                "rank": index,
                "selector": node.get("selector"),
            }
    return {
        "found": False,
        "nodeId": None,
        "kind": None,
        "rank": None,
        "selector": None,
    }


def _feedback_learning(result_packet: Mapping[str, object]) -> dict[str, object]:
    metrics = _mapping(result_packet.get("algorithmMetrics"))
    receipt_boost = _int_value(metrics.get("receiptBoostCount"))
    receipt_penalty = _int_value(metrics.get("receiptPenaltyCount"))
    read_memory = _int_value(metrics.get("readMemorySuppressedCount"))
    duplicate_suppressed = _int_value(
        metrics.get("readLoopDuplicateSelectorSuppressedCount")
    )
    same_owner_suppressed = _int_value(metrics.get("readLoopSameOwnerSuppressedCount"))
    return {
        "receiptBoostCount": receipt_boost,
        "receiptPenaltyCount": receipt_penalty,
        "readMemorySuppressedCount": read_memory,
        "duplicateSelectorSuppressedCount": duplicate_suppressed,
        "sameOwnerSuppressedCount": same_owner_suppressed,
        "repeatedMistakeSuppressed": any(
            value > 0
            for value in (
                read_memory,
                duplicate_suppressed,
                same_owner_suppressed,
                receipt_penalty,
            )
        ),
        "receiptFeedbackApplied": receipt_boost > 0 or receipt_penalty > 0,
    }


def _profile_matrix_explanation(
    summary_packet: Mapping[str, object],
) -> dict[str, object]:
    matrix = _active_profile_matrix(summary_packet)
    explanations = _explanation_refs(summary_packet)
    channels = _top_relation_channels(matrix)
    return {
        "profile": matrix.get("profile"),
        "relationMatrixCount": matrix.get("relationMatrixCount"),
        "transitionNonZeroCount": matrix.get("transitionNonZeroCount"),
        "transitionWeightMass": matrix.get("transitionWeightMass"),
        "topRelationChannels": channels,
        "explanationRefs": explanations,
        "explained": bool(channels) and bool(explanations),
    }


def _quality_gate(
    report: Mapping[str, object],
    config: Mapping[str, object],
) -> dict[str, object]:
    thresholds = _thresholds(config)
    failures: list[dict[str, object]] = []
    source = _mapping(report.get("sourceReading"))
    locator = _mapping(report.get("usefulLocator"))
    failure = _mapping(report.get("failureEvidence"))
    feedback = _mapping(report.get("feedbackLearning"))
    matrix = _mapping(report.get("profileMatrixExplanation"))
    _check_lte(
        failures,
        "sourceReading.rawReadFallbackCount",
        source.get("rawReadFallbackCount"),
        thresholds["maxRawReadFallbackCount"],
    )
    _check_lte(
        failures,
        "sourceReading.duplicateSelectorCount",
        source.get("duplicateSelectorCount"),
        thresholds["maxDuplicateSelectorCount"],
    )
    _check_optional_lte(
        failures,
        "usefulLocator.commandsToFirstUsefulLocator",
        locator.get("commandsToFirstUsefulLocator"),
        thresholds.get("maxCommandsToFirstUsefulLocator"),
    )
    _check_optional_lte(
        failures,
        "usefulLocator.rank",
        locator.get("rank"),
        thresholds.get("maxUsefulLocatorRank"),
    )
    _check_required_bool(
        failures,
        "usefulLocator.found",
        locator.get("found"),
        thresholds["requireUsefulLocator"],
    )
    _check_optional_lte(
        failures,
        "failureEvidence.rank",
        failure.get("rank"),
        thresholds.get("maxFailureEvidenceRank"),
    )
    _check_required_bool(
        failures,
        "failureEvidence.found",
        failure.get("found"),
        thresholds["requireFailureEvidence"],
    )
    _check_required_bool(
        failures,
        "feedbackLearning.repeatedMistakeSuppressed",
        feedback.get("repeatedMistakeSuppressed"),
        thresholds["requireRepeatedMistakeSuppression"],
    )
    _check_required_bool(
        failures,
        "profileMatrixExplanation.explained",
        matrix.get("explained"),
        thresholds["requireProfileMatrixExplanation"],
    )
    return {
        "status": "pass" if not failures else "fail",
        "thresholds": thresholds,
        "failures": failures,
    }


def _thresholds(config: Mapping[str, object]) -> dict[str, object]:
    return {
        "maxRawReadFallbackCount": _int_config(config, "maxRawReadFallbackCount", 0),
        "maxDuplicateSelectorCount": _int_config(
            config, "maxDuplicateSelectorCount", 0
        ),
        "maxCommandsToFirstUsefulLocator": _optional_int_config(
            config, "maxCommandsToFirstUsefulLocator"
        ),
        "maxUsefulLocatorRank": _optional_int_config(config, "maxUsefulLocatorRank"),
        "maxFailureEvidenceRank": _optional_int_config(
            config, "maxFailureEvidenceRank"
        ),
        "requireUsefulLocator": config.get("requireUsefulLocator") is True,
        "requireFailureEvidence": config.get("requireFailureEvidence") is True,
        "requireRepeatedMistakeSuppression": (
            config.get("requireRepeatedMistakeSuppression") is True
        ),
        "requireProfileMatrixExplanation": (
            config.get("requireProfileMatrixExplanation") is True
        ),
    }


def _ranked_nodes(summary_packet: Mapping[str, object]) -> list[Mapping[str, object]]:
    nodes = summary_packet.get("rankedNodes")
    return (
        [node for node in nodes if isinstance(node, Mapping)]
        if isinstance(nodes, list)
        else []
    )


def _useful_selectors(receipt: Mapping[str, object]) -> tuple[str, ...]:
    followed = receipt.get("frontierFollowed")
    selectors: list[object] = []
    if isinstance(followed, list):
        selectors.extend(
            item.get("selector") for item in followed if isinstance(item, Mapping)
        )
    selector = receipt.get("selector")
    if isinstance(selector, str):
        selectors.append(selector)
    return tuple(item for item in selectors if isinstance(item, str) and item)


def _first_locator_match(
    ranked_nodes: Sequence[Mapping[str, object]],
    selectors: Sequence[str],
) -> tuple[int | None, object, object]:
    selector_set = set(selectors)
    fallback: tuple[int | None, object, object] = (None, None, None)
    for index, node in enumerate(ranked_nodes, start=1):
        selector = node.get("selector")
        if not isinstance(selector, str) or not selector:
            continue
        if fallback[0] is None:
            fallback = (index, node.get("id"), selector)
        if not selector_set or selector in selector_set:
            return index, node.get("id"), selector
    return fallback


def _active_profile_matrix(
    summary_packet: Mapping[str, object],
) -> Mapping[str, object]:
    profile = summary_packet.get("profile")
    matrices = summary_packet.get("profileMatrices")
    if not isinstance(matrices, list):
        return {}
    for matrix in matrices:
        if isinstance(matrix, Mapping) and matrix.get("profile") == profile:
            return matrix
    first = matrices[0] if matrices else {}
    return first if isinstance(first, Mapping) else {}


def _top_relation_channels(matrix: Mapping[str, object]) -> list[dict[str, object]]:
    channels = matrix.get("relationChannels")
    if not isinstance(channels, list):
        return []
    entries = [channel for channel in channels if isinstance(channel, Mapping)]
    entries.sort(
        key=lambda channel: (
            -_number_or_zero(channel.get("frontierContributionMass")),
            -_number_or_zero(channel.get("rankedContributionMass")),
            str(channel.get("relation")),
        )
    )
    return [
        {
            "relation": channel.get("relation"),
            "matrixNonZeroCount": channel.get("matrixNonZeroCount"),
            "frontierContributionMass": channel.get("frontierContributionMass"),
            "rankedContributionMass": channel.get("rankedContributionMass"),
        }
        for channel in entries
        if _number_or_zero(channel.get("matrixNonZeroCount")) > 0
    ][:3]


def _explanation_refs(summary_packet: Mapping[str, object]) -> list[dict[str, object]]:
    explanations = summary_packet.get("rankExplanations")
    if not isinstance(explanations, list):
        return []
    refs: list[dict[str, object]] = []
    for entry in explanations[:5]:
        if not isinstance(entry, Mapping):
            continue
        reasons = entry.get("reasons")
        if not isinstance(reasons, list):
            continue
        selected = [
            reason
            for reason in reasons
            if isinstance(reason, str)
            and (
                reason.startswith("relation:")
                or reason.startswith("kind-bonus:")
                or reason.startswith("receipt-")
                or reason == "typed-ppr"
            )
        ]
        if selected:
            refs.append({"nodeId": entry.get("nodeId"), "reasons": selected[:5]})
    return refs


def _check_optional_lte(
    failures: list[dict[str, object]],
    field: str,
    actual: object,
    maximum: object,
) -> None:
    if maximum is not None:
        _check_lte(failures, field, actual, maximum)


def _check_lte(
    failures: list[dict[str, object]],
    field: str,
    actual: object,
    maximum: object,
) -> None:
    value = _number(actual)
    expected = _number(maximum)
    if value is None or expected is None or value > expected:
        _add_failure(failures, field, actual, f"value <= {maximum}")


def _check_required_bool(
    failures: list[dict[str, object]],
    field: str,
    actual: object,
    required: object,
) -> None:
    if required is True and actual is not True:
        _add_failure(failures, field, actual, "value is true")


def _add_failure(
    failures: list[dict[str, object]],
    field: str,
    actual: object,
    expected: str,
) -> None:
    failures.append({"field": field, "actual": actual, "expected": expected})


def _mapping(value: object) -> Mapping[str, object]:
    return value if isinstance(value, Mapping) else {}


def _int_value(value: object) -> int:
    return value if isinstance(value, int) and not isinstance(value, bool) else 0


def _number(value: object) -> float | None:
    if isinstance(value, bool):
        return None
    return float(value) if isinstance(value, (int, float)) else None


def _number_or_zero(value: object) -> float:
    number = _number(value)
    return number if number is not None else 0.0


def _int_config(config: Mapping[str, object], name: str, default: int) -> int:
    value = config.get(name, default)
    return value if isinstance(value, int) and not isinstance(value, bool) else default


def _optional_int_config(config: Mapping[str, object], name: str) -> int | None:
    value = config.get(name)
    return value if isinstance(value, int) and not isinstance(value, bool) else None
