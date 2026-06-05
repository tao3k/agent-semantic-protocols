"""Validate semantic AST patch request schema variants."""

from __future__ import annotations

from .support import minimal_ast_patch_request, schema_errors


def test_ast_patch_request_schema_accepts_compact_locator_packet() -> None:
    assert schema_errors("semantic-ast-patch.v1.schema.json", minimal_ast_patch_request()) == []


def test_ast_patch_request_schema_rejects_start_line_end_line_fields() -> None:
    packet = minimal_ast_patch_request()
    target = packet["target"]
    assert isinstance(target, dict)
    target["startLine"] = 10
    target["endLine"] = 43

    messages = schema_errors("semantic-ast-patch.v1.schema.json", packet)

    assert any("Additional properties are not allowed" in message for message in messages)


def test_ast_patch_request_schema_requires_source_locator_read() -> None:
    packet = minimal_ast_patch_request()
    target = packet["target"]
    assert isinstance(target, dict)
    target["read"] = "src/render.ts:10"

    messages = schema_errors("semantic-ast-patch.v1.schema.json", packet)

    assert any("does not match" in message for message in messages)


def test_ast_patch_request_schema_accepts_bounded_mechanical_delete() -> None:
    packet = minimal_ast_patch_request()
    packet["operation"] = {
        "op": "remove_statement",
        "expectedSnippet": "console.log(value);",
        "mechanicalKind": "bounded-multi-node",
        "maxEdits": 20,
        "allowLargeMechanicalEdit": True,
    }

    assert schema_errors("semantic-ast-patch.v1.schema.json", packet) == []


def test_ast_patch_request_schema_accepts_provider_native_owner_split() -> None:
    packet = minimal_ast_patch_request()
    packet["languageId"] = "rust"
    packet["providerId"] = "rs-harness"
    packet["binary"] = "rs-harness"
    packet["namespace"] = "agent.semantic-protocols.languages.rust.rs-harness"
    packet["target"] = {
        "ownerPath": "src/lib.rs",
        "locator": "src/lib.rs#fn:moved",
        "read": "src/lib.rs:7:15",
        "location": {"path": "src/lib.rs", "lineRange": "7:15"},
        "itemName": "moved",
        "itemKind": "fn",
    }
    packet["operation"] = {
        "op": "split_owner_items",
        "mutationSource": "provider-native",
        "snippetRequired": False,
        "codeInPrompt": False,
        "mechanicalKind": "owner-items",
        "maxEdits": 2,
        "fields": {
            "destinationPath": "src/split.rs",
            "moduleName": "split",
        },
    }

    assert schema_errors("semantic-ast-patch.v1.schema.json", packet) == []


def test_ast_patch_request_schema_rejects_unknown_operation() -> None:
    packet = minimal_ast_patch_request()
    packet["operation"] = {"op": "regex_replace_everything", "snippet": "changed"}

    messages = schema_errors("semantic-ast-patch.v1.schema.json", packet)

    assert any("is not one of" in message for message in messages)
