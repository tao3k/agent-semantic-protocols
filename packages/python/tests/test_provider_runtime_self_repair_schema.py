# SPDX-FileCopyrightText: 2026 tao3k team and Contributors
#
# SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

from __future__ import annotations

import json
from pathlib import Path

from jsonschema import Draft202012Validator


ROOT = Path(__file__).resolve().parents[3]
SCHEMA_PATH = ROOT / "schemas/provider-runtime-self-repair-receipt.v1.schema.json"
DIGEST = "a" * 64


def repaired_receipt() -> dict[str, object]:
    return {
        "schemaId": "asp.provider-runtime-self-repair-receipt.v1",
        "schemaVersion": "1",
        "languageId": "rust",
        "providerId": "asp-rust",
        "trigger": "provider-contract-drift",
        "installReceiptDigest": DIGEST,
        "sourceIdentityDigest": DIGEST,
        "buildRecipeDigest": DIGEST,
        "providerManifestDigest": DIGEST,
        "contractClosureDigest": DIGEST,
        "previousArtifactDigest": DIGEST,
        "repairedArtifactDigest": "b" * 64,
        "result": "repaired",
        "atomicSwitch": True,
        "activationRegenerated": True,
        "policyCoverageValidated": True,
        "enforcementState": "fail-closed",
        "directSourceReads": 0,
        "legacyAdapters": 0,
        "timings": {
            "receiptValidationMicros": 100,
            "buildMillis": 1200,
            "contractProbeMillis": 4,
            "activationPublicationMillis": 2,
        },
    }


def validator() -> Draft202012Validator:
    return Draft202012Validator(json.loads(SCHEMA_PATH.read_text()))


def test_success_requires_complete_atomic_repair() -> None:
    validator().validate(repaired_receipt())


def test_success_rejects_legacy_adapter() -> None:
    receipt = repaired_receipt()
    receipt["legacyAdapters"] = 1
    errors = list(validator().iter_errors(receipt))
    assert errors


def test_failure_remains_fail_closed_and_requires_reflection() -> None:
    receipt = repaired_receipt()
    receipt.update(
        {
            "result": "failed",
            "atomicSwitch": False,
            "activationRegenerated": False,
            "policyCoverageValidated": False,
            "failure": {
                "reasonKind": "contract-probe-failed",
                "message": "provider emitted a non-current project-resolution packet",
            },
        }
    )
    receipt.pop("repairedArtifactDigest")
    validator().validate(receipt)
