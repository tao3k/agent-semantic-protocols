"""CLI helpers for adaptive graph-turbo validation manifests."""

from __future__ import annotations

import argparse
import json
from collections.abc import Mapping
from pathlib import Path

from .large_library_adaptive_validation_manifest import (
    build_large_library_adaptive_validation_manifest,
)
from .output import emit, emit_json, write_json_file
from .utils import resolve_path


def add_large_library_adaptive_validation_manifest_arguments(
    parser: argparse.ArgumentParser,
) -> None:
    parser.add_argument(
        "--large-library-adaptive-validation-manifest",
        metavar="ADAPTIVE_POLICY_JSON",
        help=(
            "Build executable live-agent validation run entries from a "
            "graph-turbo adaptive policy."
        ),
    )
    parser.add_argument(
        "--validation-session-root",
        default=".cache/agent-semantic-protocol/adaptive-validation",
        help=(
            "Session root prefix for generated live-agent validation commands."
        ),
    )


def handle_large_library_adaptive_validation_manifest_args(
    repo_root: Path,
    args: argparse.Namespace,
) -> int | None:
    source = args.large_library_adaptive_validation_manifest
    if source is None:
        return None
    policy = _load_json_object(resolve_path(repo_root, source))
    manifest = build_large_library_adaptive_validation_manifest(
        repo_root,
        policy,
        session_root=args.validation_session_root,
    )
    output_arg = getattr(args, "output", None)
    if output_arg:
        write_json_file(resolve_path(repo_root, output_arg), manifest)
    elif args.json:
        emit_json(manifest)
    else:
        _print_manifest(manifest)
    if args.fail_on_missing and manifest["summary"]["missingPromptCount"]:
        return 1
    return 0


def _load_json_object(path: Path) -> dict[str, object]:
    value = json.loads(path.read_text(encoding="utf-8"))
    if not isinstance(value, Mapping):
        raise SystemExit(f"{path} must contain a JSON object")
    return dict(value)


def _print_manifest(manifest: dict[str, object]) -> None:
    summary = manifest.get("summary")
    if not isinstance(summary, dict):
        emit("[large-library-adaptive-validation-manifest] invalid")
        return
    emit(
        "[large-library-adaptive-validation-manifest] "
        f"runs={summary.get('runCount')} "
        f"promptResolved={summary.get('promptResolvedCount')} "
        f"missingPrompts={summary.get('missingPromptCount')}"
    )
