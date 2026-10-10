# SPDX-FileCopyrightText: 2026 tao3k team and Contributors
#
# SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

from __future__ import annotations

import json
import tomllib
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
LOCK_PATH = ROOT / "benchmarks/large-library-runtime-corpora.json"
PLAN_PATH = ROOT / "benchmarks/live-corpus-scheme-scenarios.v1.toml"
WORKFLOW_PATH = ROOT / ".github/workflows/large-library-runtime-benchmark.yml"


def load(path: Path) -> dict[str, object]:
    return json.loads(path.read_text(encoding="utf-8"))


def load_plan() -> dict[str, object]:
    with PLAN_PATH.open("rb") as stream:
        return tomllib.load(stream)


def test_every_locked_live_corpus_has_one_fixed_search_query_case() -> None:
    lock = load(LOCK_PATH)
    plan = load_plan()
    corpora = lock["corpora"]
    cases = plan["cases"]

    assert len(corpora) == 17
    assert len(cases) == 17
    assert plan["client_protocol"]["applies_to_case_count"] == 17
    assert plan["client_protocol"]["transport"] == "grpc-tokio-streams"
    assert plan["client_protocol"]["workspace_scheduling"] == "tokio-join-set"
    assert plan["client_protocol"]["phases"] == [
        "initialize", "catalog", "request", "cancel", "cancelled", "shutdown"
    ]

    locked = {
        corpus["resourceId"]: (
            corpus["scenarioId"],
            corpus["language"],
            corpus["providerId"],
        )
        for corpus in corpora
    }
    planned = {
        case["resource_id"]: (
            case["scenario_id"],
            case["language_id"],
            case["provider_id"],
        )
        for case in cases
    }
    assert len(locked) == len(corpora)
    assert len(planned) == len(cases)
    assert planned == locked

    case_ids = [case["case_id"] for case in cases]
    assert all(case_ids)
    assert len(set(case_ids)) == len(case_ids)


def test_required_languages_are_exactly_the_locked_language_set() -> None:
    lock = load(LOCK_PATH)
    plan = load_plan()
    locked_languages = {corpus["language"] for corpus in lock["corpora"]}
    assert set(plan["required_languages"]) == locked_languages
    assert locked_languages == {
        "gerbil-scheme",
        "julia",
        "md",
        "org",
        "python",
        "rust",
        "typescript",
    }


def test_client_protocol_contract_is_one_typed_runtime_lifecycle() -> None:
    contract = load_plan()["client_protocol"]
    assert contract["protocol_id"] == "agent.semantic-protocols.client"
    assert contract["protocol_version"] == "1"
    assert contract["maximum_resident_micros"] == 1000
    assert contract["required_telemetry_events"] == [
        "client_protocol_initialize",
        "client_protocol_catalog",
        "client_protocol_request",
        "client_protocol_cancel",
        "client_protocol_cancelled",
        "client_protocol_shutdown",
    ]


def test_live_corpus_workflow_uses_the_canonical_lock() -> None:
    workflow = WORKFLOW_PATH.read_text(encoding="utf-8")
    canonical = "benchmarks/large-library-runtime-corpora.json"
    assert canonical in workflow
    assert "benchmarks/large-library-runtime-corpora.v1.json" not in workflow
