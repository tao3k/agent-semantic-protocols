import json
from pathlib import Path

import jsonschema


ROOT = Path(__file__).resolve().parents[2]
SCHEMA = json.loads(
    (ROOT / "schemas/workspace-search-playbook-acquisition-receipt.v1.schema.json").read_text()
)


def receipt() -> dict:
    axis = {
        "branchCandidateOwnerPaths": [["src/registry.rs"]],
        "candidateOwnerPaths": ["src/registry.rs"],
    }
    return {
        "schemaId": "agent.semantic-protocols.local-search-playbook-acquisition-receipt",
        "schemaVersion": "1",
        "contentGenerationDigest": f"blake3-256:{'a' * 64}",
        "ownerCount": 1,
        "fd": axis,
        "rg": axis,
        "tantivy": axis,
        "candidateOwnerPaths": ["src/registry.rs"],
        "exactSelectors": [],
    }


def test_local_acquisition_receipt_is_valid_without_selector_authority() -> None:
    jsonschema.Draft202012Validator(SCHEMA).validate(receipt())


def test_local_acquisition_cannot_mint_exact_selectors() -> None:
    value = receipt()
    value["exactSelectors"] = ["rust://src/registry.rs#item/struct/Registry"]
    errors = list(jsonschema.Draft202012Validator(SCHEMA).iter_errors(value))
    assert errors
