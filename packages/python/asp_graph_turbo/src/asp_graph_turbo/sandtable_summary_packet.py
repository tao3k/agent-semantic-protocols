"""Packet builder for graph turbo sandtable summaries."""

from __future__ import annotations

from collections.abc import Mapping

from .sandtable_quality_gate import quality_gate


def summary_packet(
    benchmark: Mapping[str, object],
    receipt: Mapping[str, object],
    scenario: str | None,
    report_scenario: Mapping[str, object] | None = None,
    report_chain: Mapping[str, object] | None = None,
    gate_config: Mapping[str, object] | None = None,
) -> dict[str, object]:
    packet = {
        "schemaId": "agent.semantic-protocols.semantic-graph-turbo-sandtable-summary",
        "schemaVersion": "1",
        "protocolId": "agent.semantic-protocols.semantic-language",
        "protocolVersion": "1",
        "packetKind": "graph-turbo-sandtable-summary",
        "scenario": _scenario_name(scenario, report_scenario, receipt),
        "profile": benchmark.get("profile"),
        "benchmark": _benchmark_section(benchmark),
        "receipt": _receipt_section(receipt),
    }
    _attach_report_context(packet, report_scenario)
    _attach_report_chain(packet, report_chain)
    packet["qualityGate"] = quality_gate(packet, gate_config or {})
    return packet


def _benchmark_section(benchmark: Mapping[str, object]) -> dict[str, object]:
    benchmark_metrics = _mapping(benchmark.get("lastAlgorithmMetrics"))
    profile_matrix = _mapping(benchmark.get("lastProfileMatrix"))
    duration = _mapping(benchmark.get("durationMs"))
    return {
        "runs": benchmark.get("runs"),
        "warmupRuns": benchmark.get("warmupRuns"),
        "cacheMode": benchmark.get("cacheMode"),
        "medianMs": duration.get("median"),
        "p95Ms": duration.get("p95"),
        "pathBackend": benchmark_metrics.get("pathBackend"),
        "pathPairCount": benchmark_metrics.get("pathPairCount"),
        "pathCandidateCount": benchmark_metrics.get("pathCandidateCount"),
        "pathFallbackCount": benchmark_metrics.get("pathFallbackCount"),
        "pathCount": benchmark_metrics.get("pathCount"),
        "relationChannelCount": benchmark_metrics.get("relationChannelCount"),
        "relationCount": profile_matrix.get("relationCount"),
        "relationMatrixCount": profile_matrix.get("relationMatrixCount"),
        "zeroEdgeRelationCount": profile_matrix.get("zeroEdgeRelationCount"),
        "transitionNonZeroCount": profile_matrix.get("transitionNonZeroCount"),
        "transitionWeightMass": profile_matrix.get("transitionWeightMass"),
        "pprIterations": benchmark_metrics.get("pprIterations"),
        "pprResidual": benchmark_metrics.get("pprResidual"),
        "pprMassSum": benchmark_metrics.get("pprMassSum"),
        "pprDanglingMassLast": benchmark_metrics.get("pprDanglingMassLast"),
        "readLoopDirectCodeActionCount": benchmark_metrics.get(
            "readLoopDirectCodeActionCount"
        ),
        "readLoopDuplicateSelectorCount": benchmark_metrics.get(
            "readLoopDuplicateSelectorCount"
        ),
        "readLoopSameOwnerScanCount": benchmark_metrics.get(
            "readLoopSameOwnerScanCount"
        ),
        "readMemorySuppressedCount": benchmark_metrics.get("readMemorySuppressedCount"),
        "readLoopSecondPassSuppressedCount": benchmark_metrics.get(
            "readLoopSecondPassSuppressedCount"
        ),
        "readLoopDuplicateSelectorSuppressedCount": benchmark_metrics.get(
            "readLoopDuplicateSelectorSuppressedCount"
        ),
        "readLoopAdjacentRangeMergedCount": benchmark_metrics.get(
            "readLoopAdjacentRangeMergedCount"
        ),
        "readLoopSameOwnerSuppressedCount": benchmark_metrics.get(
            "readLoopSameOwnerSuppressedCount"
        ),
        "receiptBoostCount": benchmark_metrics.get("receiptBoostCount"),
        "receiptPenaltyCount": benchmark_metrics.get("receiptPenaltyCount"),
        "querySeedPriorCount": benchmark_metrics.get("querySeedPriorCount"),
        "querySeedPriorMass": benchmark_metrics.get("querySeedPriorMass"),
        "queryPackageCohesionCount": benchmark_metrics.get(
            "queryPackageCohesionCount"
        ),
        "queryPackageDriftPenaltyCount": benchmark_metrics.get(
            "queryPackageDriftPenaltyCount"
        ),
        "queryPackageCohesionDelta": benchmark_metrics.get(
            "queryPackageCohesionDelta"
        ),
        "queryClauseCoverageCount": benchmark_metrics.get(
            "queryClauseCoverageCount"
        ),
        "queryClauseCoverageDelta": benchmark_metrics.get(
            "queryClauseCoverageDelta"
        ),
        "queryLocalEvidenceBoostCount": benchmark_metrics.get(
            "queryLocalEvidenceBoostCount"
        ),
        "queryLocalEvidencePenaltyCount": benchmark_metrics.get(
            "queryLocalEvidencePenaltyCount"
        ),
        "queryLocalEvidenceDelta": benchmark_metrics.get(
            "queryLocalEvidenceDelta"
        ),
        "cacheStatus": benchmark_metrics.get("cacheStatus"),
    }


