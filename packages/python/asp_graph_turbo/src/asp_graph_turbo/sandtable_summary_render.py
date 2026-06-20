"""Text rendering for graph turbo sandtable summaries."""

from __future__ import annotations

from collections.abc import Mapping


def render_text(packet: Mapping[str, object]) -> str:
    benchmark = _mapping(packet.get("benchmark"))
    receipt = _mapping(packet.get("receipt"))
    output = (
        "[graph-sandtable-summary] "
        f"scenario={packet.get('scenario')} profile={packet.get('profile')} "
        f"medianMs={benchmark.get('medianMs')} p95Ms={benchmark.get('p95Ms')}\n"
        "benchmark="
        f"pathBackend={benchmark.get('pathBackend')},"
        f"pathPairs={benchmark.get('pathPairCount')},"
        f"pathCandidates={benchmark.get('pathCandidateCount')},"
        f"pathFallbacks={benchmark.get('pathFallbackCount')},"
        f"pprIterations={benchmark.get('pprIterations')},"
        f"pprMass={benchmark.get('pprMassSum')},"
        f"relations={benchmark.get('relationMatrixCount')},"
        f"transitionNnz={benchmark.get('transitionNonZeroCount')},"
        f"cache={benchmark.get('cacheStatus')}\n"
        "receipt="
        f"followRate={receipt.get('frontierFollowRate')},"
        f"rawReadFallbacks={receipt.get('rawReadFallbackCount')},"
        f"duplicateSelectors={receipt.get('duplicateSelectorCount')},"
        f"sameOwnerScans={receipt.get('sameOwnerScanCount')},"
        f"commandsToValidation={receipt.get('commandsToValidation')}"
    )
    return _append_quality_and_context(output, packet)


def _append_quality_and_context(
    output: str,
    packet: Mapping[str, object],
) -> str:
    gate = _mapping(packet.get("qualityGate"))
    failures = gate.get("failures")
    failure_count = len(failures) if isinstance(failures, list) else 0
    output += f"\ngate=status={gate.get('status')},failures={failure_count}"
    context = _mapping(packet.get("context"))
    if context:
        output += "\ncontext=" + _context_text(context)
    report_chain = _mapping(packet.get("largeLibraryReportChain"))
    if report_chain:
        output += "\nlargeLibraryReportChain=" + _report_chain_text(report_chain)
    return output


def _context_text(context: Mapping[str, object]) -> str:
    best_rank = context.get("goldFrontierBestRank")
    rank_text = "" if best_rank is None else f",bestRank={best_rank}"
    action_rank = context.get("goldSelectorActionRank")
    action_rank_text = "" if action_rank is None else f",actionRank={action_rank}"
    return (
        f"precision={_format_ratio(context.get('contextPrecision'))},"
        f"recall={_format_ratio(context.get('contextRecall'))},"
        f"utilization={_format_ratio(context.get('contextUtilization'))}"
        f"{rank_text}{action_rank_text},"
        f"exactCode={context.get('exactCodeSuccess')},"
        f"testPrecision={_format_ratio(context.get('testSelectionPrecision'))}"
    )


def _report_chain_text(report_chain: Mapping[str, object]) -> str:
    return (
        f"status={report_chain.get('status')},"
        f"languages={report_chain.get('languageCount')},"
        f"ready={report_chain.get('readyLanguageCount')},"
        f"questions={report_chain.get('deepQuestionCount')},"
        f"runs={report_chain.get('optimizationRunCount')},"
        f"variantRuns={report_chain.get('optimizationVariantRunCount')},"
        f"ablationVariants={report_chain.get('optimizationAblationVariantCount')},"
        f"localEvidenceAblation={report_chain.get('localEvidenceAblationEnabled')},"
        f"findings={report_chain.get('findingCount')}"
    )


def _format_ratio(value: object) -> str:
    if isinstance(value, bool):
        return str(value)
    if isinstance(value, (int, float)):
        return f"{float(value):.1f}"
    return str(value)


def _mapping(value: object) -> Mapping[str, object]:
    return value if isinstance(value, Mapping) else {}
