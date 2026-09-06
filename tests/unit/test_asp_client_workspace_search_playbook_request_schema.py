import json
from pathlib import Path

import jsonschema
import pytest


ROOT = Path(__file__).resolve().parents[2]
SCHEMA = json.loads(
    (ROOT / "schemas/asp-client-workspace-search-playbook-request.v1.schema.json").read_text()
)


def request() -> dict:
    return {
        "schemaId": "agent.semantic-protocols.asp-client-workspace-search-playbook-request",
        "schemaVersion": "1",
    }


def test_workspace_playbook_request_v1_accepts_minimal_contract_query() -> None:
    jsonschema.Draft202012Validator(SCHEMA).validate(request())


def test_workspace_playbook_request_v1_accepts_pipe_ordered_producers() -> None:
    value = request()
    value["languages"] = "rust|python"
    value["documents"] = "org|md"
    jsonschema.Draft202012Validator(SCHEMA).validate(value)


def test_workspace_playbook_request_v1_rejects_empty_filter() -> None:
    value = request()
    value["languages"] = ""
    with pytest.raises(jsonschema.ValidationError):
        jsonschema.Draft202012Validator(SCHEMA).validate(value)


@pytest.mark.parametrize("legacy_field", ["intent", "query", "language", "next"])
def test_workspace_playbook_request_v1_rejects_reasoning_and_continuation_fields(
    legacy_field: str,
) -> None:
    value = request()
    value[legacy_field] = "must remain agent-owned"
    with pytest.raises(jsonschema.ValidationError):
        jsonschema.Draft202012Validator(SCHEMA).validate(value)
