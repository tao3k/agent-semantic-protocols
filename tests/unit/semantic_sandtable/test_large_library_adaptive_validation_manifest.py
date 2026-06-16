"""Adaptive graph-turbo validation manifest tests."""

from __future__ import annotations

import json
from pathlib import Path

from tools.semantic_sandtable.cli import semantic_sandtable_main as main
from tools.semantic_sandtable.large_library_adaptive_validation_manifest import (
    build_large_library_adaptive_validation_manifest,
)

from .test_large_library_adaptive_validation import _ROOT, _policy, _validate_schema


def test_adaptive_validation_manifest_resolves_prompts() -> None:
    manifest = build_large_library_adaptive_validation_manifest(
        _ROOT,
        _policy(),
        session_root=".cache/validation",
    )

    _validate_schema(
        manifest,
        "semantic-graph-turbo-adaptive-validation-manifest.v1.schema.json",
    )
    assert manifest["summary"]["runCount"] == 2
    assert manifest["summary"]["promptResolvedCount"] == 2
    assert manifest["summary"]["missingPromptCount"] == 0
    first_run = manifest["runs"][0]
    assert first_run["promptResolved"] is True
    assert "BufMut" in first_run["prompt"]
    assert first_run["language"] == "rust"
    assert first_run["project"]["name"] == "bytes"
    assert first_run["sessionRoot"].startswith(".cache/validation/run-a")
    assert first_run["env"] == {
        "ASP_GRAPH_TURBO_ABLATION_VARIANT": "no-query-seed-prior"
    }
    assert first_run["commandArgs"][:2] == ["--record-agent-session", "--analyzer"]
    assert "--prompt" in first_run["commandArgs"]
    assert "--max-asp-bash-commands" in first_run["commandArgs"]


def test_adaptive_validation_manifest_cli_writes_output(tmp_path: Path) -> None:
    policy_path = tmp_path / "policy.json"
    output_path = tmp_path / "manifest.json"
    policy_path.write_text(json.dumps(_policy()), encoding="utf-8")

    assert (
        main(
            [
                "--repo-root",
                str(_ROOT),
                "--large-library-adaptive-validation-manifest",
                str(policy_path),
                "--validation-session-root",
                ".cache/validation",
                "--output",
                str(output_path),
            ]
        )
        == 0
    )

    manifest = json.loads(output_path.read_text(encoding="utf-8"))
    _validate_schema(
        manifest,
        "semantic-graph-turbo-adaptive-validation-manifest.v1.schema.json",
    )
    assert manifest["summary"]["runCount"] == 2
