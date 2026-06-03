#![allow(dead_code)]

#[path = "../../src/command/ast_patch.rs"]
mod ast_patch;

use serde_json::json;

#[test]
fn verifies_valid_packet_without_enabling_mutation() {
    let packet = json!({
        "schemaId": "agent.semantic-protocols.semantic-ast-patch",
        "schemaVersion": "1",
        "protocolId": "agent.semantic-protocols.ast-patch",
        "protocolVersion": "1",
        "languageId": "typescript",
        "target": {
            "ownerPath": "src/render.ts",
            "locator": "src/render.ts#fn:render",
            "read": "src/render.ts:10:20"
        },
        "operation": {
            "op": "append_to_block",
            "snippet": "lines.push(value);"
        }
    });
    let receipt = ast_patch::receipt_for_packet(&packet, ast_patch::AstPatchMode::DryRun);

    assert_eq!(receipt.status, "verified");
    assert!(!receipt.mutation_available);
    assert_eq!(receipt.operation.as_deref(), Some("append_to_block"));
    assert!(receipt.supported_operations.contains(&"remove_statement"));
    let plan = receipt.mechanical_edit_plan.as_ref().unwrap();
    assert_eq!(plan.kind, "codex-dry-run");
    assert_eq!(plan.operation, "append_to_block");
    assert_eq!(plan.target_read, "src/render.ts:10:20");
    assert!(!plan.safe_for_large_change);
    assert!(plan.requires_codex_apply_patch);
    assert!(receipt.failures.is_empty());
}

#[test]
fn rejects_missing_locator_fields() {
    let packet = json!({
        "schemaId": "agent.semantic-protocols.semantic-ast-patch",
        "schemaVersion": "1",
        "protocolId": "agent.semantic-protocols.ast-patch",
        "protocolVersion": "1",
        "target": {},
        "operation": {}
    });
    let receipt = ast_patch::receipt_for_packet(&packet, ast_patch::AstPatchMode::Verify);

    assert_eq!(receipt.status, "failed");
    assert_eq!(receipt.failure_kind, Some("invalid-packet"));
    assert!(
        receipt
            .failures
            .iter()
            .any(|failure| failure.contains("target.locator"))
    );
}

#[test]
fn dry_run_marks_bounded_mechanical_delete_as_large_change_safe() {
    let packet = json!({
        "schemaId": "agent.semantic-protocols.semantic-ast-patch",
        "schemaVersion": "1",
        "protocolId": "agent.semantic-protocols.ast-patch",
        "protocolVersion": "1",
        "languageId": "typescript",
        "target": {
            "ownerPath": "src/render.ts",
            "locator": "src/render.ts#fn:render#if:debug",
            "read": "src/render.ts:10:40"
        },
        "operation": {
            "op": "remove_statement",
            "expectedSnippet": "console.log(value);",
            "mechanicalKind": "bounded-multi-node",
            "maxEdits": 12,
            "allowLargeMechanicalEdit": true
        }
    });
    let receipt = ast_patch::receipt_for_packet(&packet, ast_patch::AstPatchMode::DryRun);
    assert_eq!(receipt.status, "verified");
    let plan = receipt.mechanical_edit_plan.as_ref().unwrap();
    assert!(plan.safe_for_large_change);
    assert_eq!(plan.max_edits, 12);
    assert_eq!(plan.estimated_edits, 12);
    assert!(!plan.mutation_available);
}

#[test]
fn rejects_unsupported_operation_without_text_fallback() {
    let packet = json!({
        "schemaId": "agent.semantic-protocols.semantic-ast-patch",
        "schemaVersion": "1",
        "protocolId": "agent.semantic-protocols.ast-patch",
        "protocolVersion": "1",
        "languageId": "typescript",
        "target": {
            "ownerPath": "src/render.ts",
            "locator": "src/render.ts#fn:render",
            "read": "src/render.ts:10:20"
        },
        "operation": {
            "op": "regex_replace_everything",
            "snippet": "changed"
        }
    });
    let receipt = ast_patch::receipt_for_packet(&packet, ast_patch::AstPatchMode::DryRun);
    assert_eq!(receipt.status, "failed");
    assert_eq!(receipt.failure_kind, Some("invalid-packet"));
    assert!(receipt.mechanical_edit_plan.is_none());
    assert!(
        receipt
            .failures
            .iter()
            .any(|failure| failure.contains("unsupported operation"))
    );
}
