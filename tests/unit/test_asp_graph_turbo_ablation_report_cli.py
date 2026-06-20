"""Ablation report CLI tests for graph turbo calibration."""

from __future__ import annotations

import json
import os
import subprocess
import sys
from pathlib import Path

from asp_graph_turbo_cli_support import (
    sample_graph_turbo_request,
    validate_shared_schema,
)


def test_graph_turbo_ablation_report_cli_generates_schema_packet(tmp_path) -> None:
    packet_path = tmp_path / "graph-turbo-request.json"
    packet_path.write_text(json.dumps(sample_graph_turbo_request()), encoding="utf-8")

    completed = subprocess.run(
        [
            sys.executable,
            "-m",
            "asp_graph_turbo",
            "ablation-report",
            str(packet_path),
            "--runs",
            "1",
            "--warmup-runs",
            "0",
            "--cache-mode",
            "disabled",
            "--format",
            "json",
        ],
        check=True,
        text=True,
        capture_output=True,
        env=_subprocess_env(),
    )
    payload = json.loads(completed.stdout)
    validate_shared_schema(
        payload,
        "semantic-graph-turbo-ablation-report.v1.schema.json",
    )
    variants = {entry["variant"]: entry for entry in payload["variants"]}

    assert payload["packetKind"] == "graph-turbo-ablation-report"
    assert payload["summary"]["variantCount"] == 10
    assert payload["qualityGate"]["status"] == "pass"
    assert "queryFirstStage" in payload["qualityGate"]["signals"]
    assert variants["full"]["comparison"]["rankOverlapRatio"] == 1.0
    assert variants["full"]["comparison"]["scoreDeltaL1"] == 0.0
    assert "transitionNonZeroDelta" in variants["no-provider-facts"]["comparison"]
    assert "querySeedPriorCountDelta" in variants["no-query-seed-prior"]["comparison"]


def test_graph_turbo_ablation_report_cli_can_render_text(tmp_path) -> None:
    packet_path = tmp_path / "graph-turbo-request.json"
    packet_path.write_text(json.dumps(sample_graph_turbo_request()), encoding="utf-8")

    completed = subprocess.run(
        [
            sys.executable,
            "-m",
            "asp_graph_turbo",
            "ablation-report",
            str(packet_path),
            "--variant",
            "no-provider-facts",
            "--runs",
            "1",
            "--warmup-runs",
            "0",
            "--format",
            "text",
        ],
        check=True,
        text=True,
        capture_output=True,
        env=_subprocess_env(),
    )

    assert completed.stdout.startswith("[graph-ablation-report] ")
    assert "gate=pass" in completed.stdout
    assert "variant=no-provider-facts" in completed.stdout
    assert "readMemoryDelta=" in completed.stdout
    assert "receiptBoostDelta=" in completed.stdout
    assert "transitionNnzDelta=" in completed.stdout
    assert "querySeedDelta=" in completed.stdout


def test_graph_turbo_ablation_report_exposes_receipt_read_memory_and_quality_deltas(
    tmp_path,
) -> None:
    packet_path = tmp_path / "graph-turbo-sensitive-request.json"
    packet_path.write_text(
        _sensitive_fixture_path().read_text(encoding="utf-8"),
        encoding="utf-8",
    )

    completed = subprocess.run(
        [
            sys.executable,
            "-m",
            "asp_graph_turbo",
            "ablation-report",
            str(packet_path),
            "--runs",
            "1",
            "--warmup-runs",
            "0",
            "--cache-mode",
            "disabled",
            "--format",
            "json",
        ],
        check=True,
        text=True,
        capture_output=True,
        env=_subprocess_env(),
    )
    payload = json.loads(completed.stdout)
    validate_shared_schema(
        payload,
        "semantic-graph-turbo-ablation-report.v1.schema.json",
    )
    variants = {entry["variant"]: entry for entry in payload["variants"]}

    assert variants["no-read-memory"]["comparison"]["readMemorySuppressedDelta"] < 0
    assert variants["no-receipt"]["comparison"]["receiptBoostDelta"] < 0
    assert variants["no-quality-fields"]["comparison"]["scoreDeltaL1"] > 0


def test_graph_turbo_ablation_report_exposes_query_first_stage_signal(
    tmp_path,
) -> None:
    packet_path = tmp_path / "graph-turbo-query-request.json"
    packet_path.write_text(json.dumps(_query_first_stage_request()), encoding="utf-8")

    completed = subprocess.run(
        [
            sys.executable,
            "-m",
            "asp_graph_turbo",
            "ablation-report",
            str(packet_path),
            "--runs",
            "1",
            "--warmup-runs",
            "0",
            "--cache-mode",
            "disabled",
            "--format",
            "json",
        ],
        check=True,
        text=True,
        capture_output=True,
        env=_subprocess_env(),
    )
    payload = json.loads(completed.stdout)
    validate_shared_schema(
        payload,
        "semantic-graph-turbo-ablation-report.v1.schema.json",
    )
    variants = {entry["variant"]: entry for entry in payload["variants"]}

    assert payload["qualityGate"]["signals"]["queryFirstStage"] is True
    assert (
        variants["no-query-seed-prior"]["comparison"]["querySeedPriorCountDelta"]
        < 0
    )
    assert (
        variants["no-package-cohesion"]["comparison"][
            "queryPackageCohesionCountDelta"
        ]
        < 0
    )
    assert (
        variants["no-query-clause-coverage"]["comparison"][
            "queryClauseCoverageCountDelta"
        ]
        < 0
    )
    assert (
        variants["no-local-evidence"]["comparison"][
            "queryLocalEvidenceBoostCountDelta"
        ]
        < 0
    )


