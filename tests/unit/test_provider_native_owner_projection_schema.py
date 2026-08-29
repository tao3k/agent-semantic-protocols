import json
from pathlib import Path

import jsonschema
from referencing import Registry, Resource


SCHEMA_ROOT = Path(__file__).resolve().parents[2] / "schemas"
OWNER_SCHEMA_PATH = SCHEMA_ROOT / "provider-native-owner-search-response.v1.schema.json"
CANONICAL_SELECTOR_SCHEMA_PATH = SCHEMA_ROOT / "canonical-item-selector.v1.schema.json"
EXACT_DEFINITIONS_SCHEMA_PATH = SCHEMA_ROOT / "exact-definitions.v1.schema.json"


def load_schema(path: Path) -> dict:
    return json.loads(path.read_text(encoding="utf-8"))


def validator() -> jsonschema.Draft202012Validator:
    owner_schema = load_schema(OWNER_SCHEMA_PATH)
    selector_schema = load_schema(CANONICAL_SELECTOR_SCHEMA_PATH)
    exact_definitions_schema = load_schema(EXACT_DEFINITIONS_SCHEMA_PATH)
    registry = Registry().with_resource(
        selector_schema["$id"], Resource.from_contents(selector_schema)
    ).with_resource(
        exact_definitions_schema["$id"], Resource.from_contents(exact_definitions_schema)
    )
    return jsonschema.Draft202012Validator(owner_schema, registry=registry)


def canonical_method_projection() -> dict:
    return {
        "canonicalItemSelector": {
            "schemaId": "asp.canonical-item-selector.v1",
            "schemaVersion": "1",
            "languageId": "rust",
            "kind": "method",
            "symbol": "render",
            "scopes": [
                {
                    "relation": "implementation-owner",
                    "kind": "type",
                    "symbol": "Widget",
                },
                {
                    "relation": "trait-owner",
                    "kind": "trait",
                    "symbol": "Render",
                },
            ],
            "structuralSelector": "rust://src/lib.rs#item/method/render/scope/implementation-owner/type/Widget/scope/trait-owner/trait/Render",
        },
        "signature": "fn render(&self)",
        "captureName": "declaration.name",
        "sourceByteStart": 0,
        "sourceByteEnd": 16,
    }


def response(projection: dict) -> dict:
    return {
        "schemaId": "agent.semantic-protocols.provider-native-owner-search-response",
        "schemaVersion": "1",
        "languageId": "rust",
        "providerId": "rs-harness",
        "requestedOwnerPath": "src/lib.rs",
        "requestedProjectionMode": "complete-owner",
        "sourceContentDigest": "a" * 64,
        "parsedOwnerCount": 1,
        "projectionCompleteness": "complete-owner",
        "projections": [projection],
    }


def test_nested_provider_projection_validates_as_one_typed_identity() -> None:
    validator().validate(response(canonical_method_projection()))


def test_projection_schema_has_no_duplicate_legacy_identity_fields() -> None:
    projection_schema = load_schema(OWNER_SCHEMA_PATH)["properties"]["projections"]["items"]
    assert set(projection_schema["required"]) == {
        "canonicalItemSelector",
        "signature",
        "captureName",
        "sourceByteStart",
        "sourceByteEnd",
    }
    for removed in ("structuralSelector", "itemKind", "itemName"):
        assert removed not in projection_schema["properties"]


def test_legacy_projection_packet_fails_closed() -> None:
    legacy = canonical_method_projection()
    selector = legacy.pop("canonicalItemSelector")
    legacy.update(
        structuralSelector=selector["structuralSelector"],
        itemKind=selector["kind"],
        itemName=selector["symbol"],
    )
    errors = list(validator().iter_errors(response(legacy)))
    assert errors
