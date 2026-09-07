# SPDX-FileCopyrightText: 2026 tao3k team and Contributors
#
# SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

import json
from pathlib import Path

from jsonschema import Draft202012Validator


ROOT = Path(__file__).resolve().parents[2]
RESPONSE_SCHEMA = ROOT / "schemas" / "provider-native-exact-response.v1.schema.json"


def _resolution(state: str) -> dict[str, object]:
    reason_kind = {
        "item-missing": "item-not-in-live-owner",
        "selector-stale": "selector-not-in-active-generation",
        "kind-mismatch": "owner-item-kind-mismatch",
        "ambiguous": "multiple-owner-items",
    }[state]
    packet: dict[str, object] = {
        "schemaId": "agent.semantic-protocols.provider-native-exact-projection",
        "schemaVersion": "1",
        "languageId": "rust",
        "providerId": "agent.semantic-protocols.providers.rust.asp-rust",
        "ownerPath": "src/lib.rs",
        "requestedStructuralSelector": "rust://src/lib.rs#item/function/missing",
        "resolutionState": state,
        "reasonKind": reason_kind,
        "activeGenerationDigest": f"blake3-256:{'a' * 64}",
        "rootDigest": "b" * 64,
        "itemKind": "function",
        "itemName": "missing",
        "candidates": [],
        "actualKinds": [],
    }
    if state != "item-missing":
        packet["recommendedNext"] = {
            "command": "asp rust search lexical --query 'missing' --query 'function missing' --workspace . --view seeds"
        }
    return packet


def test_exact_resolution_states_are_schema_valid_semantic_results() -> None:
    schema = json.loads(RESPONSE_SCHEMA.read_text())
    validator = Draft202012Validator(schema)

    for state in ("item-missing", "selector-stale", "kind-mismatch", "ambiguous"):
        validator.validate(_resolution(state))


def test_exact_resolution_requires_reason_and_recovery_action() -> None:
    schema = json.loads(RESPONSE_SCHEMA.read_text())
    validator = Draft202012Validator(schema)
    packet = _resolution("selector-stale")
    packet.pop("recommendedNext")

    assert not list(validator.iter_errors(packet))


def test_selector_stale_requires_active_generation_evidence() -> None:
    schema = json.loads(RESPONSE_SCHEMA.read_text())
    validator = Draft202012Validator(schema)
    packet = _resolution("selector-stale")
    packet.pop("activeGenerationDigest")

    assert list(validator.iter_errors(packet))
