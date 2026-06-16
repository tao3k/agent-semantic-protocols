"""Build graph-turbo ablation comparison reports."""

from __future__ import annotations

import argparse
import json
import sys
from collections.abc import Mapping, Sequence

from .ablation_cli import ABLATION_VARIANTS
from .ablation_report_packet import build_ablation_report
from .cli import _load_packet


def main(argv: Sequence[str] | None = None) -> int:
    args = _parse_args(argv)
    variants = _selected_variants(args.variant)
    report = build_ablation_report(
        _load_packet(args.packet),
        variants=variants,
        runs=args.runs,
        warmup_runs=args.warmup_runs,
        cache_mode=args.cache_mode,
        profile=args.profile,
        seed=args.seed,
        limit=args.limit,
        quality_config=_quality_config(args),
    )
    if args.format == "json":
        sys.stdout.write(json.dumps(report, sort_keys=True) + "\n")
    else:
        sys.stdout.write(_render_text(report) + "\n")
    if (
        args.fail_on_quality_gate
        and _mapping(report.get("qualityGate")).get("status") != "pass"
    ):
        return 1
    return 0


def _render_text(report: Mapping[str, object]) -> str:
    summary = _mapping(report.get("summary"))
    quality_gate = _mapping(report.get("qualityGate"))
    output = (
        "[graph-ablation-report] "
        f"profile={report.get('sourceProfile')} "
        f"variants={summary.get('variantCount')} "
        f"rankChanged={summary.get('rankChangedVariantCount')} "
        f"worstOverlap={summary.get('worstRankOverlapRatio')} "
        f"maxScoreDeltaL1={summary.get('maxScoreDeltaL1')} "
        f"gate={quality_gate.get('status')}"
    )
    for entry in _variant_entries(report):
        comparison = _mapping(entry.get("comparison"))
        output += (
            "\nvariant="
            f"{entry.get('variant')},"
            f"rankChanged={comparison.get('rankChanged')},"
            f"overlap={comparison.get('rankOverlapRatio')},"
            f"scoreDeltaL1={comparison.get('scoreDeltaL1')},"
            f"readMemoryDelta={comparison.get('readMemorySuppressedDelta')},"
            f"receiptBoostDelta={comparison.get('receiptBoostDelta')},"
            f"transitionNnzDelta={comparison.get('transitionNonZeroDelta')},"
            f"querySeedDelta={comparison.get('querySeedPriorCountDelta')},"
            f"queryPackageDelta={comparison.get('queryPackageCohesionCountDelta')},"
            f"queryClauseDelta={comparison.get('queryClauseCoverageCountDelta')}"
        )
    return output


def _parse_args(argv: Sequence[str] | None) -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("packet", nargs="?", default="-")
    parser.add_argument("--variant", action="append", default=[])
    parser.add_argument("--runs", type=_positive_int, default=1)
    parser.add_argument("--warmup-runs", type=_non_negative_int, default=0)
    parser.add_argument(
        "--cache-mode",
        choices=["packet", "enabled", "disabled"],
        default="disabled",
    )
    parser.add_argument("--profile", default=None)
    parser.add_argument("--seed", action="append", default=[])
    parser.add_argument("--limit", type=int, default=None)
    parser.add_argument("--min-worst-rank-overlap-ratio", type=float, default=0.0)
    parser.add_argument("--max-score-delta-l1", type=float)
    parser.add_argument("--require-channel-signals", action="store_true")
    parser.add_argument("--fail-on-quality-gate", action="store_true")
    parser.add_argument("--format", choices=["json", "text"], default="json")
    return parser.parse_args(argv)


def _quality_config(args: argparse.Namespace) -> Mapping[str, object]:
    return {
        "minWorstRankOverlapRatio": args.min_worst_rank_overlap_ratio,
        "maxScoreDeltaL1": args.max_score_delta_l1,
        "requireChannelSignals": args.require_channel_signals,
    }


def _selected_variants(variants: Sequence[str]) -> tuple[str, ...]:
    selected = ABLATION_VARIANTS if not variants else tuple(variants)
    unknown = [variant for variant in selected if variant not in ABLATION_VARIANTS]
    if unknown:
        raise SystemExit(f"unknown ablation variant: {','.join(unknown)}")
    return selected


def _variant_entries(report: Mapping[str, object]) -> list[Mapping[str, object]]:
    variants = report.get("variants")
    if not isinstance(variants, list):
        return []
    return [entry for entry in variants if isinstance(entry, Mapping)]


def _mapping(value: object) -> Mapping[str, object]:
    return value if isinstance(value, Mapping) else {}


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
