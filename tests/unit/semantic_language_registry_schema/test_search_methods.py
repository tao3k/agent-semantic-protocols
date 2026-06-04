"""Search method registry schema tests."""

from .support import language_registry_errors, registry_with_descriptor


def test_search_method_can_declare_secondary_type_surface_schema() -> None:
    registry = registry_with_descriptor(
        {
            "method": "search/public-external-types",
            "command": "search",
            "view": "public-external-types",
            "outputSchemaIds": [
                "agent.semantic-protocols.semantic-search-packet",
                "agent.semantic-protocols.semantic-type-surface",
            ],
            "requiresQuery": True,
            "acceptsStdin": False,
            "supportsPackageScope": True,
            "supportsJson": True,
            "supportsCompact": True,
        },
        schemas=[
            {
                "schemaId": "agent.semantic-protocols.semantic-search-packet",
                "schemaVersion": "1",
                "path": "schemas/semantic-search-packet.v1.schema.json",
            },
            {
                "schemaId": "agent.semantic-protocols.semantic-type-surface",
                "schemaVersion": "1",
                "path": "schemas/semantic-type-surface.v1.schema.json",
            },
        ],
    )

    assert language_registry_errors(registry) == []
    language = registry["languages"][0]
    registered_schema_ids = {schema["schemaId"] for schema in language["schemas"]}
    declared_schema_ids = set(language["methodDescriptors"][0]["outputSchemaIds"])

    assert declared_schema_ids <= registered_schema_ids
