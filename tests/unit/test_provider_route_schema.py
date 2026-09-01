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


def test_all_seven_provider_registrations_have_canonical_inventory_and_root_identity() -> None:
    registrations = {
        "rust": ROOT / "languages/asp-rust/provider/asp-provider-registration.json",
        "typescript": ROOT / "languages/typescript-lang-project-harness/provider/asp-provider-registration.json",
        "python": ROOT / "languages/asp-python/provider/asp-provider-registration.json",
        "julia": ROOT / "languages/AspJulia.jl/juliac/asp-provider-registration.json",
        "gerbil-scheme": ROOT / "languages/gerbil-scheme-language-project-harness/provider/asp-provider-registration.json",
        "org": ROOT / "languages/orgize/provider/asp-provider-registration.json",
        "md": ROOT / "languages/orgize/provider/asp-md-provider-registration.json",
    }
    root_register = json.loads((ROOT / "schemas/provider-register.json").read_text())
    identities = {(entry["languageId"], entry["providerId"]) for entry in root_register["providers"]}
    for language_id, path in registrations.items():
        registration = json.loads(path.read_text())
        assert registration["languageId"] == language_id
        assert (language_id, registration["providerId"]) in identities
        assert registration["runtimeContract"]["transport"] == "http-json"
        assert registration["runtimeContract"]["clientBinding"] == "schema-driven"
        operations = {
            operation["operation"]: operation
            for operation in registration["runtimeContract"]["operations"]
        }
        # High-level search is owned by ASP Server; provider runtime exposes
        # only language-specific capabilities.  Route declarations are the
        # client/server boundary and must carry structured schema references.
        assert all(
            operation.get("requestSchema", {}).get("schemaId")
            and operation["requestSchema"].get("schemaVersion")
            and operation.get("responseSchema", {}).get("schemaId")
            and operation["responseSchema"].get("schemaVersion")
            for operation in operations.values()
        )
        # Provider runtime operations must not claim ASP Server's high-level
        # search authority; that route is validated separately in the shared
        # provider-route contract.
        assert "search" not in operations
        inventory = registration["sourceInventory"]
        assert inventory["configFiles"]
        assert inventory["sourceExtensions"]
        assert inventory["projectResolution"]["entryMarkers"]
        assert registration["routes"]
        descriptor_ref = registration["providerDescriptor"]["$ref"]
        assert (path.parent / descriptor_ref).is_file()
    assert registrations["org"].name != registrations["md"].name


def test_rust_registration_routes_use_structured_output_schema_references() -> None:
    registration = json.loads(
        (
            ROOT
            / "languages/asp-rust/provider/asp-provider-registration.json"
        ).read_text()
    )
    routes = registration["routes"]
    assert len(routes) == 4
    for route in routes:
        output = route["output"]
        assert set(output) <= {"schema", "mediaType", "projectionKind"}
        assert output["schema"]["schemaId"]
        assert output["schema"]["schemaVersion"] == "1"
        assert output["mediaType"] == "application/json"
        assert "schemaId" not in output
        assert "schemaVersion" not in output
        if "projectionKind" in output:
            assert output["projectionKind"]
