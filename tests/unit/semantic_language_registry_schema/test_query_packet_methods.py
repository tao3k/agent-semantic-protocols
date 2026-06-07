"""Query packet method registry schema tests."""

import json
from pathlib import Path

from .support import language_registry_errors, registry_with_descriptor


CODE_LANGUAGE_IDS = {"rust", "typescript", "python", "julia"}


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
            "outputModes": ["frontier", "json", "code", "names"],
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
            "outputModes": ["frontier", "json", "names", "read-packet"],
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


def test_search_method_can_declare_semantic_fact_graph_packet_schema() -> None:
    registry = registry_with_descriptor(
        {
            "method": "search/semantic-facts",
            "command": "search",
            "outputSchemaIds": [
                "agent.semantic-protocols.semantic-fact-graph"
            ],
            "packetSchemas": [
                "semantic-fact-graph.v1",
            ],
            "view": "semantic-facts",
            "supportsJson": True,
            "supportsCompact": True,
            "requiresQuery": True,
            "acceptsStdin": True,
            "supportsPackageScope": True,
        },
        schemas=[
            {
                "schemaId": "agent.semantic-protocols.semantic-fact-graph",
                "schemaVersion": "1",
                "path": "schemas/semantic-fact-graph.v1.schema.json",
            },
        ],
    )

    assert language_registry_errors(registry) == []


def test_code_providers_expose_semantic_fact_graph_search_surface() -> None:
    registry_path = (
        Path(__file__).resolve().parents[3]
        / "schemas"
        / "semantic-language-registry.providers.v1.json"
    )
    registry = json.loads(registry_path.read_text())

    code_providers = [
        language
        for language in registry["languages"]
        if language["languageId"] in CODE_LANGUAGE_IDS
    ]

    assert {provider["languageId"] for provider in code_providers} == CODE_LANGUAGE_IDS
    for provider in code_providers:
        assert "search/semantic-facts" in provider["methods"]
        descriptor = next(
            method
            for method in provider["methodDescriptors"]
            if method["method"] == "search/semantic-facts"
        )
        assert descriptor["command"] == "search"
        assert descriptor["view"] == "semantic-facts"
        assert descriptor["packetSchemas"] == ["semantic-fact-graph.v1"]
        assert descriptor["outputSchemaIds"] == [
            "agent.semantic-protocols.semantic-fact-graph"
        ]
        assert descriptor["requiresQuery"] is True
        assert descriptor["acceptsStdin"] is True
        assert descriptor["supportsPackageScope"] is True
