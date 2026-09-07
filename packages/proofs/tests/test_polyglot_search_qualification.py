# SPDX-FileCopyrightText: 2026 tao3k team and Contributors
#
# SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

from __future__ import annotations

import copy
import json
from dataclasses import replace
from pathlib import Path

from jsonschema import Draft202012Validator

from asp_proofs.polyglot_search_qualification import (
    digest,
    qualification_payload,
    qualify,
    reference_scenarios,
    render_json_fixture,
)


REPOSITORY_ROOT = Path(__file__).resolve().parents[3]
SCHEMA_PATH = (
    REPOSITORY_ROOT / "schemas/semantic-polyglot-search-qualification.v1.schema.json"
)
FIXTURE_PATH = (
    REPOSITORY_ROOT
    / "tests/fixtures/semantic_polyglot_search/qualification.valid.json"
)


def test_reference_qualification_is_schema_valid_and_byte_stable() -> None:
    generated = render_json_fixture()
    schema = json.loads(SCHEMA_PATH.read_text(encoding="utf-8"))

    Draft202012Validator(schema).validate(json.loads(generated))
    assert generated.encode() == FIXTURE_PATH.read_bytes()
    assert generated.endswith("\n")


def test_reference_qualification_is_deterministic_and_digest_bound() -> None:
    assert render_json_fixture() == render_json_fixture()
    payload = qualification_payload()
    claimed = payload.pop("payloadDigest")

    assert digest(payload) == claimed


def test_reference_candidate_satisfies_every_scenario_and_aggregate_gate() -> None:
    payload = qualification_payload()

    assert payload["decision"] == {"state": "accepted", "reasons": []}
    assert payload["aggregate"]["admittedScenarioCount"] == 4
    assert payload["aggregate"]["candidateTotalTokens"] == 2400
    assert payload["aggregate"]["legacyTotalTokens"] == 3900
    assert payload["aggregate"]["coldScenarioCount"] == 1
    assert payload["aggregate"]["warmScenarioCount"] == 3


def test_average_token_win_cannot_hide_one_scenario_budget_failure() -> None:
    scenarios = list(reference_scenarios())
    failing_candidate = replace(scenarios[0].candidate, totalTokens=701)
    scenarios[0] = replace(scenarios[0], candidate=failing_candidate)
    result = qualify(scenarios)

    assert result["aggregate"]["candidateTotalTokens"] < result["aggregate"]["legacyTotalTokens"]
    assert result["scenarios"][0]["reasons"] == ["token-ratio"]
    assert result["decision"]["state"] == "rejected"
    assert "scenario-rejected" in result["decision"]["reasons"]


def test_quality_regression_rejects_an_otherwise_fast_candidate() -> None:
    scenarios = list(reference_scenarios())
    failing_candidate = replace(scenarios[2].candidate, recallBps=9000)
    scenarios[2] = replace(scenarios[2], candidate=failing_candidate)
    result = qualify(scenarios)

    assert result["scenarios"][2]["reasons"] == ["recall"]
    assert result["decision"]["state"] == "rejected"


def test_cache_coverage_and_read_only_state_are_hard_gates() -> None:
    scenarios = list(reference_scenarios())
    no_warm = [replace(item, cacheMode="cold", candidate=replace(item.candidate, cacheHit=False)) for item in scenarios]
    coverage_result = qualify(no_warm)
    mutated_candidate = replace(
        scenarios[1].candidate,
        stateDigestAfter=scenarios[1].candidate.resultDigest,
    )
    scenarios[1] = replace(scenarios[1], candidate=mutated_candidate)
    mutation_result = qualify(scenarios)

    assert "warm-scenario-missing" in coverage_result["decision"]["reasons"]
    assert mutation_result["scenarios"][1]["reasons"] == ["candidate-read-only"]
    assert mutation_result["decision"]["state"] == "rejected"


def test_tampering_preserves_shape_but_breaks_payload_identity() -> None:
    payload = qualification_payload()
    tampered = copy.deepcopy(payload)
    tampered["scenarios"][0]["candidate"]["totalTokens"] = 1
    claimed = tampered.pop("payloadDigest")

    assert digest(tampered) != claimed
