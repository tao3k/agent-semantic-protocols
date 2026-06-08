"""Summarize graph turbo benchmark and receipt packets for sandtables."""

from __future__ import annotations

import argparse
import json
import sys
from collections.abc import Mapping, Sequence
from pathlib import Path

from .benchmark_cli import benchmark_packet
from .cli import _load_packet


def main(argv: Sequence[str] | None = None) -> int:
    args = _parse_args(argv)
    benchmark = _load_benchmark(args)
    report_scenario = _load_report_scenario(args.benchmark_report, args.report_scenario)
    if report_scenario is None:
        if args.receipt is None:
            raise SystemExit("--receipt is required unless --benchmark-report is provided")
        receipt = _load_receipt(args.receipt, args.receipt_fixture_id)
    else:
        receipt = _receipt_from_report_scenario(report_scenario)
    summary = _summary_packet(benchmark, receipt, args.scenario, report_scenario)
    if args.format == "json":
        sys.stdout.write(json.dumps(summary, sort_keys=True) + "\n")
    else:
        sys.stdout.write(_render_text(summary) + "\n")
    return 0


def _load_benchmark(args: argparse.Namespace) -> Mapping[str, object]:
    if args.benchmark is not None:
        return _load_json(args.benchmark)
    packet = _load_packet(args.benchmark_packet)
    return benchmark_packet(
        packet,
        runs=args.benchmark_runs,
        warmup_runs=args.benchmark_warmup_runs,
        cache_mode=args.benchmark_cache_mode,
        profile=args.profile,
        seed=args.seed,
        limit=args.limit,
    )


def _summary_packet(
    benchmark: Mapping[str, object],
    receipt: Mapping[str, object],
    scenario: str | None,
    report_scenario: Mapping[str, object] | None = None,
) -> dict[str, object]:
    benchmark_metrics = _mapping(benchmark.get("lastAlgorithmMetrics"))
    receipt_metrics = _mapping(receipt.get("metrics"))
    duration = _mapping(benchmark.get("durationMs"))
    scenario_name = scenario or str(
        _mapping(report_scenario).get("scenarioId")
        or receipt.get("taskFingerprint")
        or "graph-turbo"
    )
    packet = {
        "schemaId": "agent.semantic-protocols.semantic-graph-turbo-sandtable-summary",
        "schemaVersion": "1",
        "protocolId": "agent.semantic-protocols.semantic-language",
        "protocolVersion": "1",
        "packetKind": "graph-turbo-sandtable-summary",
        "scenario": scenario_name,
        "profile": benchmark.get("profile"),
        "benchmark": {
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
            "pprIterations": benchmark_metrics.get("pprIterations"),
            "cacheStatus": benchmark_metrics.get("cacheStatus"),
        },
        "receipt": {
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
        },
    }
    if report_scenario is not None:
        readiness = _mapping(report_scenario.get("benchmarkReadiness"))
        packet["benchmarkReport"] = {
            "reportId": report_scenario.get("reportId"),
            "scenarioId": report_scenario.get("scenarioId"),
            "captureKind": report_scenario.get("captureKind"),
            "readyForWeightCalibration": readiness.get("readyForWeightCalibration"),
        }
        packet["context"] = dict(_mapping(report_scenario.get("contextMetrics")))
    return packet


def _format_ratio(value: object) -> str:
    if isinstance(value, bool):
        return str(value)
    if isinstance(value, (int, float)):
        return f"{float(value):.1f}"
    return str(value)


def _render_text(packet: Mapping[str, object]) -> str:
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
        f"cache={benchmark.get('cacheStatus')}\n"
        "receipt="
        f"followRate={receipt.get('frontierFollowRate')},"
        f"rawReadFallbacks={receipt.get('rawReadFallbackCount')},"
        f"duplicateSelectors={receipt.get('duplicateSelectorCount')},"
        f"sameOwnerScans={receipt.get('sameOwnerScanCount')},"
        f"commandsToValidation={receipt.get('commandsToValidation')}"
    )
    context = _mapping(packet.get("context"))
    if context:
        best_rank = context.get("goldFrontierBestRank")
        rank_text = "" if best_rank is None else f",bestRank={best_rank}"
        action_rank = context.get("goldSelectorActionRank")
        action_rank_text = (
            "" if action_rank is None else f",actionRank={action_rank}"
        )
        output += (
            "\ncontext="
            f"precision={_format_ratio(context.get('contextPrecision'))},"
            f"recall={_format_ratio(context.get('contextRecall'))},"
            f"utilization={_format_ratio(context.get('contextUtilization'))}"
            f"{rank_text}{action_rank_text},"
            f"exactCode={context.get('exactCodeSuccess')},"
            f"testPrecision={_format_ratio(context.get('testSelectionPrecision'))}"
        )
    return output


