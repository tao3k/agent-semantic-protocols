# SPDX-FileCopyrightText: 2026 tao3k team and Contributors
#
# SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

import json
from pathlib import Path

import pytest
from jsonschema import Draft202012Validator, ValidationError


SCHEMAS = Path(__file__).resolve().parents[2] / "schemas"
DIGEST = "blake3-256:" + "a" * 64


def _validator(name: str) -> Draft202012Validator:
    schema = json.loads((SCHEMAS / name).read_text())
    Draft202012Validator.check_schema(schema)
    return Draft202012Validator(schema)


def _inventory() -> dict:
    return {
        "schemaId": "agent.semantic-protocols.search-architecture-inventory",
        "schemaVersion": "1",
        "inventoryDigest": DIGEST,
        "productionFeatureSet": ["agent-semantic-client/default"],
        "productionEntries": ["generated-client"],
        "nodes": [
            {
                "nodeId": "generated-client",
                "sourceSelector": "rust://client#item/struct/ClientFrame",
                "declaredOwner": "generated-client",
                "capabilityClaims": ["frame-codec"],
            },
            {
                "nodeId": "old-fallback",
                "sourceSelector": "rust://client#item/function/legacy_fallback",
                "declaredOwner": "legacy-client-fallback",
                "capabilityClaims": ["retry-policy"],
            },
        ],
        "edges": [
            {
                "sourceNodeId": "generated-client",
                "targetNodeId": "old-fallback",
                "kind": "rust-reexport",
                "enabled": True,
                "evidenceSelector": "rust://client#item/reexport/legacy_fallback",
            }
        ],
    }


def _proof(state: str = "admitted") -> dict:
    return {
        "schemaId": "agent.semantic-protocols.search-architecture-proof-receipt",
        "schemaVersion": "1",
        "inventoryDigest": DIGEST,
        "modelDigest": DIGEST,
        "proofDigest": DIGEST,
        "state": state,
        "unknownNodes": [],
        "capabilityViolations": [],
        "legacyNodes": [],
        "productionToLegacyPaths": [],
        "legacyToAuthorityPaths": [],
        "axioms": [],
    }


def test_fact_inventory_is_neutral_about_legacy_classification() -> None:
    _validator("search-architecture-inventory.v1.schema.json").validate(_inventory())


def test_admitted_proof_requires_empty_legacy_impact_and_axioms() -> None:
    _validator("search-architecture-proof-receipt.v1.schema.json").validate(_proof())


def test_admitted_proof_rejects_a_legacy_path() -> None:
    proof = _proof()
    proof["productionToLegacyPaths"] = [["generated-client", "old-fallback"]]
    with pytest.raises(ValidationError):
        _validator("search-architecture-proof-receipt.v1.schema.json").validate(proof)


def test_rejected_proof_preserves_the_lean_counterexample() -> None:
    proof = _proof("rejected")
    proof["legacyNodes"] = ["old-fallback"]
    proof["productionToLegacyPaths"] = [["generated-client", "old-fallback"]]
    _validator("search-architecture-proof-receipt.v1.schema.json").validate(proof)


def test_proof_receipt_rejects_any_axiom() -> None:
    proof = _proof()
    proof["axioms"] = ["propext"]
    with pytest.raises(ValidationError):
        _validator("search-architecture-proof-receipt.v1.schema.json").validate(proof)
