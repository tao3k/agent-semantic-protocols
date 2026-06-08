"""Agent-benefit report CLI tests for graph turbo behavior quality."""

from __future__ import annotations

import json
import os
import subprocess
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[2] / "tests/unit"))

from asp_graph_turbo_cli_support import validate_shared_schema


def test_graph_turbo_agent_benefit_reports_read_locator_feedback_and_matrix() -> None:
    payload = _run_agent_benefit(
        _fixture_path("graph-turbo-sensitive-ablation.json"),
        "--scenario",
        "unit.sensitive",
        "--receipt",
        str(_fixture_path("graph-turbo-sensitive-receipt.json")),
        "--require-useful-locator",
        "--max-useful-locator-rank",
        "1",
        "--max-commands-to-first-useful-locator",
        "1",
        "--require-repeated-mistake-suppression",
        "--require-profile-matrix-explanation",
        "--fail-on-quality-gate",
    )

    validate_shared_schema(
        payload,
        "semantic-graph-turbo-agent-benefit.v1.schema.json",
    )
    assert payload["qualityGate"]["status"] == "pass"
    assert payload["usefulLocator"]["rank"] == 1
    assert payload["usefulLocator"]["commandsToFirstUsefulLocator"] == 1
    assert payload["feedbackLearning"]["repeatedMistakeSuppressed"] is True
    assert payload["profileMatrixExplanation"]["explained"] is True


def test_graph_turbo_agent_benefit_reports_failure_evidence_first() -> None:
    payload = _run_agent_benefit(
        _fixture_path("graph-turbo-failure-evidence.json"),
        "--scenario",
        "unit.failure",
        "--require-failure-evidence",
        "--max-failure-evidence-rank",
        "2",
        "--require-useful-locator",
        "--max-useful-locator-rank",
        "2",
        "--require-profile-matrix-explanation",
        "--fail-on-quality-gate",
    )

    validate_shared_schema(
        payload,
        "semantic-graph-turbo-agent-benefit.v1.schema.json",
    )
    assert payload["qualityGate"]["status"] == "pass"
    assert payload["failureEvidence"]["kind"] == "assert"
    assert payload["failureEvidence"]["rank"] == 2


def test_graph_turbo_agent_benefit_can_fail_quality_gate() -> None:
    completed = _run_agent_benefit_process(
        _fixture_path("graph-turbo-failure-evidence.json"),
        "--scenario",
        "unit.fail",
        "--require-failure-evidence",
        "--max-failure-evidence-rank",
        "1",
        "--fail-on-quality-gate",
    )
    payload = json.loads(completed.stdout)

    assert completed.returncode == 1
    assert payload["qualityGate"]["status"] == "fail"
    assert payload["qualityGate"]["failures"][0]["field"] == "failureEvidence.rank"


def _run_agent_benefit(packet_path: Path, *args: str) -> dict[str, object]:
    completed = _run_agent_benefit_process(packet_path, *args)
    assert completed.returncode == 0, completed.stderr
    payload = json.loads(completed.stdout)
    assert isinstance(payload, dict)
    return payload


def _run_agent_benefit_process(
    packet_path: Path,
    *args: str,
) -> subprocess.CompletedProcess[str]:
    return subprocess.run(
        [
            sys.executable,
            "-m",
            "asp_graph_turbo",
            "agent-benefit",
            str(packet_path),
            *args,
            "--format",
            "json",
        ],
        text=True,
        capture_output=True,
        env=_subprocess_env(),
    )


def _fixture_path(name: str) -> Path:
    return Path(__file__).resolve().parents[2] / f"sandtables/fixtures/asp/{name}"


def _subprocess_env() -> dict[str, str]:
    repo_root = Path(__file__).resolve().parents[2]
    package_src = repo_root / "packages/python/asp_graph_turbo/src"
    unit_tests = repo_root / "tests/unit"
    env = os.environ.copy()
    env["PYTHONPATH"] = os.pathsep.join(
        [str(package_src), str(unit_tests), env.get("PYTHONPATH", "")]
    ).rstrip(os.pathsep)
    return env
