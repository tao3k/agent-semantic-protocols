from __future__ import annotations

import copy
import json
from pathlib import Path

from jsonschema import Draft202012Validator

from asp_proofs.polyglot_search_trace_fixture import (
    GQL_SOURCE,
    LOGIC_SOURCE,
    GraphNode,
    PropertyGraph,
    digest,
    execute_semantics,
    reference_graph,
    reference_relations,
    render_json_fixture,
    render_lean_fixture,
    trace_payload,
)


REPOSITORY_ROOT = Path(__file__).resolve().parents[3]
SCHEMA_PATH = REPOSITORY_ROOT / "schemas/semantic-polyglot-search-trace.v1.schema.json"
JSON_FIXTURE_PATH = (
    REPOSITORY_ROOT
    / "tests/fixtures/semantic_polyglot_search/semantic-trace.valid.json"
)
LEAN_FIXTURE_PATH = (
    REPOSITORY_ROOT
    / "packages/proofs/ASPProof/Generated/PolyglotSearchTraceFixture.lean"
)


def test_generated_json_is_schema_valid_and_byte_stable() -> None:
    generated = render_json_fixture()
    payload = json.loads(generated)
    schema = json.loads(SCHEMA_PATH.read_text(encoding="utf-8"))

    Draft202012Validator(schema).validate(payload)
    assert generated.encode() == JSON_FIXTURE_PATH.read_bytes()
    assert generated.endswith("\n")


def test_generated_lean_fixture_is_byte_stable() -> None:
    generated = render_lean_fixture()

    assert generated.encode() == LEAN_FIXTURE_PATH.read_bytes()
    assert generated.endswith("\n")


def test_generation_is_deterministic() -> None:
    assert render_json_fixture() == render_json_fixture()
    assert render_lean_fixture() == render_lean_fixture()


def test_payload_digest_recomputes_from_canonical_payload() -> None:
    payload = trace_payload()
    claimed = payload.pop("payloadDigest")

    assert digest(payload) == claimed


def test_semantic_row_or_digest_tampering_fails_identity_check() -> None:
    payload = trace_payload()
    tampered = copy.deepcopy(payload)
    tampered["gqlRows"][0]["candidateId"] = "4"
    claimed = tampered.pop("payloadDigest")

    assert digest(tampered) != claimed


def test_graph_drift_changes_bound_artifact_without_changing_query_source() -> None:
    graph = reference_graph()
    changed_graph = PropertyGraph(
        nodes=graph.nodes
        + (GraphNode("5", ("Module",), (("name", "unconnected"),)),),
        edges=graph.edges,
    )
    changed_trace = execute_semantics(
        gql_source=GQL_SOURCE,
        graph=changed_graph,
        logic_source=LOGIC_SOURCE,
        relations=reference_relations(),
    )
    baseline = trace_payload()
    changed = trace_payload(changed_trace)

    assert changed["gqlSourceDigest"] == baseline["gqlSourceDigest"]
    assert changed["graphDigestBefore"] != baseline["graphDigestBefore"]
    assert changed["payloadDigest"] != baseline["payloadDigest"]


def test_reference_trace_is_read_only_and_preserves_witness_bags() -> None:
    payload = trace_payload()

    assert payload["graphDigestBefore"] == payload["graphDigestAfter"]
    assert payload["gqlRows"][:2] == payload["gqlRows"][0:2]
    assert payload["gqlRows"][0] == payload["gqlRows"][1]
    assert payload["logicRows"][0] == payload["logicRows"][1]
    assert payload["multiplicity"] == "bag-by-witness-edge"
    assert payload["ordering"] == "canonical-row-json"
