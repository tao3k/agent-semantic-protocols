"""AST patch method registry schema tests."""

from .support import language_registry_errors, registry_with_descriptor


AST_PATCH_RECEIPT_SCHEMA = {
    "schemaId": "agent.semantic-protocols.semantic-ast-patch-receipt",
    "schemaVersion": "1",
    "path": "schemas/semantic-ast-patch-receipt.v1.schema.json",
}


def test_ast_patch_dry_run_method_declares_non_mutating_receipt() -> None:
    registry = registry_with_descriptor(
        {
            "method": "ast-patch/dry-run",
            "command": "ast-patch",
            "input": "semantic-ast-patch packet",
            "requiredOptions": ["--packet"],
            "outputSchemaIds": [
                "agent.semantic-protocols.semantic-ast-patch-receipt"
            ],
            "supportsJson": True,
            "supportsCompact": False,
            "mutationAvailable": False,
        },
        schemas=[AST_PATCH_RECEIPT_SCHEMA],
    )

    assert language_registry_errors(registry) == []


def test_ast_patch_apply_method_can_declare_mutating_receipt() -> None:
    registry = registry_with_descriptor(
        {
            "method": "ast-patch/apply",
            "command": "ast-patch",
            "input": "semantic-ast-patch packet",
            "requiredOptions": ["--packet"],
            "outputSchemaIds": [
                "agent.semantic-protocols.semantic-ast-patch-receipt"
            ],
            "supportsJson": True,
            "supportsCompact": False,
            "mutationAvailable": True,
        },
        schemas=[AST_PATCH_RECEIPT_SCHEMA],
    )

    assert language_registry_errors(registry) == []
