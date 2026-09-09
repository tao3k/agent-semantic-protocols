# SPDX-FileCopyrightText: 2026 tao3k team and Contributors
#
# SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

"""Validate the stable Runtime binary bundle and activation receipt schemas."""

import json
from pathlib import Path

from jsonschema import Draft202012Validator

from unit.schema_validation import schema_validator_for


ROOT = Path(__file__).resolve().parents[2]
SCHEMAS = ROOT / "schemas"
DIGEST_A = f"blake3-256:{'a' * 64}"
DIGEST_B = f"blake3-256:{'b' * 64}"


def bundle_fixture() -> dict[str, object]:
    return {
        "schemaId": "agent.semantic-protocols.runtime-binary-bundle",
        "schemaVersion": 1,
        "bundleDigest": DIGEST_A,
        "members": {
            "asp": DIGEST_A,
            "provider-registration.json": DIGEST_A,
            "provider-artifact-set": DIGEST_B,
            "evaluator-policy.json": DIGEST_A,
            "schema-bundle.json": DIGEST_B,
        },
        "executionBinding": {
            "schemaId": "agent.semantic-protocols.runtime-artifact-bundle-binding",
            "schemaVersion": "1",
            "providerRegistrationDigest": DIGEST_A,
            "providerArtifactSetDigest": DIGEST_B,
            "evaluatorPolicyDigest": DIGEST_A,
            "schemaBundleDigest": DIGEST_B,
        },
    }


def activation_fixture() -> dict[str, object]:
    return {
        "schemaId": "agent.semantic-protocols.runtime-artifact-activation",
        "schemaVersion": 1,
        "bundleDigest": DIGEST_A,
        "artifactDigest": DIGEST_B,
        "activationGeneration": 1,
        "artifactPath": "/state/runtime/artifacts/asp",
        "candidateSlotPath": "/state/runtime/resident/candidates/bundle",
        "previousArtifactDigest": None,
        "artifactMode": "dev",
        "publishedAtUnixMillis": 1,
        "publicationNonce": "publication-1",
        "candidateIdentity": {
            "artifactDigest": DIGEST_B,
            "artifactPath": "/state/runtime/artifacts/asp",
            "stablePath": "/state/runtime/bin/asp",
            "artifactMode": "dev",
            "publicationNonce": "publication-1",
        },
    }


def test_stable_schema_names_carry_numeric_version_one() -> None:
    for name in (
        "runtime-binary-bundle.schema.json",
        "runtime-artifact-activation.schema.json",
    ):
        assert ".v1." not in name
        schema = json.loads((SCHEMAS / name).read_text())
        Draft202012Validator.check_schema(schema)
        assert schema["properties"]["schemaVersion"]["const"] == 1


def test_bundle_and_activation_receipts_validate() -> None:
    schema_validator_for(SCHEMAS / "runtime-binary-bundle.schema.json").validate(
        bundle_fixture()
    )
    schema_validator_for(SCHEMAS / "runtime-artifact-activation.schema.json").validate(
        activation_fixture()
    )


def test_legacy_generation_partial_and_unknown_fields_fail_closed() -> None:
    bundle_validator = schema_validator_for(SCHEMAS / "runtime-binary-bundle.schema.json")
    invalid_bundle = bundle_fixture()
    invalid_bundle["members"] = {"../asp": DIGEST_A}
    assert list(bundle_validator.iter_errors(invalid_bundle))

    activation_validator = schema_validator_for(
        SCHEMAS / "runtime-artifact-activation.schema.json"
    )
    missing_bundle = activation_fixture()
    del missing_bundle["bundleDigest"]
    assert list(activation_validator.iter_errors(missing_bundle))

    unknown_generation = activation_fixture()
    unknown_generation["generation"] = 54
    assert list(activation_validator.iter_errors(unknown_generation))