def _receipt_section(receipt: Mapping[str, object]) -> dict[str, object]:
    receipt_metrics = _mapping(receipt.get("metrics"))
    return {
        "receiptId": receipt.get("receiptId"),
        "frontierReturnedCount": receipt_metrics.get("frontierReturnedCount"),
        "frontierFollowedCount": receipt_metrics.get("frontierFollowedCount"),
        "frontierFollowRate": receipt_metrics.get("frontierFollowRate"),
        "codeActuallyReadCount": receipt_metrics.get("codeActuallyReadCount"),
        "rawReadFallbackCount": receipt_metrics.get("rawReadFallbackCount"),
        "duplicateSelectorCount": receipt_metrics.get("duplicateSelectorCount"),
        "sameOwnerScanCount": receipt_metrics.get("sameOwnerScanCount"),
        "commandsToFirstUsefulLocator": receipt_metrics.get(
            "commandsToFirstUsefulLocator"
        ),
        "commandsToValidation": receipt_metrics.get("commandsToValidation"),
    }


def _scenario_name(
    scenario: str | None,
    report_scenario: Mapping[str, object] | None,
    receipt: Mapping[str, object],
) -> str:
    return str(
        scenario
        or _mapping(report_scenario).get("scenarioId")
        or receipt.get("taskFingerprint")
        or "graph-turbo"
    )


def _attach_report_context(
    packet: dict[str, object],
    report_scenario: Mapping[str, object] | None,
) -> None:
    if report_scenario is None:
        return
    readiness = _mapping(report_scenario.get("benchmarkReadiness"))
    packet["benchmarkReport"] = {
        "reportId": report_scenario.get("reportId"),
        "scenarioId": report_scenario.get("scenarioId"),
        "captureKind": report_scenario.get("captureKind"),
        "readyForWeightCalibration": readiness.get("readyForWeightCalibration"),
    }
    packet["context"] = dict(_mapping(report_scenario.get("contextMetrics")))


def _attach_report_chain(
    packet: dict[str, object],
    report_chain: Mapping[str, object] | None,
) -> None:
    if report_chain is None:
        return
    rollup = _mapping(report_chain.get("rollup"))
    gate = _mapping(report_chain.get("optimizationGate"))
    batch = _mapping(report_chain.get("optimizationBatch"))
    variants = _string_list(batch.get("ablationVariants"))
    packet["largeLibraryReportChain"] = {
        "schemaId": report_chain.get("schemaId"),
        "packetKind": report_chain.get("packetKind"),
        "languageCount": rollup.get("languageCount"),
        "libraryCount": rollup.get("libraryCount"),
        "scenarioCount": rollup.get("scenarioCount"),
        "deepQuestionCount": rollup.get("deepQuestionCount"),
        "readyLanguageCount": rollup.get("readyLanguageCount"),
        "optimizationRunCount": rollup.get("optimizationRunCount"),
        "optimizationVariantRunCount": rollup.get("optimizationVariantRunCount"),
        "optimizationAblationVariantCount": batch.get("ablationVariantCount"),
        "optimizationAblationVariants": variants,
        "localEvidenceAblationEnabled": "no-local-evidence" in variants,
        "findingCount": rollup.get("findingCount"),
        "aspBinaryFreshnessRiskCommandCount": rollup.get(
            "aspBinaryFreshnessRiskCommandCount"
        )
        or 0,
        "aspBinaryFreshnessRiskScenarioCount": rollup.get(
            "aspBinaryFreshnessRiskScenarioCount"
        )
        or 0,
        "status": gate.get("status"),
        "reason": gate.get("reason"),
        "blockingFindingCount": gate.get("blockingFindingCount"),
    }


def _mapping(value: object) -> Mapping[str, object]:
    return value if isinstance(value, Mapping) else {}


def _string_list(value: object) -> list[str]:
    if not isinstance(value, list):
        return []
    return [entry for entry in value if isinstance(entry, str)]
