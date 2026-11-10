# SPDX-FileCopyrightText: 2026 tao3k team and Contributors
#
# SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

from __future__ import annotations

import json
from pathlib import Path

from jsonschema import Draft202012Validator
ROOT = Path(__file__).resolve().parents[2]
SCHEMAS = ROOT / "schemas"
def _load(path: Path) -> dict[str, object]:
    return json.loads(path.read_text(encoding="utf-8"))


def test_projection_schema_is_closed_and_versioned_exactly_one() -> None:
    schema = _load(SCHEMAS / "provider-method-argument-projection.v1.schema.json")
    Draft202012Validator.check_schema(schema)
    validator = Draft202012Validator(schema)
    valid = {
        "schemaVersion": "1",
        "tokens": [
            {"kind": "literal", "value": "search"},
            {"kind": "slot", "name": "query", "valueType": "string"},
        ],
    }
    validator.validate(valid)

    for invalid in [
        {**valid, "schemaVersion": "v1"},
        {**valid, "tokens": [{"kind": "slot", "name": "query", "valueType": "shell"}]},
        {**valid, "tokens": [{"kind": "format", "value": "{query}"}]},
        {**valid, "tokens": [{"kind": "literal", "value": "search", "eval": True}]},
    ]:
        assert list(validator.iter_errors(invalid)), invalid


def test_provider_registrations_use_canonical_identity() -> None:
    canonical = {
        "rust": "asp-rust",
        "typescript": "asp-typescript",
        "python": "asp-python",
        "julia": "asp-julia",
        "gerbil-scheme": "asp-gerbil-scheme",
    }
    registration_paths = {
        "rust": "languages/asp-rust/provider/asp-provider-registration.json",
        "typescript": "languages/asp-typescript/provider/asp-provider-registration.json",
        "python": "languages/asp-python/provider/asp-provider-registration.json",
        "julia": "languages/AspJulia.jl/juliac/asp-provider-registration.json",
        "gerbil-scheme": "languages/asp-gerbil-scheme/provider/asp-provider-registration.json",
    }
    registrations = {
        language: _load(ROOT / registration_paths[language])
        for language in canonical
    }

    for language, provider_id in canonical.items():
        registration = registrations[language]
        assert registration["providerId"] == provider_id
        assert registration["binary"] == provider_id
        assert registration["namespace"]
        assert registration["routes"]
        assert all(
            route["target"]["providerId"] == provider_id
            for route in registration["routes"]
        )
