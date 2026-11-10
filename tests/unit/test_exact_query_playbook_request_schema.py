# SPDX-FileCopyrightText: 2026 tao3k team and Contributors
#
# SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

import json
from pathlib import Path

import jsonschema


SCHEMA_PATH = (
    Path(__file__).parents[2] / "schemas" / "asp-client-exact-query-request.schema.json"
)


def validator() -> jsonschema.Draft202012Validator:
    return jsonschema.Draft202012Validator(json.loads(SCHEMA_PATH.read_text()))


def request(**overrides: object) -> dict[str, object]:
    value: dict[str, object] = {
        "schemaId": "agent.semantic-protocols.asp-client-exact-query-request",
        "schemaVersion": "1",
        "mode": "playbook",
        "selectors": [
            "rust://src/registry.rs#item/method/refresh/scope/implementation-owner/type/Registry",
            "org://docs/design.org#item/heading/Runtime-Authority",
        ],
        "projection": "source",
    }
    value.update(overrides)
    return value


def test_playbook_accepts_an_ordered_cross_producer_selector_batch() -> None:
    validator().validate(request())


def test_playbook_rejects_duplicate_selectors_and_second_producer_authority() -> None:
    duplicate = request(selectors=["rust://src/lib.rs#item/function/run"] * 2)
    assert list(validator().iter_errors(duplicate))

    second_authority = request(languages="rust")
    assert list(validator().iter_errors(second_authority))
