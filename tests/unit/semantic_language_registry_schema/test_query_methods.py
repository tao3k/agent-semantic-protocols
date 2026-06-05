"""Query method registry schema tests."""

from .support import language_registry_errors, registry_with_descriptor


def test_query_method_can_declare_query_packet_schema() -> None:
    registry = registry_with_descriptor(
        {
            "method": "query/owner-items",
            "command": "query",
            "input": "owner-path",
            "requiredOptions": ["--term"],
            "outputSchemaIds": [
                "agent.semantic-protocols.semantic-query-packet"
            ],
            "packetSchemas": [
                "semantic-query-packet.v1",
                "semantic-tree-sitter-query.v1",
            ],
            "grammarId": "tree-sitter-rust",
            "grammarProfileVersion": "2026-06-04.v1",
            "grammarProfileSchema": "semantic-tree-sitter-grammar-profile.v1",
            "grammarProfilePath": "tree-sitter/tree-sitter-rust/grammar-profile.json",
            "cacheReplay": True,
            "supportsJson": True,
            "supportsCompact": True,
            "supportsQuerySet": True,
            "acceptedQuerySetSelectors": ["exact-set"],
            "querySetScopes": ["owner"],
            "outputModes": ["compact", "json", "code", "names"],
        },
        schemas=[
            {
                "schemaId": "agent.semantic-protocols.semantic-query-packet",
                "schemaVersion": "1",
                "path": "schemas/semantic-query-packet.v1.schema.json",
            },
        ],
    )

    assert language_registry_errors(registry) == []


def test_query_method_can_declare_provider_read_packet_schema() -> None:
    registry = registry_with_descriptor(
        {
            "method": "query/direct-source-read",
            "command": "query",
            "input": "owner-path",
            "requiredOptions": ["--from-hook", "--selector"],
            "outputSchemaIds": [
                "agent.semantic-protocols.semantic-query-packet",
                "agent.semantic-protocols.semantic-read-packet",
            ],
            "packetSchemas": [
                "semantic-query-packet.v1",
                "semantic-read-packet.v1",
                "semantic-tree-sitter-query.v1",
            ],
            "grammarId": "tree-sitter-rust",
            "grammarProfileVersion": "2026-06-04.v1",
            "grammarProfileSchema": "semantic-tree-sitter-grammar-profile.v1",
            "grammarProfilePath": "tree-sitter/tree-sitter-rust/grammar-profile.json",
            "queryInputForms": ["selector"],
            "cacheReplay": True,
            "supportsJson": True,
            "supportsCompact": True,
            "outputModes": ["compact", "json", "names", "read-packet"],
        },
        schemas=[
            {
                "schemaId": "agent.semantic-protocols.semantic-query-packet",
                "schemaVersion": "1",
                "path": "schemas/semantic-query-packet.v1.schema.json",
            },
            {
                "schemaId": "agent.semantic-protocols.semantic-read-packet",
                "schemaVersion": "1",
                "path": "schemas/semantic-read-packet.v1.schema.json",
            },
        ],
    )

    assert language_registry_errors(registry) == []


def test_query_method_can_declare_binary_embedded_catalog_delivery() -> None:
    registry = registry_with_descriptor(
        {
            "method": "query",
            "command": "query",
            "input": "catalog-id",
            "requiredOptions": ["--catalog"],
            "outputSchemaIds": [
                "agent.semantic-protocols.semantic-tree-sitter-query"
            ],
            "packetSchemas": ["semantic-tree-sitter-query.v1"],
            "grammarId": "tree-sitter-rust",
            "grammarProfileVersion": "2026-06-04.v1",
            "grammarProfileSchema": "semantic-tree-sitter-grammar-profile.v1",
            "grammarProfilePath": "tree-sitter/tree-sitter-rust/grammar-profile.json",
            "queryInputForms": ["catalog-id"],
            "queryCatalogs": [
                {
                    "id": "calls",
                    "path": "tree-sitter/tree-sitter-rust/queries/calls.scm",
                    "sourceDelivery": "provider-binary-embedded",
                    "captures": ["call.expression", "call.target"],
                }
            ],
            "supportsJson": True,
            "supportsCompact": True,
            "cacheReplay": True,
        },
        schemas=[
            {
                "schemaId": "agent.semantic-protocols.semantic-tree-sitter-query",
                "schemaVersion": "1",
                "path": "schemas/semantic-tree-sitter-query.v1.schema.json",
            },
        ],
    )

    assert language_registry_errors(registry) == []


def test_query_catalog_rejects_source_delivery_that_requires_package_source() -> None:
    registry = registry_with_descriptor(
        {
            "method": "query",
            "command": "query",
            "input": "catalog-id",
            "requiredOptions": ["--catalog"],
            "outputSchemaIds": [
                "agent.semantic-protocols.semantic-tree-sitter-query"
            ],
            "queryCatalogs": [
                {
                    "id": "calls",
                    "path": "tree-sitter/tree-sitter-rust/calls.scm",
                    "sourceDelivery": "provider-package-source",
                    "captures": ["call.expression"],
                }
            ],
            "supportsJson": True,
            "supportsCompact": True,
        }
    )

    assert any(
        "provider-package-source" in error
        and "provider-binary-embedded" in error
        for error in language_registry_errors(registry)
    )
