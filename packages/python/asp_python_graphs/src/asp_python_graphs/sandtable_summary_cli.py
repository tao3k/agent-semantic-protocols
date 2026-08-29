"""Summarize graph turbo benchmark and receipt packets for sandtables."""

from __future__ import annotations

import argparse
import json
import sys
from collections.abc import Mapping, Sequence
from pathlib import Path

from .benchmark_cli import benchmark_packet
from .cli import _load_packet
from .sandtable_summary_args import parse_args
from .sandtable_summary_packet import summary_packet
from .sandtable_summary_render import render_text
from .sandtable_quality_gate import gate_config


def main(argv: Sequence[str] | None = None) -> int:
    args = parse_args(argv)
    benchmark = _load_benchmark(args)
    report_scenario = _load_report_scenario(args.benchmark_report, args.report_scenario)
    report_chain = _load_report_chain(args.large_library_report_chain)
    if report_scenario is None:
        if args.receipt is None:
            raise SystemExit(
                "--receipt is required unless --benchmark-report is provided"
            )
        receipt = _load_receipt(args.receipt, args.receipt_fixture_id)
    else:
        receipt = _receipt_from_report_scenario(report_scenario)
    summary = summary_packet(
        benchmark,
        receipt,
        args.scenario,
        report_scenario,
        report_chain,
        gate_config(args),
    )
    if args.format == "json":
        sys.stdout.write(json.dumps(summary, sort_keys=True) + "\n")
    else:
        sys.stdout.write(render_text(summary) + "\n")
    if (
        args.fail_on_gate
        and _mapping(summary.get("qualityGate")).get("status") != "pass"
    ):
        return 1
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


def _load_report_chain(path: str | None) -> Mapping[str, object] | None:
    if path is None:
        return None
    return _load_json(path)


def _receipt_from_report_scenario(
    scenario: Mapping[str, object],
) -> Mapping[str, object]:
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


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))
