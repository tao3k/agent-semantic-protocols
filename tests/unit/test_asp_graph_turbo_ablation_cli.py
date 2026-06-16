"""Ablation packet generator tests for graph turbo sandtable calibration."""

from __future__ import annotations

import json
import os
import subprocess
import sys
from pathlib import Path

from asp_graph_turbo_cli_support import validate_shared_schema


def test_graph_turbo_ablation_cli_generates_packet_variants(tmp_path) -> None:
    packet_path = tmp_path / "graph-turbo-request.json"
    packet_path.write_text(json.dumps(_ablation_request()), encoding="utf-8")

    completed = subprocess.run(
        [
            sys.executable,
            "-m",
            "asp_graph_turbo",
            "ablate",
            str(packet_path),
            "--format",
            "json",
        ],
        check=True,
        text=True,
        capture_output=True,
        env=_subprocess_env(),
    )
    payload = json.loads(completed.stdout)
    validate_shared_schema(payload, "semantic-graph-turbo-ablation-set.v1.schema.json")
    variants = {entry["variant"]: entry["packet"] for entry in payload["variants"]}

    assert payload["packetKind"] == "graph-turbo-ablation-set"
    assert set(variants) == {
        "full",
        "no-package-cohesion",
        "no-query-clause-coverage",
        "no-query-seed-prior",
        "no-provider-facts",
        "no-quality-fields",
        "no-read-memory",
        "no-receipt",
        "relation-weight-flat",
    }
    assert variants["no-read-memory"]["readMemory"]["seenSelectors"] == []
    assert _node_kinds(variants["no-receipt"]) == {"field", "query", "test"}
    assert _node_kinds(variants["no-provider-facts"]) == {"query", "receipt"}
    assert all(
        "confidence" not in edge
        and "freshness" not in edge
        and "provenance" not in edge
        for edge in variants["no-quality-fields"]["graph"]["edges"]
    )
    flat_match = variants["relation-weight-flat"]["graph"]["edges"][0]
    assert flat_match["relation"] == "matches"
    assert flat_match["weight"] == 1.0 / 1.5
    assert variants["no-query-seed-prior"]["queryAdjustmentPolicy"] == {
        "seedPrior": False
    }
    assert variants["no-package-cohesion"]["queryAdjustmentPolicy"] == {
        "packageCohesion": False
    }
    assert variants["no-query-clause-coverage"]["queryAdjustmentPolicy"] == {
        "queryClauseCoverage": False
    }


def test_graph_turbo_ablation_cli_can_emit_one_variant_as_text(tmp_path) -> None:
    packet_path = tmp_path / "graph-turbo-request.json"
    packet_path.write_text(json.dumps(_ablation_request()), encoding="utf-8")

    completed = subprocess.run(
        [
            sys.executable,
            "-m",
            "asp_graph_turbo",
            "ablate",
            str(packet_path),
            "--variant",
            "no-quality-fields",
            "--format",
            "text",
        ],
        check=True,
        text=True,
        capture_output=True,
        env=_subprocess_env(),
    )

    assert completed.stdout.strip() == (
        "[graph-ablation] variants=1 names=no-quality-fields"
    )


def _node_kinds(packet: dict[str, object]) -> set[str]:
    graph = packet["graph"]
    assert isinstance(graph, dict)
    nodes = graph["nodes"]
    assert isinstance(nodes, list)
    return {str(node["kind"]) for node in nodes if isinstance(node, dict)}


def _subprocess_env() -> dict[str, str]:
    repo_root = Path(__file__).resolve().parents[2]
    package_src = repo_root / "packages/python/asp_graph_turbo/src"
    env = os.environ.copy()
    env["PYTHONPATH"] = (
        f"{package_src}{os.pathsep}{env['PYTHONPATH']}"
        if env.get("PYTHONPATH")
        else str(package_src)
    )
    return env


def _ablation_request() -> dict[str, object]:
    return {
        "schemaId": "agent.semantic-protocols.semantic-graph-turbo-request",
        "schemaVersion": "1",
        "protocolId": "agent.semantic-protocols.semantic-language",
        "protocolVersion": "1",
        "packetKind": "graph-turbo-request",
        "profile": "owner-query",
        "algorithm": "typed-ppr-diverse",
        "seedIds": ["query:vec"],
        "budget": 4,
        "readMemory": {"seenSelectors": ["src/lib.rs:10:12"]},
        "graph": {
            "nodes": [
                {
                    "id": "query:vec",
                    "kind": "query",
                    "role": "term",
                    "value": "Vec field",
                },
                {
                    "id": "field:items",
                    "kind": "field",
                    "role": "struct-field",
                    "value": "items: Vec<Item>",
                },
                {
                    "id": "test:items",
                    "kind": "test",
                    "role": "path",
                    "value": "tests/items.rs",
                },
                {
                    "id": "receipt:items",
                    "kind": "receipt",
                    "role": "feedback",
                    "value": "followed",
                },
            ],
            "edges": [
                {
                    "source": "query:vec",
                    "target": "field:items",
                    "relation": "matches",
                    "confidence": "exact",
                    "freshness": "fresh",
                    "provenance": "parser",
                },
                {
                    "source": "field:items",
                    "target": "test:items",
                    "relation": "covers",
                    "confidence": "high",
                },
                {
                    "source": "receipt:items",
                    "target": "field:items",
                    "relation": "selects",
                    "provenance": "receipt",
                },
            ],
        },
    }
