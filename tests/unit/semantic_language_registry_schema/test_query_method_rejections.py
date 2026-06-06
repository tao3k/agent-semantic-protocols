"""Query method registry rejection tests."""

from .support import language_registry_errors, registry_with_descriptor


def test_query_method_rejects_unknown_execution_backend() -> None:
    registry = registry_with_descriptor(
        {
            "method": "query",
            "command": "query",
            "input": "catalog-id",
            "requiredOptions": ["--catalog"],
            "outputSchemaIds": [
                "agent.semantic-protocols.semantic-tree-sitter-query"
            ],
            "queryInputForms": ["catalog-id"],
            "executionBackends": ["grep"],
            "supportsJson": True,
            "supportsCompact": True,
        }
    )

    assert any(
        "'grep' is not one of" in error
        for error in language_registry_errors(registry)
    )


def test_query_method_rejects_unknown_adapter_mode() -> None:
    registry = registry_with_descriptor(
        {
            "method": "query",
            "command": "query",
            "input": "catalog-id",
            "outputSchemaIds": [
                "agent.semantic-protocols.semantic-tree-sitter-query"
            ],
            "adapterModes": ["regex-projection"],
            "supportsJson": True,
            "supportsCompact": True,
        }
    )

    assert any(
        "'regex-projection' is not one of" in error
        for error in language_registry_errors(registry)
    )


def test_query_method_rejects_unknown_source_authority() -> None:
    registry = registry_with_descriptor(
        {
            "method": "query",
            "command": "query",
            "input": "catalog-id",
            "outputSchemaIds": [
                "agent.semantic-protocols.semantic-tree-sitter-query"
            ],
            "sourceAuthorities": ["raw-grep"],
            "supportsJson": True,
            "supportsCompact": True,
        }
    )

    assert any(
        "'raw-grep' is not one of" in error
        for error in language_registry_errors(registry)
    )


def test_query_method_rejects_unknown_render_profile() -> None:
    registry = registry_with_descriptor(
        {
            "method": "query",
            "command": "query",
            "input": "catalog-id",
            "outputSchemaIds": [
                "agent.semantic-protocols.semantic-tree-sitter-query"
            ],
            "renderProfiles": ["raw-json-dump"],
            "supportsJson": True,
            "supportsCompact": True,
        }
    )

    assert any(
        "'raw-json-dump' is not one of" in error
        for error in language_registry_errors(registry)
    )


def test_query_method_rejects_unknown_unsupported_pattern_behavior() -> None:
    registry = registry_with_descriptor(
        {
            "method": "query",
            "command": "query",
            "input": "catalog-id",
            "outputSchemaIds": [
                "agent.semantic-protocols.semantic-tree-sitter-query"
            ],
            "unsupportedPatternBehavior": "raw-search",
            "supportsJson": True,
            "supportsCompact": True,
        }
    )

    assert any(
        "'raw-search' is not one of" in error
        for error in language_registry_errors(registry)
    )


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
