# SPDX-FileCopyrightText: 2026 tao3k team and Contributors
#
# SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

from __future__ import annotations

import json
from functools import cache
from pathlib import Path

from jsonschema import Draft202012Validator
from referencing import Registry, Resource


ROOT = Path(__file__).resolve().parents[2]
REQUEST_SCHEMA = ROOT / "schemas/provider-native-exact-request.v1.schema.json"
RESPONSE_SCHEMA = ROOT / "schemas/provider-native-exact-response.v1.schema.json"
DEPENDENCY_SCHEMAS = (
    ROOT / "schemas/canonical-item-selector.v1.schema.json",
    ROOT / "schemas/exact-structural-selector.v1.schema.json",
    ROOT / "schemas/callable-skeleton.schema.json",
)
SKELETON_FIXTURE = (
    ROOT
    / "schemas/fixtures/semantic-projection.callable-skeleton.v1.json"
)
EXACT_RESPONSE_FIXTURES = ROOT / "schemas/fixtures/provider-native-exact-response"


def _load(path: Path) -> dict[str, object]:
    return json.loads(path.read_text())


@cache
def _validator(path: Path) -> Draft202012Validator:
    schemas = [_load(item) for item in (*DEPENDENCY_SCHEMAS, path)]
    for schema in schemas:
        Draft202012Validator.check_schema(schema)
    registry = Registry().with_resources(
        [
            (str(schema["$id"]), Resource.from_contents(schema))
            for schema in schemas
        ]
    )
    return Draft202012Validator(schemas[-1], registry=registry)


def _response(projection_mode: str) -> dict[str, object]:
    return {
        "schemaId": "agent.semantic-protocols.provider-native-exact-projection",
        "schemaVersion": "1",
        "languageId": "rust",
        "providerId": "asp-rust",
        "ownerPath": "crates/example/src/dispatch.rs",
        "requestedStructuralSelector": (
            "rust://crates/example/src/dispatch.rs#item/function/run"
        ),
        "structuralSelector": (
            "rust://crates/example/src/dispatch.rs#item/function/run"
        ),
        "projectionMode": projection_mode,
        "normalizedParserFacts": {},
        "sourceContentDigest": "a" * 64,
        "sourceByteStart": 0,
        "sourceByteEnd": 100,
    }


def test_request_requires_explicit_v1_projection() -> None:
    request = {
        "schemaId": "agent.semantic-protocols.provider-native-exact-request",
        "schemaVersion": "1",
        "languageId": "rust",
        "providerId": "asp-rust",
        "projectionKind": "source",
        "structuralSelector": (
            "rust://crates/example/src/dispatch.rs#item/function/run"
        ),
        "ownerPath": "crates/example/src/dispatch.rs",
        "generationIdentityDigest": "b" * 64,
        "parserIdentityDigest": "c" * 64,
        "queryPackDigest": "d" * 64,
        "sourceDigest": "a" * 64,
        "sourceByteLength": 100,
        "sourceEncoding": "base64",
        "sourceBytesBase64": "Zm4gcnVuKCkge30=",
        "transport": "stdin-json",
    }
    _validator(REQUEST_SCHEMA).validate(request)


def test_source_response_requires_text_and_forbids_skeleton_payload() -> None:
    response = _response("source")
    response["projectionText"] = "fn run() {}"
    _validator(RESPONSE_SCHEMA).validate(response)

    response["projectionPayload"] = _load(SKELETON_FIXTURE)
    assert list(_validator(RESPONSE_SCHEMA).iter_errors(response))


def test_skeleton_response_requires_typed_payload_and_forbids_text() -> None:
    response = _response("callable-skeleton")
    response["projectionPayload"] = _load(SKELETON_FIXTURE)["payload"]
    _validator(RESPONSE_SCHEMA).validate(response)

    response["projectionText"] = "fn run() {}"
    assert list(_validator(RESPONSE_SCHEMA).iter_errors(response))


def test_skeleton_response_without_payload_fails_closed() -> None:
    response = _response("callable-skeleton")
    errors = list(_validator(RESPONSE_SCHEMA).iter_errors(response))
    assert any(
        error.validator == "required" and "projectionPayload" in error.message
        for error in errors
    )


def test_live_owner_item_missing_is_terminal_typed_evidence() -> None:
    response = _load(EXACT_RESPONSE_FIXTURES / "valid-terminal-item-missing.v1.json")
    _validator(RESPONSE_SCHEMA).validate(response)
    assert "recommendedNext" not in response


def test_live_owner_item_missing_cannot_request_repeat_discovery() -> None:
    response = _load(
        EXACT_RESPONSE_FIXTURES / "invalid-item-missing-repeat-search.v1.json"
    )
    errors = list(_validator(RESPONSE_SCHEMA).iter_errors(response))
    assert any(error.validator == "not" for error in errors)
