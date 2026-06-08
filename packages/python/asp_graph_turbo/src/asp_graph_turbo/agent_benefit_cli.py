"""Build graph-turbo agent behavior benefit reports."""

from __future__ import annotations

import argparse
import json
import sys
from collections.abc import Mapping, Sequence
from pathlib import Path

from .agent_benefit_packet import build_agent_benefit_report
from .cli import _load_packet


def main(argv: Sequence[str] | None = None) -> int:
    args = _parse_args(argv)
    report = build_agent_benefit_report(
        _load_packet(args.packet),
        scenario=args.scenario,
        receipt=_load_receipt(args.receipt, args.receipt_fixture_id),
        rank_args=args,
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
    source = _mapping(report.get("sourceReading"))
    locator = _mapping(report.get("usefulLocator"))
    failure = _mapping(report.get("failureEvidence"))
    feedback = _mapping(report.get("feedbackLearning"))
    matrix = _mapping(report.get("profileMatrixExplanation"))
    gate = _mapping(report.get("qualityGate"))
    return (
        "[graph-agent-benefit] "
        f"scenario={report.get('scenario')} profile={report.get('profile')} "
        f"gate={gate.get('status')}\n"
        "source="
        f"directCode={source.get('directCodeActionCount')},"
        f"rawFallback={source.get('rawReadFallbackCount')},"
        f"duplicateSelectors={source.get('duplicateSelectorCount')},"
        f"readMemorySuppressed={source.get('readMemorySuppressedCount')}\n"
        "locator="
        f"found={locator.get('found')},"
        f"rank={locator.get('rank')},"
        f"commands={locator.get('commandsToFirstUsefulLocator')}\n"
        "failure="
        f"found={failure.get('found')},"
        f"rank={failure.get('rank')},"
        f"kind={failure.get('kind')}\n"
        "feedback="
        f"repeatedSuppressed={feedback.get('repeatedMistakeSuppressed')},"
        f"receiptApplied={feedback.get('receiptFeedbackApplied')}\n"
        "matrix="
        f"explained={matrix.get('explained')},"
        f"transitionNnz={matrix.get('transitionNonZeroCount')}"
    )


def _parse_args(argv: Sequence[str] | None) -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("packet", nargs="?", default="-")
    parser.add_argument("--scenario")
    parser.add_argument("--profile", default=None)
    parser.add_argument("--seed", action="append", default=[])
    parser.add_argument("--limit", type=int, default=None)
    parser.add_argument("--receipt")
    parser.add_argument("--receipt-fixture-id")
    parser.add_argument(
        "--max-raw-read-fallback-count", type=_non_negative_int, default=0
    )
    parser.add_argument(
        "--max-duplicate-selector-count", type=_non_negative_int, default=0
    )
    parser.add_argument(
        "--max-commands-to-first-useful-locator", type=_non_negative_int
    )
    parser.add_argument("--max-useful-locator-rank", type=_positive_int)
    parser.add_argument("--max-failure-evidence-rank", type=_positive_int)
    parser.add_argument("--require-useful-locator", action="store_true")
    parser.add_argument("--require-failure-evidence", action="store_true")
    parser.add_argument("--require-repeated-mistake-suppression", action="store_true")
    parser.add_argument("--require-profile-matrix-explanation", action="store_true")
    parser.add_argument("--fail-on-quality-gate", action="store_true")
    parser.add_argument("--format", choices=["json", "text"], default="json")
    return parser.parse_args(argv)


def _quality_config(args: argparse.Namespace) -> Mapping[str, object]:
    return {
        "maxRawReadFallbackCount": args.max_raw_read_fallback_count,
        "maxDuplicateSelectorCount": args.max_duplicate_selector_count,
        "maxCommandsToFirstUsefulLocator": args.max_commands_to_first_useful_locator,
        "maxUsefulLocatorRank": args.max_useful_locator_rank,
        "maxFailureEvidenceRank": args.max_failure_evidence_rank,
        "requireUsefulLocator": args.require_useful_locator,
        "requireFailureEvidence": args.require_failure_evidence,
        "requireRepeatedMistakeSuppression": args.require_repeated_mistake_suppression,
        "requireProfileMatrixExplanation": args.require_profile_matrix_explanation,
    }


def _load_receipt(
    path: str | None,
    fixture_id: str | None,
) -> Mapping[str, object] | None:
    if path is None:
        return None
    packet = json.loads(Path(path).read_text(encoding="utf-8"))
    if not isinstance(packet, Mapping):
        raise SystemExit(f"{path} must contain a JSON object")
    fixtures = packet.get("fixtures")
    if not isinstance(fixtures, list):
        return packet
    fixture = _select_fixture(fixtures, fixture_id)
    receipt = _mapping(fixture).get("receipt")
    if not isinstance(receipt, Mapping):
        raise SystemExit("receipt fixture entry must contain a receipt object")
    return receipt


def _select_fixture(
    fixtures: Sequence[object],
    fixture_id: str | None,
) -> Mapping[str, object]:
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
    if not isinstance(fixture, Mapping):
        raise SystemExit(f"receipt fixture not found: {fixture_id}")
    return fixture


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
