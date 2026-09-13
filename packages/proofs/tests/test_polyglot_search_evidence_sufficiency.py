# SPDX-FileCopyrightText: 2026 tao3k team and Contributors
#
# SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

from __future__ import annotations

import json
from dataclasses import replace
from pathlib import Path

from jsonschema import Draft202012Validator

from asp_proofs.polyglot_search_evidence_sufficiency import (
    Evidence,
    digest,
    evidence_payload,
    evaluate,
    reference_evidence,
    render_json_fixture,
)


REPOSITORY_ROOT = Path(__file__).resolve().parents[3]
SCHEMA_PATH = (
    REPOSITORY_ROOT
    / "schemas/semantic-polyglot-search-evidence-sufficiency.v1.schema.json"
)
FIXTURE_PATH = (
    REPOSITORY_ROOT
    / "tests/fixtures/semantic_polyglot_search/evidence-sufficiency.blocked.json"
)
QUALIFICATION_DIGEST = (
    "sha256:815d41e88d2230b6d89ec10a83c40f9544d45d80a80c6208b97b02c4daaa850c"
)


def executable_bound_evidence(**changes: bool) -> Evidence:
    base = replace(
        reference_evidence(),
        candidateExecutable=True,
        candidateArtifactBound=True,
    )
    return replace(base, **changes)


def test_current_blocked_receipt_is_schema_valid_and_byte_stable() -> None:
    generated = render_json_fixture()
    schema = json.loads(SCHEMA_PATH.read_text(encoding="utf-8"))

    Draft202012Validator(schema).validate(json.loads(generated))
    assert generated.encode() == FIXTURE_PATH.read_bytes()
    assert generated.endswith("\n")


def test_current_receipt_is_digest_bound_and_reference_only() -> None:
    payload = evidence_payload()
    claimed = payload.pop("payloadDigest")

    assert digest(payload) == claimed
    assert payload["derivedState"] == "reference-only"
    assert payload["decision"] == "blocked"
    assert payload["blockers"][0] == "candidate-executable-missing"


def test_executable_without_artifact_identity_remains_blocked() -> None:
    evidence = replace(reference_evidence(), candidateExecutable=True)

    assert evaluate(evidence)["derivedState"] == "executable-unbound"
    assert evaluate(evidence)["decision"] == "blocked"


def test_artifact_bound_candidate_is_only_ready_for_paired_runs() -> None:
    result = evaluate(executable_bound_evidence())

    assert result["derivedState"] == "executable-bound"
    assert result["decision"] == "ready-for-paired-runs"
    assert "raw-metrics-incomplete" in result["blockers"]


def test_complete_measurements_without_qualification_remain_blocked() -> None:
    evidence = executable_bound_evidence(
        pairedTaskIdentity=True,
        pairedSourceSnapshot=True,
        pairedEnvironment=True,
        rawMetricsComplete=True,
        coldCacheCovered=True,
        warmCacheCovered=True,
        repeatedSamples=True,
    )
    result = evaluate(evidence)

    assert result["derivedState"] == "paired-measured"
    assert result["decision"] == "blocked"
    assert result["blockers"] == ["qualification-not-passed"]


def test_qualification_requires_all_evidence_and_bound_artifacts() -> None:
    evidence = Evidence(*(True for _ in range(10)))
    payload = evidence_payload(
        evidence,
        candidate_runtime_digest="sha256:candidate-runtime",
        candidate_provider_digest="sha256:candidate-provider",
        qualification_payload_digest=QUALIFICATION_DIGEST,
    )
    schema = json.loads(SCHEMA_PATH.read_text(encoding="utf-8"))

    Draft202012Validator(schema).validate(payload)
    assert payload["derivedState"] == "qualified"
    assert payload["decision"] == "qualified"
    assert payload["blockers"] == []


def test_downstream_claim_without_prerequisites_is_inconsistent() -> None:
    evidence = replace(reference_evidence(), qualificationPassed=True)
    result = evaluate(evidence)

    assert result["derivedState"] == "inconsistent"
    assert result["decision"] == "blocked"
    assert result["blockers"][0] == "evidence-consistency-error"


def test_missing_qualification_digest_cannot_emit_qualified_receipt() -> None:
    evidence = Evidence(*(True for _ in range(10)))
    payload = evidence_payload(
        evidence,
        candidate_runtime_digest="sha256:candidate-runtime",
        candidate_provider_digest="sha256:candidate-provider",
    )

    assert payload["derivedState"] == "inconsistent"
    assert payload["decision"] == "blocked"
    assert payload["blockers"][0] == "evidence-consistency-error"
