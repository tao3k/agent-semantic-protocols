"""Query backend and relation-flow registry schema tests."""

from .support import language_registry_errors, registry_with_descriptor


def test_query_method_can_declare_codeql_execution_backend_without_new_command_surface() -> None:
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
            "executionBackends": ["native-parser", "codeql"],
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


def test_query_method_can_declare_relation_flow_packet_schemas_without_codeql_backend() -> None:
    registry = registry_with_descriptor(
        {
            "method": "query/relation-flow",
            "command": "query",
            "input": "owner-path",
            "requiredOptions": ["--term"],
            "outputSchemaIds": [
                "agent.semantic-protocols.semantic-relation-plan",
                "agent.semantic-protocols.semantic-flow-lite",
            ],
            "packetSchemas": [
                "semantic-relation-plan.v1",
                "semantic-flow-lite.v1",
                "semantic-codeql-evidence.v1",
            ],
            "queryInputForms": ["selector"],
            "executionBackends": ["native-parser"],
            "supportsJson": True,
            "supportsCompact": True,
            "cacheReplay": True,
        },
        schemas=[
            {
                "schemaId": "agent.semantic-protocols.semantic-relation-plan",
                "schemaVersion": "1",
                "path": "schemas/semantic-relation-plan.v1.schema.json",
            },
            {
                "schemaId": "agent.semantic-protocols.semantic-flow-lite",
                "schemaVersion": "1",
                "path": "schemas/semantic-flow-lite.v1.schema.json",
            },
            {
                "schemaId": "agent.semantic-protocols.semantic-codeql-evidence",
                "schemaVersion": "1",
                "path": "schemas/semantic-codeql-evidence.v1.schema.json",
            },
        ],
    )

    assert language_registry_errors(registry) == []
