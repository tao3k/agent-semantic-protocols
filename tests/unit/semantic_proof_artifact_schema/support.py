"""Shared fixtures for formal proof artifact schema tests."""

from __future__ import annotations

import json
from pathlib import Path

from jsonschema import Draft202012Validator
from referencing import Registry, Resource
from referencing.jsonschema import DRAFT202012


REPO_ROOT = Path(__file__).resolve().parents[3]
SCHEMA_DIR = REPO_ROOT / "schemas"


def schema_registry() -> Registry:
    resources = []
    for path in SCHEMA_DIR.glob("*.schema.json"):
        with path.open("r", encoding="utf-8") as handle:
            contents = json.load(handle)
        resource = Resource.from_contents(contents, default_specification=DRAFT202012)
        resources.append((path.name, resource))
        resources.append((path.as_uri(), resource))
    return Registry().with_resources(resources)


def load_validator(name: str) -> Draft202012Validator:
    with (SCHEMA_DIR / name).open("r", encoding="utf-8") as handle:
        schema = json.load(handle)
    return Draft202012Validator(schema, registry=schema_registry())


def load_json(path: Path) -> dict:
    with path.open("r", encoding="utf-8") as handle:
        return json.load(handle)


def validation_errors(validator: Draft202012Validator, payload: dict) -> list[str]:
    return [error.message for error in validator.iter_errors(payload)]


def claims() -> list[dict]:
    return [
        {
            "id": "producer-bug-packet-invalid",
            "verified": True,
            "meaning": "A packet with no selector and pathLine identity violates the contract.",
            "theorem": "producer_bug_packet_invalid",
        },
        {
            "id": "defensive-renderer-not-compliant",
            "verified": True,
            "meaning": "The defensive renderer does not satisfy rendererCompliant.",
            "theorem": "defensive_renderer_not_compliant",
        },
    ]


def assessment() -> dict:
    return {
        "result": "blocked",
        "blockedBranch": "renderer-path-line-fallback",
        "whyBlocked": "The defensive renderer accepts invalid producer output.",
        "correctBoundary": "provider-result-construction-boundary",
        "replacementBranches": [
            "fix-provider-selector-construction",
            "schema-migration-if-contract-changed",
        ],
    }


def schema_projection() -> dict:
    return {
        "sourceSchema": "schemas/semantic-search-packet.v1.schema.json",
        "formalLeanPath": "schema-projection-formal.lean",
        "candidateLeanPath": "schema-projection-candidate.lean",
        "facts": [
            {
                "id": "item.structuralSelector",
                "owner": "item",
                "field": "structuralSelector",
                "role": "executable-selector",
            },
            {
                "id": "item.displayLineRange",
                "owner": "item",
                "field": "displayLineRange",
                "role": "display-only",
            },
        ],
    }


def packet_projection() -> dict:
    return {
        "sourceKind": "contract-fixture",
        "sourcePacket": "tests/fixtures/semantic_search_packet/bad_path_line_identity_packet.json",
        "formalLeanPath": "packet-projection-formal.lean",
        "candidateLeanPath": "packet-projection-candidate.lean",
        "identityKind": "path-line-only",
        "contractValid": False,
        "facts": [
            {
                "id": "bad_path_line_identity_packet.identityKind",
                "path": "$",
                "role": "identity-kind",
                "value": "path-line-only",
                "meaning": "Projected packet identity class used by the proof obligation.",
            },
            {
                "id": "bad_path_line_identity_packet.contractValid",
                "path": "$",
                "role": "contract-valid",
                "value": "false",
                "meaning": "True only when the packet exposes executable selector identity.",
            },
        ],
    }
