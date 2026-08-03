from __future__ import annotations

import argparse
import hashlib
import json
from dataclasses import asdict, dataclass
from typing import Any, Sequence


SCHEMA_ID = "agent.semantic-protocols.polyglot-search-evidence-sufficiency.v1"
GENERATOR_ID = "asp-proofs-polyglot-search-evidence-sufficiency"
GENERATOR_VERSION = "1"


def canonical_json(value: Any) -> str:
    return json.dumps(value, ensure_ascii=False, separators=(",", ":"), sort_keys=True)


def digest(value: Any) -> str:
    return "sha256:" + hashlib.sha256(canonical_json(value).encode()).hexdigest()


@dataclass(frozen=True)
class Evidence:
    candidateExecutable: bool
    candidateArtifactBound: bool
    pairedTaskIdentity: bool
    pairedSourceSnapshot: bool
    pairedEnvironment: bool
    rawMetricsComplete: bool
    coldCacheCovered: bool
    warmCacheCovered: bool
    repeatedSamples: bool
    qualificationPassed: bool


PAIRED_FIELDS = (
    "pairedTaskIdentity",
    "pairedSourceSnapshot",
    "pairedEnvironment",
    "rawMetricsComplete",
    "coldCacheCovered",
    "warmCacheCovered",
    "repeatedSamples",
)

BLOCKER_BY_FIELD = {
    "candidateExecutable": "candidate-executable-missing",
    "candidateArtifactBound": "candidate-artifact-identity-missing",
    "pairedTaskIdentity": "paired-task-identity-missing",
    "pairedSourceSnapshot": "paired-source-snapshot-missing",
    "pairedEnvironment": "paired-environment-missing",
    "rawMetricsComplete": "raw-metrics-incomplete",
    "coldCacheCovered": "cold-cache-evidence-missing",
    "warmCacheCovered": "warm-cache-evidence-missing",
    "repeatedSamples": "repeated-samples-missing",
    "qualificationPassed": "qualification-not-passed",
}


def _inconsistent(evidence: Evidence) -> bool:
    paired_present = any(getattr(evidence, field) for field in PAIRED_FIELDS)
    return (
        (evidence.candidateArtifactBound and not evidence.candidateExecutable)
        or (paired_present and not (evidence.candidateExecutable and evidence.candidateArtifactBound))
        or (
            evidence.qualificationPassed
            and not all(getattr(evidence, field) for field in ("candidateExecutable", "candidateArtifactBound", *PAIRED_FIELDS))
        )
    )


def evaluate(evidence: Evidence) -> dict[str, Any]:
    missing = [
        BLOCKER_BY_FIELD[field]
        for field in BLOCKER_BY_FIELD
        if not getattr(evidence, field)
    ]
    if _inconsistent(evidence):
        return {
            "derivedState": "inconsistent",
            "decision": "blocked",
            "blockers": ["evidence-consistency-error", *missing],
        }
    if not evidence.candidateExecutable:
        state = "reference-only"
        decision = "blocked"
    elif not evidence.candidateArtifactBound:
        state = "executable-unbound"
        decision = "blocked"
    elif not all(getattr(evidence, field) for field in PAIRED_FIELDS):
        state = "executable-bound"
        decision = "ready-for-paired-runs"
    elif not evidence.qualificationPassed:
        state = "paired-measured"
        decision = "blocked"
    else:
        state = "qualified"
        decision = "qualified"
    return {"derivedState": state, "decision": decision, "blockers": missing}


def reference_evidence() -> Evidence:
    return Evidence(
        candidateExecutable=False,
        candidateArtifactBound=False,
        pairedTaskIdentity=False,
        pairedSourceSnapshot=False,
        pairedEnvironment=False,
        rawMetricsComplete=False,
        coldCacheCovered=False,
        warmCacheCovered=False,
        repeatedSamples=False,
        qualificationPassed=False,
    )


def evidence_payload(
    evidence: Evidence | None = None,
    *,
    candidate_runtime_digest: str | None = None,
    candidate_provider_digest: str | None = None,
    qualification_payload_digest: str | None = None,
) -> dict[str, Any]:
    active = evidence or reference_evidence()
    result = evaluate(active)
    artifact_identity_present = bool(candidate_runtime_digest and candidate_provider_digest)
    identity_consistent = active.candidateArtifactBound == artifact_identity_present
    qualification_consistent = (
        not active.qualificationPassed or qualification_payload_digest is not None
    )
    if not identity_consistent or not qualification_consistent:
        result = {
            "derivedState": "inconsistent",
            "decision": "blocked",
            "blockers": ["evidence-consistency-error", *result["blockers"]],
        }
    payload: dict[str, Any] = {
        "schemaId": SCHEMA_ID,
        "schemaVersion": "1",
        "generator": {"id": GENERATOR_ID, "version": GENERATOR_VERSION},
        "baselineSurface": {
            "kind": "production-executable",
            "surfaceId": "asp-language-search-query",
            "runtimeArtifactDigest": "fc12e93e11cdaa66413b1914bb959162167a2d55d61d0ee7785bcbc647eeec2b",
            "providerArtifactDigest": "blake3-256:e97a911c4fefc78360f7d90ca91ce7bac6044b5577ed496115e79bc4009a7dcc",
        },
        "candidateSurface": {
            "kind": "reference-only" if not active.candidateExecutable else "production-executable",
            "surfaceId": "asp-gql-logic-progressive-reference",
            "runtimeArtifactDigest": candidate_runtime_digest,
            "providerArtifactDigest": candidate_provider_digest,
        },
        "evidence": asdict(active),
        **result,
        "qualificationPayloadDigest": qualification_payload_digest,
    }
    payload["payloadDigest"] = digest(payload)
    return payload


def render_json_fixture() -> str:
    return canonical_json(evidence_payload()) + "\n"


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser()
    parser.add_argument("--format", choices=("json",), default="json")
    return parser


def main(argv: Sequence[str] | None = None) -> int:
    build_parser().parse_args(argv)
    print(render_json_fixture(), end="")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
