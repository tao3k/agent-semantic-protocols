"""CLI helpers for adaptive graph-turbo deep-search simulation."""

from __future__ import annotations

import argparse
import json
from collections.abc import Mapping
from pathlib import Path

from .large_library_adaptive_simulation import run_large_library_adaptive_simulation
from .large_library_adaptive_validation_manifest import (
    build_large_library_adaptive_validation_manifest,
)
from .output import emit, emit_json
from .utils import resolve_path


def add_large_library_adaptive_simulation_arguments(
    parser: argparse.ArgumentParser,
) -> None:
    parser.add_argument(
        "--large-library-adaptive-simulation",
        metavar="ADAPTIVE_POLICY_JSON",
        help="Run simulated TS/Rust deep-search sessions from an adaptive policy.",
    )
    parser.add_argument(
        "--simulation-manifest",
        metavar="VALIDATION_MANIFEST_JSON",
        help="Optional prebuilt validation manifest for adaptive simulation.",
    )
    parser.add_argument(
        "--simulation-output-root",
        default=".cache/agent-semantic-protocol/adaptive-simulation",
        help="Output directory for adaptive simulation artifacts.",
    )
    parser.add_argument(
        "--simulation-limit",
        type=int,
        help="Optional maximum number of manifest runs to simulate.",
    )


def handle_large_library_adaptive_simulation_args(
    repo_root: Path,
    args: argparse.Namespace,
) -> int | None:
    source = args.large_library_adaptive_simulation
    if source is None:
        return None
    policy = _load_json_object(resolve_path(repo_root, source))
    manifest = _manifest(repo_root, args, policy)
    report = run_large_library_adaptive_simulation(
        repo_root,
        policy,
        manifest,
        resolve_path(repo_root, args.simulation_output_root),
        limit=args.simulation_limit,
    )
    if args.json:
        emit_json(report)
    else:
        _print_report(report)
    if args.fail_on_missing and report["summary"]["statusCounts"].get("pass") != report[
        "summary"
    ]["runCount"]:
        return 1
    return 0


def _manifest(
    repo_root: Path,
    args: argparse.Namespace,
    policy: dict[str, object],
) -> dict[str, object]:
    manifest_arg = getattr(args, "simulation_manifest", None)
    if manifest_arg:
        return _load_json_object(resolve_path(repo_root, manifest_arg))
    return build_large_library_adaptive_validation_manifest(repo_root, policy)


def _load_json_object(path: Path) -> dict[str, object]:
    value = json.loads(path.read_text(encoding="utf-8"))
    if not isinstance(value, Mapping):
        raise SystemExit(f"{path} must contain a JSON object")
    return dict(value)


def _print_report(report: dict[str, object]) -> None:
    summary = report.get("summary")
    if not isinstance(summary, dict):
        emit("[large-library-adaptive-simulation] invalid")
        return
    emit(
        "[large-library-adaptive-simulation] "
        f"runs={summary.get('runCount')} "
        f"statuses={summary.get('statusCounts')} "
        f"commands={summary.get('totalCommandCount')} "
        f"elapsedMs={summary.get('totalElapsedMs')} "
        f"ownerRecovery={summary.get('ownerItemsRecoveryCounts')} "
        f"selectorQuality={summary.get('selectorQualityCounts')} "
        f"finalAction={summary.get('finalStepActionCounts')} "
        f"finalNext={summary.get('finalRecommendedNextCounts')} "
        f"finalRecovery={summary.get('finalOwnerItemsRecoveryCounts')} "
        f"finalTransition={summary.get('finalOwnerItemsTransitionCounts')} "
        f"probeRecovery={summary.get('recoveryProbeOwnerItemsRecoveryCounts')} "
        f"probeSelectorQuality={summary.get('recoveryProbeSelectorQualityCounts')}"
    )
    for item in report.get("algorithmImprovementPlan", []):
        if isinstance(item, dict):
            emit(
                "|improve "
                f"id={item.get('id')} "
                f"evidenceRuns={item.get('evidenceRunCount')} "
                f"action={item.get('recommendedAction')}"
            )
    for item in report.get("ownerItemsRecoveryCases", []):
        if isinstance(item, dict):
            emit(
                "|owner-recovery "
                f"run={item.get('runId')} "
                f"recovery={item.get('recovery')} "
                f"owner={item.get('owner')} "
                f"query={item.get('query')} "
                f"next={item.get('nextCommand')}"
            )
    for item in report.get("selectorQualityCases", []):
        if isinstance(item, dict):
            emit(
                "|selector-quality "
                f"run={item.get('runId')} "
                f"quality={item.get('selectorQuality')} "
                f"owner={item.get('owner')} "
                f"selector={item.get('selector')} "
                f"matched={item.get('matchedQueryTerms')} "
                f"missing={item.get('missingQueryTerms')}"
            )
