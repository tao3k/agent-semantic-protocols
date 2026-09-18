# SPDX-FileCopyrightText: 2026 tao3k team and Contributors
#
# SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

"""Wire alignment for exact-content Search execution frames."""

from __future__ import annotations

from copy import deepcopy
from pathlib import Path

import pytest
from jsonschema import Draft202012Validator, ValidationError

from unit.schema_validation import schema_validator_for


SCHEMA_PATH = Path(__file__).resolve().parents[2] / "schemas/search-execution.schema.json"


@pytest.fixture(scope="module")
def validator():
    result = schema_validator_for(SCHEMA_PATH)
    Draft202012Validator.check_schema(result.schema)
    return result


def frame(operation: str = "playbook") -> dict[str, object]:
    digest = "blake3-256:" + "a" * 64
    return {
        "frameSchemaId": "asp.search-execution",
        "frameSchemaVersion": "1",
        "requestId": "request-1",
        "sessionId": "session-1",
        "operation": operation,
        "context": {
            "binding": {
                "schemaId": "asp.content-binding",
                "schemaVersion": "1",
                "runtimeArtifactDigest": digest,
                "workspaceSnapshotDigest": digest,
                "sourceGenerationDigest": digest,
                "sourceIndexDigest": digest,
                "schemaDigest": digest,
                "providerCatalogDigest": digest,
                "authorityStamp": {
                    "keyId": "key-1",
                    "canonicalDigest": digest,
                    "signature": "signature-1",
                },
            },
            "commitDigest": digest,
        },
        "cancellationId": "cancel-1",
    }


def test_playbook_frame_resolves_canonical_content_binding(validator) -> None:
    validator.validate(frame())


@pytest.mark.parametrize("operation", ["query", "exact"])
def test_selector_operations_require_a_selector(validator, operation: str) -> None:
    payload = frame(operation)
    with pytest.raises(ValidationError):
        validator.validate(payload)
    payload["selector"] = "rust://src/lib.rs#item/function/run"
    validator.validate(payload)


def test_operation_is_closed_to_the_typed_rust_surface(validator) -> None:
    payload = deepcopy(frame())
    payload["operation"] = "search"
    with pytest.raises(ValidationError):
        validator.validate(payload)
