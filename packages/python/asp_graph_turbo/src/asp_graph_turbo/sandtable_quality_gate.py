"""Quality gate helpers for graph turbo sandtable summaries."""

from __future__ import annotations

import argparse
from collections.abc import Mapping


def gate_config(args: argparse.Namespace) -> Mapping[str, object]:
    return {
        "maxPprIterations": args.max_ppr_iterations,
        "pprMassTolerance": args.ppr_mass_tolerance,
        "minFrontierFollowRate": args.min_frontier_follow_rate,
        "maxRawReadFallbackCount": args.max_raw_read_fallback_count,
        "maxDuplicateSelectorCount": args.max_duplicate_selector_count,
        "maxSameOwnerScanCount": args.max_same_owner_scan_count,
        "maxCommandsToFirstUsefulLocator": args.max_commands_to_first_useful_locator,
        "maxP95Ms": args.max_p95_ms,
    }


def quality_gate(
    packet: Mapping[str, object],
    config: Mapping[str, object],
) -> dict[str, object]:
    failures: list[dict[str, object]] = []
    _check_benchmark_gate(failures, _mapping(packet.get("benchmark")), config)
    _check_receipt_gate(failures, _mapping(packet.get("receipt")), config)
    _check_report_chain_gate(
        failures, _mapping(packet.get("largeLibraryReportChain"))
    )
    return {
        "status": "pass" if not failures else "fail",
        "failures": failures,
    }


def _check_benchmark_gate(
    failures: list[dict[str, object]],
    benchmark: Mapping[str, object],
    config: Mapping[str, object],
) -> None:
    tolerance = _float_config(config, "pprMassTolerance", 0.000001)
    _expect_between(
        failures,
        "benchmark.pprMassSum",
        benchmark.get("pprMassSum"),
        1.0 - tolerance,
        1.0 + tolerance,
    )
    _expect_lte(
        failures,
        "benchmark.pprIterations",
        benchmark.get("pprIterations"),
        _int_config(config, "maxPprIterations", 100),
    )
    _expect_gt(
        failures,
        "benchmark.transitionNonZeroCount",
        benchmark.get("transitionNonZeroCount"),
        0,
    )
    _expect_gt(
        failures,
        "benchmark.transitionWeightMass",
        benchmark.get("transitionWeightMass"),
        0.0,
    )
    _expect_equal(
        failures,
        "benchmark.relationMatrixCount",
        benchmark.get("relationMatrixCount"),
        benchmark.get("relationCount"),
    )
    max_p95 = config.get("maxP95Ms")
    if max_p95 is not None:
        _expect_lte(failures, "benchmark.p95Ms", benchmark.get("p95Ms"), max_p95)


def _check_receipt_gate(
    failures: list[dict[str, object]],
    receipt: Mapping[str, object],
    config: Mapping[str, object],
) -> None:
    _expect_gte(
        failures,
        "receipt.frontierFollowRate",
        receipt.get("frontierFollowRate"),
        _float_config(config, "minFrontierFollowRate", 0.0),
    )
    _expect_lte(
        failures,
        "receipt.rawReadFallbackCount",
        receipt.get("rawReadFallbackCount"),
        _int_config(config, "maxRawReadFallbackCount", 0),
    )
    _expect_lte(
        failures,
        "receipt.duplicateSelectorCount",
        receipt.get("duplicateSelectorCount"),
        _int_config(config, "maxDuplicateSelectorCount", 0),
    )
    _expect_lte(
        failures,
        "receipt.sameOwnerScanCount",
        receipt.get("sameOwnerScanCount"),
        _int_config(config, "maxSameOwnerScanCount", 0),
    )
    max_first_locator = config.get("maxCommandsToFirstUsefulLocator")
    if max_first_locator is not None:
        _expect_lte(
            failures,
            "receipt.commandsToFirstUsefulLocator",
            receipt.get("commandsToFirstUsefulLocator"),
            max_first_locator,
        )


def _check_report_chain_gate(
    failures: list[dict[str, object]],
    report_chain: Mapping[str, object],
) -> None:
    if not report_chain:
        return
    _expect_equal(
        failures,
        "largeLibraryReportChain.status",
        report_chain.get("status"),
        "pass",
    )
    _expect_equal(
        failures,
        "largeLibraryReportChain.blockingFindingCount",
        report_chain.get("blockingFindingCount"),
        0,
    )


def _expect_between(
    failures: list[dict[str, object]],
    field: str,
    actual: object,
    minimum: float,
    maximum: float,
) -> None:
    value = _number(actual)
    if value is None or value < minimum or value > maximum:
        _add_failure(failures, field, actual, f"{minimum} <= value <= {maximum}")


def _expect_equal(
    failures: list[dict[str, object]],
    field: str,
    actual: object,
    expected: object,
) -> None:
    if actual != expected:
        _add_failure(failures, field, actual, f"value == {expected}")


def _expect_gt(
    failures: list[dict[str, object]],
    field: str,
    actual: object,
    threshold: int | float,
) -> None:
    value = _number(actual)
    if value is None or value <= threshold:
        _add_failure(failures, field, actual, f"value > {threshold}")


def _expect_gte(
    failures: list[dict[str, object]],
    field: str,
    actual: object,
    threshold: int | float,
) -> None:
    value = _number(actual)
    if value is None or value < threshold:
        _add_failure(failures, field, actual, f"value >= {threshold}")


def _expect_lte(
    failures: list[dict[str, object]],
    field: str,
    actual: object,
    threshold: object,
) -> None:
    value = _number(actual)
    expected = _number(threshold)
    if value is None or expected is None or value > expected:
        _add_failure(failures, field, actual, f"value <= {threshold}")


def _add_failure(
    failures: list[dict[str, object]],
    field: str,
    actual: object,
    expected: str,
) -> None:
    failures.append({"field": field, "actual": actual, "expected": expected})


def _mapping(value: object) -> Mapping[str, object]:
    return value if isinstance(value, Mapping) else {}


def _number(value: object) -> float | None:
    if isinstance(value, bool):
        return None
    if isinstance(value, (int, float)):
        return float(value)
    return None


def _int_config(config: Mapping[str, object], name: str, default: int) -> int:
    value = config.get(name, default)
    return value if isinstance(value, int) else default


def _float_config(config: Mapping[str, object], name: str, default: float) -> float:
    value = config.get(name, default)
    return float(value) if isinstance(value, (int, float)) else default
