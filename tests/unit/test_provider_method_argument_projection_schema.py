from __future__ import annotations

import json
from pathlib import Path

from jsonschema import Draft202012Validator
from referencing import Registry, Resource


ROOT = Path(__file__).resolve().parents[2]
SCHEMAS = ROOT / "schemas"
REGISTRATIONS = SCHEMAS / "semantic-language-registrations"


def _load(path: Path) -> dict[str, object]:
    return json.loads(path.read_text(encoding="utf-8"))


def _schema_registry() -> Registry:
    resources = []
    for schema_path in SCHEMAS.glob("*.schema.json"):
        schema = _load(schema_path)
        if schema_id := schema.get("$id"):
            resources.append((schema_id, Resource.from_contents(schema)))
    return Registry().with_resources(resources)


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


def test_registry_validates_native_projection_capability_truth() -> None:
    registry_schema = _load(SCHEMAS / "semantic-language-registry.v1.schema.json")
    registry_validator = Draft202012Validator(
        registry_schema, registry=_schema_registry()
    )
    registrations = {
        language: _load(REGISTRATIONS / filename)
        for language, filename in {
            "rust": "rust.rs-harness.v1.json",
            "python": "python.py-harness.v1.json",
            "gerbil-scheme": "gerbil-scheme.gerbil-scheme-harness.v1.json",
        }.items()
    }
    for language in ("rust", "python"):
        lexical = next(
            descriptor
            for descriptor in registrations[language]["methodDescriptors"]
            if descriptor["method"] == "search/lexical"
        )
        language_registration = registrations[language]
        registry_validator.validate(
            {
                "registryId": "agent.semantic-protocols.semantic-language-registry",
                "registryVersion": "1",
                "protocolId": "agent.semantic-protocols.semantic-language",
                "protocolVersion": "1",
                "languages": [
                    {
                        key: value
                        for key, value in language_registration.items()
                        if key not in {"methods", "methodDescriptors"}
                    }
                    | {"methods": ["search/lexical"], "methodDescriptors": [lexical]}
                ],
            }
        )
        assert lexical["argumentProjection"]["schemaVersion"] == "1"
        assert not any(
            token.get("kind") == "slot" and token.get("name") == "owner"
            for token in lexical["argumentProjection"]["tokens"]
        )

    gerbil_lexical = next(
        descriptor
        for descriptor in registrations["gerbil-scheme"]["methodDescriptors"]
        if descriptor["method"] == "search/lexical"
    )
    assert "argumentProjection" not in gerbil_lexical
    gerbil_owner = next(
        descriptor
        for descriptor in registrations["gerbil-scheme"]["methodDescriptors"]
        if descriptor["method"] == "search/owner"
    )
    gerbil_registration = registrations["gerbil-scheme"]
    registry_validator.validate(
        {
            "registryId": "agent.semantic-protocols.semantic-language-registry",
            "registryVersion": "1",
            "protocolId": "agent.semantic-protocols.semantic-language",
            "protocolVersion": "1",
            "languages": [
                {
                    key: value
                    for key, value in gerbil_registration.items()
                    if key not in {"methods", "methodDescriptors"}
                }
                | {"methods": ["search/owner"], "methodDescriptors": [gerbil_owner]}
            ],
        }
    )
    assert any(
        token.get("kind") == "slot" and token.get("name") == "owner"
        for token in gerbil_owner["argumentProjection"]["tokens"]
    )
