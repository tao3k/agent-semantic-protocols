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
        "language": None,
        "intent": "relationship",
        "query": "runtime transport authority",
        "scope": "workspace",
        "coverage": "candidates",
        "maxOwners": 32,
        "deadlineMs": 1000,
        "explain": "full",
    }


def test_workspace_playbook_request_v1_accepts_no_language_filter() -> None:
    jsonschema.Draft202012Validator(SCHEMA).validate(request())


def test_workspace_playbook_request_v1_accepts_one_explicit_language_filter() -> None:
    value = request()
    value["language"] = "rust"
    jsonschema.Draft202012Validator(SCHEMA).validate(value)


def test_workspace_playbook_request_v1_rejects_empty_filter() -> None:
    value = request()
    value["language"] = ""
    with pytest.raises(jsonschema.ValidationError):
        jsonschema.Draft202012Validator(SCHEMA).validate(value)
