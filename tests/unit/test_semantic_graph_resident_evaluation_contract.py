import json
from pathlib import Path


SCHEMAS = Path(__file__).parents[2] / "schemas"


def load(name: str) -> dict:
    return json.loads((SCHEMAS / name).read_text())


def test_request_is_intent_only() -> None:
    schema = load("semantic-graph-resident-evaluation-request.v1.schema.json")
    properties = schema["properties"]
    assert schema["additionalProperties"] is False
    assert properties["schemaId"]["const"].endswith("resident-evaluation-request")
    assert not ({"graph", "sourceSnapshot", "workspaceGeneration", "providerId", "algorithm"} & properties.keys())


def test_result_binds_ready_generation_and_forbids_query_time_side_effects() -> None:
    schema = load("semantic-graph-resident-evaluation-result.v1.schema.json")
    assert schema["properties"]["state"]["const"] == "Ready"
    counters = schema["$defs"]["workCounters"]["properties"]
    assert counters["providerRpcCount"]["const"] == 0
    assert counters["durableReadCount"]["const"] == 0
    assert counters["generationMutationCount"]["const"] == 0


def test_contract_is_new_v1_not_legacy_v2() -> None:
    request = load("semantic-graph-resident-evaluation-request.v1.schema.json")
    result = load("semantic-graph-resident-evaluation-result.v1.schema.json")
    assert request["properties"]["schemaVersion"]["const"] == "1"
    assert result["properties"]["schemaVersion"]["const"] == "1"
    assert request["properties"]["packetKind"]["const"] == "resident-graph-evaluation-request"
    assert result["properties"]["packetKind"]["const"] == "resident-graph-evaluation-result"
    assert request["properties"]["surface"]["enum"] == ["search-playbook", "query"]
    assert result["properties"]["surface"]["enum"] == ["search-playbook", "query"]
