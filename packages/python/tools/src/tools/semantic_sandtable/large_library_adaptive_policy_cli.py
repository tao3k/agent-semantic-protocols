"""CLI helpers for large-library adaptive graph-turbo policy candidates."""

from __future__ import annotations

import argparse
import json
from collections.abc import Mapping
from pathlib import Path

from .large_library_adaptive_policy import build_large_library_adaptive_policy
from .output import emit, emit_json, write_json_file
from .utils import resolve_path


def add_large_library_adaptive_policy_arguments(
    parser: argparse.ArgumentParser,
) -> None:
    parser.add_argument(
        "--large-library-adaptive-policy",
        metavar="OPTIMIZATION_ANALYSIS_JSON",
        help=(
            "Build a graph-turbo adaptive query policy candidate from a "
            "large-library optimization analysis packet."
        ),
    )


def handle_large_library_adaptive_policy_args(
    repo_root: Path,
    args: argparse.Namespace,
) -> int | None:
    source = args.large_library_adaptive_policy
    if source is None:
        return None
    analysis = _load_json_object(resolve_path(repo_root, source))
    packet = build_large_library_adaptive_policy(analysis)
    output_arg = getattr(args, "output", None)
    if output_arg:
        write_json_file(resolve_path(repo_root, output_arg), packet)
    elif args.json:
        emit_json(packet)
    else:
        _print_policy(packet)
    if args.fail_on_missing and packet["status"] != "ready":
        return 1
    return 0


def _load_json_object(path: Path) -> dict[str, object]:
    value = json.loads(path.read_text(encoding="utf-8"))
    if not isinstance(value, Mapping):
        raise SystemExit(f"{path} must contain a JSON object")
    return dict(value)


def _print_policy(packet: dict[str, object]) -> None:
    default_policy = packet.get("defaultPolicy")
    variant = "-"
    if isinstance(default_policy, dict):
        variant = str(default_policy.get("ablationVariant", "-"))
    bucket_count = 0
    if isinstance(packet.get("bucketPolicies"), list):
        bucket_count = len(packet["bucketPolicies"])
    emit(
        "[large-library-adaptive-policy] "
        f"status={packet.get('status')} "
        f"default={variant} "
        f"buckets={bucket_count}"
    )
