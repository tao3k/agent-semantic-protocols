import copy
import json
from pathlib import Path

import jsonschema
import pytest

from asp_proofs.query_playbook_materialization import validate_query_playbook_handoff
from asp_proofs.search_topology_settlement import SettlementError


ROOT = Path(__file__).resolve().parents[3]
REQUEST_SCHEMA = json.loads(
    (ROOT / "schemas/query-playbook-materialization-request.v1.schema.json").read_text()
)
REQUEST = ROOT / (
    "schemas/fixtures/query-playbook-materialization-request/"
    "valid-search-handoff.v1.json"
)
SETTLEMENT = ROOT / (
    "schemas/fixtures/search-topology-settlement/valid-derived-and-proposed.v1.json"
)


def load(path: Path) -> dict:
    return json.loads(path.read_text())


def test_query_playbook_handoff_fixture_is_schema_valid_and_semantically_admitted() -> (
    None
):
    request = load(REQUEST)
    settlement = load(SETTLEMENT)
    jsonschema.Draft202012Validator(REQUEST_SCHEMA).validate(request)
    validate_query_playbook_handoff(request, settlement)


def test_query_playbook_handoff_rejects_selector_subset() -> None:
    request = load(REQUEST)
    request["selectors"] = request["selectors"][:1]
    with pytest.raises(SettlementError) as caught:
        validate_query_playbook_handoff(request, load(SETTLEMENT))
    assert caught.value.reason_kind == "query-playbook-materialization-mismatch"


def test_query_playbook_handoff_rejects_cross_generation_replay() -> None:
    request = load(REQUEST)
    request["settlementBinding"]["topologyGenerationDigest"] = (
        "blake3-256:ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff"
    )
    with pytest.raises(SettlementError) as caught:
        validate_query_playbook_handoff(request, load(SETTLEMENT))
    assert caught.value.reason_kind == "query-playbook-binding-mismatch"


def test_query_playbook_handoff_rejects_changed_proof_dependencies() -> None:
    request = copy.deepcopy(load(REQUEST))
    request["proofDependencies"] = []
    with pytest.raises(SettlementError) as caught:
        validate_query_playbook_handoff(request, load(SETTLEMENT))
    assert caught.value.reason_kind == "query-playbook-materialization-mismatch"
