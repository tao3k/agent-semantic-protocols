"""Validate semantic AST patch receipt schema variants."""

from __future__ import annotations

from .support import minimal_ast_patch_receipt, schema_errors


def test_ast_patch_receipt_schema_accepts_codex_verifier_receipt() -> None:
    assert (
        schema_errors(
            "semantic-ast-patch-receipt.v1.schema.json",
            minimal_ast_patch_receipt(),
        )
        == []
    )


def test_ast_patch_receipt_schema_accepts_provider_ast_dry_run_receipt() -> None:
    receipt = minimal_ast_patch_receipt()
    receipt["mode"] = "dry-run"
    receipt["capability"] = "provider-ast-dry-run"
    receipt["operation"] = "remove_statement"
    receipt["mechanicalEditPlan"] = {
        "kind": "provider-dry-run",
        "operation": "remove_statement",
        "targetRead": "src/render.ts:10:43",
        "estimatedEdits": 2,
        "maxEdits": 20,
        "safeForLargeChange": True,
        "mutationAvailable": False,
        "requiresCodexApplyPatch": False,
        "changedRanges": ["src/render.ts:12:12", "src/render.ts:20:20"],
        "notes": ["provider AST dry-run resolved exact remove_statement nodes"],
    }

    assert schema_errors("semantic-ast-patch-receipt.v1.schema.json", receipt) == []


def test_ast_patch_receipt_schema_accepts_provider_unsupported_operation() -> None:
    receipt = minimal_ast_patch_receipt()
    receipt["status"] = "failed"
    receipt["mode"] = "dry-run"
    receipt["capability"] = "provider-ast-dry-run"
    receipt["operation"] = None
    receipt["supportedOperations"] = []
    receipt["mechanicalEditPlan"] = None
    receipt["verification"] = ["packet-parsed", "mutation-disabled"]
    receipt["failureKind"] = "unsupported-operation"
    receipt["failures"] = ["provider ast-patch dry-run does not support operation"]

    assert schema_errors("semantic-ast-patch-receipt.v1.schema.json", receipt) == []


def test_ast_patch_receipt_schema_rejects_codex_mutation_available() -> None:
    receipt = minimal_ast_patch_receipt()
    receipt["mutationAvailable"] = True

    messages = schema_errors("semantic-ast-patch-receipt.v1.schema.json", receipt)

    assert any("False was expected" in message for message in messages)


def test_ast_patch_receipt_schema_accepts_provider_ast_apply_receipt() -> None:
    receipt = minimal_ast_patch_receipt()
    receipt["status"] = "applied"
    receipt["mode"] = "apply"
    receipt["capability"] = "provider-ast-apply"
    receipt["mutationAvailable"] = True
    receipt["languageId"] = "rust"
    receipt["target"] = {
        "ownerPath": "src/lib.rs",
        "locator": "src/lib.rs#fn:demo",
        "read": "src/lib.rs:1:3",
    }
    receipt["operation"] = "replace_item"
    receipt["supportedOperations"] = ["replace_item"]
    receipt["verification"] = [
        "packet-parsed",
        "operation-supported",
        "target-read-valid",
        "snippet-parsed",
        "target-range-resolved",
        "target-item-parsed",
        "file-reparsed",
        "file-written",
        "rustfmt-ran",
        "formatter-output-reparsed",
    ]
    receipt["mechanicalEditPlan"] = {
        "kind": "provider-apply",
        "operation": "replace_item",
        "targetRead": "src/lib.rs:1:3",
        "estimatedEdits": 1,
        "maxEdits": 1,
        "safeForLargeChange": False,
        "mutationAvailable": True,
        "requiresCodexApplyPatch": False,
        "changedRanges": ["src/lib.rs:1:3"],
        "notes": ["Rust provider reparsed and formatted the replacement"],
    }
    receipt["next"] = "provider apply completed; check: asp rust check --changed ."

    assert schema_errors("semantic-ast-patch-receipt.v1.schema.json", receipt) == []


def test_ast_patch_receipt_schema_accepts_provider_native_owner_split() -> None:
    receipt = minimal_ast_patch_receipt()
    receipt["status"] = "applied"
    receipt["mode"] = "apply"
    receipt["capability"] = "provider-ast-apply"
    receipt["mutationAvailable"] = True
    receipt["mutationSource"] = "provider-native"
    receipt["snippetRequired"] = False
    receipt["codeInPrompt"] = False
    receipt["languageId"] = "rust"
    receipt["target"] = {
        "ownerPath": "src/lib.rs",
        "locator": "src/lib.rs#fn:moved",
        "read": "src/lib.rs:7:15",
    }
    receipt["operation"] = "split_owner_items"
    receipt["supportedOperations"] = ["replace_item", "split_owner_items"]
    receipt["verification"] = [
        "packet-parsed",
        "operation-supported",
        "target-read-valid",
        "provider-native-operation",
        "destination-validated",
        "target-items-parsed",
        "file-reparsed",
        "formatter-output-reparsed",
        "source-written",
    ]
    receipt["mechanicalEditPlan"] = {
        "kind": "provider-apply",
        "operation": "split_owner_items",
        "targetRead": "src/lib.rs:7:15",
        "estimatedEdits": 2,
        "maxEdits": 2,
        "safeForLargeChange": False,
        "mutationAvailable": True,
        "requiresCodexApplyPatch": False,
        "mutationSource": "provider-native",
        "snippetRequired": False,
        "codeInPrompt": False,
        "changedPaths": ["src/lib.rs", "src/split.rs"],
        "sourceBytesReadLocal": 240,
        "promptBytesAvoided": 140,
        "changedRanges": ["src/lib.rs:7:15", "src/split.rs:1:9"],
        "notes": ["provider moved parser-selected Rust items without source hunks"],
    }
    receipt["next"] = "provider apply completed; check: asp rust check --changed ."

    assert schema_errors("semantic-ast-patch-receipt.v1.schema.json", receipt) == []