def test_graph_turbo_ablation_report_can_fail_quality_gate(tmp_path) -> None:
    packet_path = tmp_path / "graph-turbo-sensitive-request.json"
    packet_path.write_text(
        _sensitive_fixture_path().read_text(encoding="utf-8"),
        encoding="utf-8",
    )

    completed = subprocess.run(
        [
            sys.executable,
            "-m",
            "asp_graph_turbo",
            "ablation-report",
            str(packet_path),
            "--runs",
            "1",
            "--warmup-runs",
            "0",
            "--cache-mode",
            "disabled",
            "--min-worst-rank-overlap-ratio",
            "0.95",
            "--fail-on-quality-gate",
            "--format",
            "json",
        ],
        text=True,
        capture_output=True,
        env=_subprocess_env(),
    )
    payload = json.loads(completed.stdout)

    assert completed.returncode == 1
    assert payload["qualityGate"]["status"] == "fail"
    assert payload["qualityGate"]["failures"][0]["field"] == (
        "summary.worstRankOverlapRatio"
    )


def _subprocess_env() -> dict[str, str]:
    repo_root = Path(__file__).resolve().parents[2]
    package_src = repo_root / "packages/python/asp_graph_turbo/src"
    unit_tests = repo_root / "tests/unit"
    env = os.environ.copy()
    env["PYTHONPATH"] = os.pathsep.join(
        [str(package_src), str(unit_tests), env.get("PYTHONPATH", "")]
    ).rstrip(os.pathsep)
    return env


def _sensitive_fixture_path() -> Path:
    return (
        Path(__file__).resolve().parents[2]
        / "sandtables/fixtures/asp/graph-turbo-sensitive-ablation.json"
    )


def _query_first_stage_request() -> dict[str, object]:
    packet = sample_graph_turbo_request()
    packet["queryTerms"] = [
        "asp_graph_turbo",
        "queryClauses",
        "typed",
        "graph",
        "request",
    ]
    packet["queryClauses"] = [
        "asp_graph_turbo queryClauses clause coverage scoring",
        "typed graph request rank objective",
    ]
    packet["seedIds"] = [
        "query:asp_graph_turbo",
        "owner:packages/python/asp_graph_turbo/src/asp_graph_turbo/ranking_score.py",
    ]
    graph = packet["graph"]
    assert isinstance(graph, dict)
    graph["nodes"] = _query_first_stage_nodes()
    graph["edges"] = _query_first_stage_edges()
    return packet


def _query_first_stage_nodes() -> list[dict[str, object]]:
    return [
        {
            "id": "query:asp_graph_turbo",
            "kind": "query",
            "role": "term",
            "value": "asp_graph_turbo queryClauses typed graph request",
            "action": "fzf",
        },
        {
            "id": "owner:packages/python/asp_graph_turbo/src/asp_graph_turbo/ranking_score.py",
            "kind": "owner",
            "role": "path",
            "value": "packages/python/asp_graph_turbo/src/asp_graph_turbo/ranking_score.py",
            "path": "packages/python/asp_graph_turbo/src/asp_graph_turbo/ranking_score.py",
            "ownerPath": "packages/python/asp_graph_turbo/src/asp_graph_turbo/ranking_score.py",
        },
        {
            "id": "item:package-and-request",
            "kind": "item",
            "role": "symbol",
            "value": "queryClauses coverage scoring typed graph request rank",
            "path": "packages/python/asp_graph_turbo/src/asp_graph_turbo/ranking_score.py",
            "ownerPath": "packages/python/asp_graph_turbo/src/asp_graph_turbo/ranking_score.py",
            "symbol": "queryClauses",
        },
        {
            "id": "item:rust-mention",
            "kind": "item",
            "role": "symbol",
            "value": "asp_graph_turbo queryClauses",
            "path": "crates/agent-semantic-client/tests/unit/search_history.rs",
            "ownerPath": "crates/agent-semantic-client/tests/unit/search_history.rs",
            "symbol": "asp_graph_turbo",
        },
        {
            "id": "test:ranking-score",
            "kind": "test",
            "role": "path",
            "value": "tests/unit/test_asp_graph_turbo_ranking_query.py",
        },
    ]


def _query_first_stage_edges() -> list[dict[str, str]]:
    return [
        {
            "source": "query:asp_graph_turbo",
            "target": "item:package-and-request",
            "relation": "matches",
        },
        {
            "source": "query:asp_graph_turbo",
            "target": "item:rust-mention",
            "relation": "matches",
        },
        {
            "source": "owner:packages/python/asp_graph_turbo/src/asp_graph_turbo/ranking_score.py",
            "target": "item:package-and-request",
            "relation": "contains",
        },
        {
            "source": "owner:packages/python/asp_graph_turbo/src/asp_graph_turbo/ranking_score.py",
            "target": "test:ranking-score",
            "relation": "covers",
        },
    ]
