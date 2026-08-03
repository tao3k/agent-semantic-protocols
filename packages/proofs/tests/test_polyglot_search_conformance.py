from __future__ import annotations

import copy
import json
from pathlib import Path

from asp_proofs.polyglot_search_conformance import (
    validate_progressive_turn,
    validate_relation_batch,
    validate_replacement_certificate,
    validate_search_document,
)


REPOSITORY_ROOT = Path(__file__).resolve().parents[3]
FIXTURE_ROOT = REPOSITORY_ROOT / "tests" / "fixtures" / "semantic_polyglot_search"
STATE_DIGEST = "sha256:" + "0" * 64
SNAPSHOT_DIGEST = "blake3-256:" + "a" * 64
PROVIDER_DIGEST = "blake3-256:" + "c" * 64
SEMANTIC_DIGEST = "blake3-256:" + "1" * 64
REGISTERED_PREDICATES = {
    "covers",
    "frontier_member",
    "gql_candidate",
}


VALID_DOCUMENT = """SEARCH
  SNAPSHOT @snapshot:42
  BUDGET NODES 10
  SELECT AT MOST 3

GQL
  MATCH (f:Callable)-[:OWNED_BY]->(o:Owner)
  WHERE f.name = 'open_runtime_project_proxy'
  RETURN f AS function, o AS candidate

LOGIC
  ?- gql_candidate(Function, Candidate),
     frontier_member(Candidate),
     covers(Candidate, Obligation).

TURBO
  USE evidence-frontier-v1

EMIT
  FRONTIER
"""


def load_fixture(name: str) -> dict[str, object]:
    return json.loads((FIXTURE_ROOT / name).read_text(encoding="utf-8"))


def violation_codes(receipt: object) -> set[str]:
    return {violation.code for violation in receipt.violations}


def test_valid_polyglot_document_is_admitted() -> None:
    receipt = validate_search_document(
        VALID_DOCUMENT,
        registered_predicates=REGISTERED_PREDICATES,
        state_digest=STATE_DIGEST,
    )

    assert receipt.admitted
    assert receipt.parsed_summary is not None
    assert receipt.parsed_summary["gql"]["clauses"] == ["match", "where", "return"]
    assert receipt.state_before_digest == receipt.state_after_digest


def test_gql_write_is_rejected_without_state_change() -> None:
    document = VALID_DOCUMENT.replace(
        "RETURN f AS function, o AS candidate",
        "CREATE (n:Injected) RETURN f AS function, o AS candidate",
    )
    receipt = validate_search_document(
        document,
        registered_predicates=REGISTERED_PREDICATES,
        state_digest=STATE_DIGEST,
    )

    assert not receipt.admitted
    assert "gql-write-or-procedure-not-admitted" in violation_codes(receipt)
    assert receipt.state_before_digest == receipt.state_after_digest


def test_duplicate_section_frame_is_rejected() -> None:
    document = VALID_DOCUMENT.replace("EMIT\n  FRONTIER", "GQL\n  MATCH (n) RETURN n\n\nEMIT\n  FRONTIER")
    receipt = validate_search_document(
        document,
        registered_predicates=REGISTERED_PREDICATES,
        state_digest=STATE_DIGEST,
    )

    assert not receipt.admitted
    assert "duplicate-section" in violation_codes(receipt)


def test_logic_rule_and_unregistered_predicate_are_rejected() -> None:
    document = VALID_DOCUMENT.replace(
        "?- gql_candidate(Function, Candidate),",
        "?- injected(X) :- gql_candidate(Function, Candidate),",
    )
    receipt = validate_search_document(
        document,
        registered_predicates=REGISTERED_PREDICATES,
        state_digest=STATE_DIGEST,
    )

    assert not receipt.admitted
    assert {"logic-rule-injection", "logic-predicate-unregistered"}.issubset(
        violation_codes(receipt)
    )


def test_relation_batch_rows_must_match_declared_columns() -> None:
    packet = load_fixture("relation-batch.valid.json")
    admitted = validate_relation_batch(
        packet,
        expected_snapshot_digest=SNAPSHOT_DIGEST,
        expected_provider_digest=PROVIDER_DIGEST,
        state_digest=STATE_DIGEST,
    )
    assert admitted.admitted

    mismatch = copy.deepcopy(packet)
    mismatch["relations"][0]["rows"][0].pop("path")
    rejected = validate_relation_batch(
        mismatch,
        expected_snapshot_digest=SNAPSHOT_DIGEST,
        expected_provider_digest=PROVIDER_DIGEST,
        state_digest=STATE_DIGEST,
    )
    assert not rejected.admitted
    assert "relation-row-schema-mismatch" in violation_codes(rejected)


def test_graph_turbo_relation_cannot_claim_evidence_authority() -> None:
    packet = load_fixture("relation-batch.valid.json")
    packet["relations"][0]["relationId"] = "asp.turbo_feature"
    packet["relations"][0]["authority"] = "derived"
    receipt = validate_relation_batch(
        packet,
        expected_snapshot_digest=SNAPSHOT_DIGEST,
        expected_provider_digest=PROVIDER_DIGEST,
        state_digest=STATE_DIGEST,
    )

    assert not receipt.admitted
    assert "turbo-feature-authority-violation" in violation_codes(receipt)


def test_progressive_turn_rejects_unexposed_selection() -> None:
    packet = load_fixture("turn.valid.json")
    packet["selectedCandidateIds"] = ["c1", "c11"]
    receipt = validate_progressive_turn(
        packet,
        expected_semantic_digest=SEMANTIC_DIGEST,
        state_digest=STATE_DIGEST,
    )

    assert not receipt.admitted
    assert "unexposed-candidate-selected" in violation_codes(receipt)


def test_progressive_turn_rejects_stale_continuation() -> None:
    packet = load_fixture("turn.valid.json")
    packet["continuation"]["semanticDigest"] = "blake3-256:" + "9" * 64
    receipt = validate_progressive_turn(
        packet,
        expected_semantic_digest=SEMANTIC_DIGEST,
        state_digest=STATE_DIGEST,
    )

    assert not receipt.admitted
    assert "stale-continuation" in violation_codes(receipt)


def test_replacement_certificate_binds_runtime_and_projection() -> None:
    packet = load_fixture("replacement.valid.json")
    packet["boundProjectionDigest"] = "sha256:" + "d" * 64
    admitted = validate_replacement_certificate(packet, state_digest=STATE_DIGEST)
    assert admitted.admitted

    drift = copy.deepcopy(packet)
    drift["runtime"]["liveDigest"] = "sha256:" + "e" * 64
    rejected = validate_replacement_certificate(drift, state_digest=STATE_DIGEST)
    assert not rejected.admitted
    assert "replacement-runtime-drift" in violation_codes(rejected)


def test_conformance_receipt_is_deterministic() -> None:
    first = validate_search_document(
        VALID_DOCUMENT,
        registered_predicates=REGISTERED_PREDICATES,
        state_digest=STATE_DIGEST,
    )
    second = validate_search_document(
        VALID_DOCUMENT,
        registered_predicates=REGISTERED_PREDICATES,
        state_digest=STATE_DIGEST,
    )

    assert first.to_dict() == second.to_dict()
