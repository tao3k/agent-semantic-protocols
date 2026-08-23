import json
from pathlib import Path

import pytest
from jsonschema import Draft202012Validator, ValidationError


ROOT = Path(__file__).resolve().parents[2]
SCHEMA = json.loads(
    (ROOT / "schemas/runtime-merkle-owner-proof-qualification-receipt.schema.json").read_text()
)


def receipt(
    *, status: str = "qualified", evidence_layer: str = "scenario"
) -> dict[str, object]:
    digest = "blake3-256:" + "a" * 64
    qualified = status == "qualified"
    return {
        "schemaId": "agent.semantic-protocols.runtime-merkle-owner-proof-qualification-receipt",
        "schemaVersion": "1",
        "evidenceLayer": evidence_layer,
        "runtimeEcosystem": "tokio",
        "readMode": "synchronous-mmap",
        "caseId": "ready-owner-proof",
        "resourceId": None,
        "languageId": "rust",
        "providerId": "rs-harness",
        "workspaceIdentity": "workspace-proof",
        "generationDigest": digest if qualified else None,
        "rootDigest": digest if qualified else None,
        "ownerPath": "src/lib.rs" if qualified else None,
        "ownerContentDigest": digest if qualified else None,
        "ownerSubtreeDigest": digest if qualified else None,
        "structuralSelector": "rust://src/lib.rs#item/function/main" if qualified else None,
        "proofDigest": digest if qualified else None,
        "proofStepCount": 2 if qualified else None,
        "elapsedMicros": 17,
        "workCounters": {
            "providerProcessCount": 0,
            "schedulerTaskCount": 0,
            "filesystemReadCount": 0,
            "databaseReadCount": 0,
            "socketOperationCount": 0,
        },
        "status": status,
        "reasonKind": None if qualified else "runtime-merkle-owner-proof-invalid",
    }


def test_ready_receipt_requires_complete_identity_and_zero_warm_work() -> None:
    Draft202012Validator.check_schema(SCHEMA)
    Draft202012Validator(SCHEMA).validate(receipt())


def test_rejected_scenario_receipt_carries_a_typed_reason() -> None:
    Draft202012Validator(SCHEMA).validate(receipt(status="rejected"))


def test_live_corpus_uses_the_same_ready_proof_contract() -> None:
    live_corpus = receipt(evidence_layer="live-corpus")
    live_corpus["resourceId"] = "rust.tokio"
    Draft202012Validator(SCHEMA).validate(live_corpus)


def test_timeout_or_socket_work_cannot_qualify_a_warm_read() -> None:
    invalid = receipt()
    invalid["workCounters"]["socketOperationCount"] = 1
    with pytest.raises(ValidationError):
        Draft202012Validator(SCHEMA).validate(invalid)
