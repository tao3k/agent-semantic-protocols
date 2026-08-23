import json
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]


def load_schema(name: str) -> dict:
    return json.loads((ROOT / "schemas" / name).read_text())


def walk(value):
    yield value
    if isinstance(value, dict):
        for child in value.values():
            yield from walk(child)
    elif isinstance(value, list):
        for child in value:
            yield from walk(child)


def test_provider_route_owns_semantics_without_process_invocation() -> None:
    schema = load_schema("provider-route.schema.json")
    assert schema["properties"]["schemaId"]["const"] == (
        "agent.semantic-protocols.provider-route"
    )
    assert schema["properties"]["schemaVersion"]["const"] == "1"
    assert schema["properties"]["authority"]["const"] == "asp-server"

    forbidden = {
        "argv",
        "argumentProjection",
        "command",
        "executable",
        "invocation",
        "methodDescriptor",
        "stdinMode",
        "transport",
    }
    keys = {
        key
        for node in walk(schema)
        if isinstance(node, dict)
        for key in node
    }
    assert keys.isdisjoint(forbidden)


def test_provider_registration_requires_route_dsl_and_has_no_method_legacy() -> None:
    schema = load_schema("provider-registration.schema.json")
    assert "routes" in schema["required"]
    assert "sourceInventory" in schema["required"]
    assert "searchCapabilities" in schema["required"]
    assert schema["properties"]["routes"]["items"] == {
        "$ref": "provider-route.schema.json"
    }

    serialized = json.dumps(schema, sort_keys=True)
    for legacy in (
        "argumentProjection",
        "argumentToken",
        "invocation",
        "methodDescriptor",
        "methodDescriptors",
        '"methods"',
        "manifestDigest",
        "runtimeProfile",
    ):
        assert legacy not in serialized