def _load_receipt(path: str, fixture_id: str | None) -> Mapping[str, object]:
    packet = _load_json(path)
    fixtures = packet.get("fixtures")
    if not isinstance(fixtures, list):
        return packet
    if fixture_id is None:
        fixture = fixtures[0]
    else:
        fixture = next(
            (
                item
                for item in fixtures
                if isinstance(item, Mapping) and item.get("fixtureId") == fixture_id
            ),
            None,
        )
        if fixture is None:
            raise SystemExit(f"receipt fixture not found: {fixture_id}")
    if not isinstance(fixture, Mapping) or not isinstance(
        fixture.get("receipt"), Mapping
    ):
        raise SystemExit("receipt fixture entry must contain a receipt object")
    return fixture["receipt"]


def _load_report_scenario(
    path: str | None, scenario_id: str | None
) -> Mapping[str, object] | None:
    if path is None:
        return None
    packet = _load_json(path)
    reports = packet.get("reports")
    if not isinstance(reports, list) or not reports:
        raise SystemExit("benchmark report fixture must contain reports")
    report = reports[0]
    if not isinstance(report, Mapping):
        raise SystemExit("benchmark report entry must be an object")
    scenarios = report.get("scenarios")
    if not isinstance(scenarios, list) or not scenarios:
        raise SystemExit("benchmark report entry must contain scenarios")
    if scenario_id is None:
        scenario = next(
            (
                item
                for item in scenarios
                if isinstance(item, Mapping)
                and _mapping(item.get("benchmarkReadiness")).get(
                    "readyForWeightCalibration"
                )
                is True
            ),
            scenarios[0],
        )
    else:
        scenario = next(
            (
                item
                for item in scenarios
                if isinstance(item, Mapping) and item.get("scenarioId") == scenario_id
            ),
            None,
        )
    if not isinstance(scenario, Mapping):
        raise SystemExit(f"benchmark report scenario not found: {scenario_id}")
    scenario_packet = dict(scenario)
    scenario_packet["reportId"] = report.get("reportId")
    return scenario_packet


def _receipt_from_report_scenario(scenario: Mapping[str, object]) -> Mapping[str, object]:
    return {
        "receiptId": scenario.get("receiptId"),
        "metrics": dict(_mapping(scenario.get("receiptMetrics"))),
    }


def _load_json(path: str) -> Mapping[str, object]:
    packet = json.loads(Path(path).read_text(encoding="utf-8"))
    if not isinstance(packet, Mapping):
        raise SystemExit(f"{path} must contain a JSON object")
    return packet


def _mapping(value: object) -> Mapping[str, object]:
    return value if isinstance(value, Mapping) else {}


def _parse_args(argv: Sequence[str] | None) -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    benchmark_source = parser.add_mutually_exclusive_group(required=True)
    benchmark_source.add_argument("--benchmark")
    benchmark_source.add_argument("--benchmark-packet")
    parser.add_argument("--benchmark-runs", type=_positive_int, default=30)
    parser.add_argument("--benchmark-warmup-runs", type=_non_negative_int, default=3)
    parser.add_argument(
        "--benchmark-cache-mode",
        choices=["packet", "enabled", "disabled"],
        default="packet",
    )
    parser.add_argument("--profile", default=None)
    parser.add_argument("--seed", action="append", default=[])
    parser.add_argument("--limit", type=int, default=None)
    parser.add_argument("--receipt")
    parser.add_argument("--receipt-fixture-id")
    parser.add_argument("--benchmark-report")
    parser.add_argument("--report-scenario")
    parser.add_argument("--scenario")
    parser.add_argument("--format", choices=["json", "text"], default="json")
    return parser.parse_args(argv)


def _positive_int(value: str) -> int:
    parsed = int(value)
    if parsed < 1:
        raise argparse.ArgumentTypeError("value must be positive")
    return parsed


def _non_negative_int(value: str) -> int:
    parsed = int(value)
    if parsed < 0:
        raise argparse.ArgumentTypeError("value must be non-negative")
    return parsed


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))
