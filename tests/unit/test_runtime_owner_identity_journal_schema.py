import copy
import json
from pathlib import Path

import jsonschema


ROOT = Path(__file__).resolve().parents[2]
SCHEMA = json.loads(
    (ROOT / "schemas/runtime-owner-identity-journal.v1.schema.json").read_text()
)
DIGEST_A = "blake3-256:" + "a" * 64
DIGEST_B = "blake3-256:" + "b" * 64


def valid_packet() -> dict[str, object]:
    return {
        "schemaId": "agent.semantic-protocols.runtime-owner-identity-journal",
        "schemaVersion": "1",
        "workspaceIdentity": "workspace-a",
        "epoch": 7,
        "baseGenerationDigest": DIGEST_A,
        "sourceMutationId": "mutation-7",
        "sourceMutationDigest": DIGEST_B,
        "previousEpochReadable": True,
        "entries": [
            {
                "ownerPath": "src/a.rs",
                "state": "present",
                "contentDigest": DIGEST_A,
            },
            {"ownerPath": "src/deleted.rs", "state": "missing"},
            {
                "ownerPath": "src/editing.rs",
                "state": "mutating",
                "mutationId": "mutation-8",
            },
        ],
    }


def assert_invalid(packet: dict[str, object]) -> None:
    errors = list(jsonschema.Draft202012Validator(SCHEMA).iter_errors(packet))
    assert errors, packet


def test_runtime_owner_identity_journal_v1_accepts_complete_packet() -> None:
    jsonschema.Draft202012Validator(SCHEMA).validate(valid_packet())


def test_runtime_owner_identity_journal_v1_rejects_ambiguous_mutation_identity() -> None:
    missing_digest = valid_packet()
    del missing_digest["sourceMutationDigest"]
    assert_invalid(missing_digest)

    malformed_digest = valid_packet()
    malformed_digest["sourceMutationDigest"] = "generation-7"
    assert_invalid(malformed_digest)


def test_runtime_owner_identity_journal_v1_rejects_state_and_path_drift() -> None:
    tombstone_with_digest = copy.deepcopy(valid_packet())
    tombstone_with_digest["entries"][1]["contentDigest"] = DIGEST_B  # type: ignore[index]
    assert_invalid(tombstone_with_digest)

    path_escape = copy.deepcopy(valid_packet())
    path_escape["entries"][0]["ownerPath"] = "../outside.rs"  # type: ignore[index]
    assert_invalid(path_escape)

    mutating_without_identity = copy.deepcopy(valid_packet())
    del mutating_without_identity["entries"][2]["mutationId"]  # type: ignore[index]
    assert_invalid(mutating_without_identity)

    present_with_mutation = copy.deepcopy(valid_packet())
    present_with_mutation["entries"][0]["mutationId"] = "mutation-9"  # type: ignore[index]
    assert_invalid(present_with_mutation)
