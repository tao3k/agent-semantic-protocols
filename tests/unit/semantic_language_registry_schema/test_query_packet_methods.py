"""Query packet method registry schema tests."""

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
