"""Tree-sitter query method registry schema tests."""

from .support import language_registry_errors, registry_with_descriptor


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
            "adapterModes": ["native-projection"],
            "sourceAuthorities": ["native-parser-adapter", "native-parser"],
            "executionBackends": ["native-parser"],
            "renderProfiles": ["corpus-locator"],
            "unsupportedPatternBehavior": "diagnostic",
            "supportedPredicates": ["#eq?", "#any-eq?", "#match?", "#any-match?"],
            "unsupportedPredicates": ["#not-eq?", "#not-match?"],
            "queryCatalogs": [
                {
                    "id": "calls",
                    "path": "tree-sitter/tree-sitter-rust/queries/calls.scm",
                    "sourceDelivery": "provider-binary-embedded",
                    "captures": ["call.expression", "call.target"],
                    "nodeTypes": ["call_expression"],
                    "fields": ["function"],
                    "fingerprint": (
                        "sha256:"
                        "0123456789abcdef0123456789abcdef"
                        "0123456789abcdef0123456789abcdef"
                    ),
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


def test_query_method_can_declare_full_tree_sitter_conformance_fields() -> None:
    registry = registry_with_descriptor(
        {
            "method": "query",
            "command": "query",
            "input": "s-expression",
            "requiredOptions": ["--treesitter-query"],
            "outputSchemaIds": [
                "agent.semantic-protocols.semantic-tree-sitter-query"
            ],
            "packetSchemas": ["semantic-tree-sitter-query.v1"],
            "grammarId": "tree-sitter-typescript",
            "grammarProfileVersion": "2026-06-05.v1",
            "grammarProfileSchema": "semantic-tree-sitter-grammar-profile.v1",
            "grammarProfilePath": (
                "tree-sitter/tree-sitter-typescript/grammar-profile.json"
            ),
            "queryInputForms": [
                "selector",
                "code-shaped",
                "catalog-id",
                "s-expression",
            ],
            "adapterModes": ["native-projection", "cached-replay"],
            "sourceAuthorities": [
                "native-parser-adapter",
                "native-parser",
                "cached-provider-export",
            ],
            "executionBackends": ["native-parser", "cached-replay"],
            "renderProfiles": [
                "compact-graph-frontier",
                "corpus-locator",
                "flow-lite-frontier",
            ],
            "codeOutput": {
                "mode": "pure-code",
                "multiMatch": "deny",
                "requires": ["exact-selector", "unique-predicate"],
            },
            "unsupportedPatternBehavior": "empty-frontier",
            "queryCatalogs": [
                {
                    "id": "typescript.declarations",
                    "path": (
                        "tree-sitter/tree-sitter-typescript/queries/"
                        "declarations.scm"
                    ),
                    "sourceDelivery": "provider-binary-embedded",
                    "captures": [
                        "function.name",
                        "class.name",
                        "interface.name",
                    ],
                    "nodeTypes": [
                        "function_declaration",
                        "class_declaration",
                        "interface_declaration",
                    ],
                    "fields": ["name"],
                    "fingerprint": (
                        "sha256:"
                        "abcdef0123456789abcdef0123456789"
                        "abcdef0123456789abcdef0123456789"
                    ),
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
