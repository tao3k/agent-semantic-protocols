# SPDX-FileCopyrightText: 2026 tao3k team and Contributors
#
# SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

from __future__ import annotations

from copy import deepcopy
from pathlib import Path

from jsonschema.exceptions import ValidationError
import pytest

from unit.schema_validation import schema_validator_for


ROOT = Path(__file__).resolve().parents[2]
SCHEMA = ROOT / "schemas/asp-client-workspace-query-playbook-request.v1.schema.json"


def request() -> dict:
    return {
        "schemaId": "agent.semantic-protocols.asp-client-workspace-query-playbook-request",
        "schemaVersion": "1",
        "language": "rust",
        "documents": "org",
        "selectors": [
            "org://docs/publication.org#item/heading/Publication",
            "rust://src/registry.rs#item/method/refresh/scope/implementation-owner/type/Registry",
        ],
        "projection": "source",
    }


def test_query_playbook_client_request_separates_code_and_document_producers() -> None:
    schema_validator_for(SCHEMA).validate(request())


@pytest.mark.parametrize(
    "forbidden_field",
    [
        "runtimeExecutionBinding",
        "projectWorkspaceIdentity",
        "worktreeInstanceId",
        "languages",
        "fromSearch",
        "recommendedNext",
    ],
)
def test_query_playbook_client_request_rejects_runtime_or_planner_authority(
    forbidden_field: str,
) -> None:
    packet = request()
    packet[forbidden_field] = "forbidden"
    with pytest.raises(ValidationError):
        schema_validator_for(SCHEMA).validate(packet)


def test_query_playbook_client_request_rejects_duplicate_selectors() -> None:
    packet = deepcopy(request())
    packet["selectors"].append(packet["selectors"][0])
    with pytest.raises(ValidationError):
        schema_validator_for(SCHEMA).validate(packet)
