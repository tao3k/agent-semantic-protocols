#![allow(dead_code)]

#[path = "../../src/command/ast_patch.rs"]
mod ast_patch;

use serde_json::json;

fn args(values: &[&str]) -> Vec<String> {
    values.iter().map(|value| (*value).to_string()).collect()
}

#[test]
fn template_command_builds_valid_packet_shape_from_exact_read_locator() {
    let packet = ast_patch::template_packet_for_args(&args(&[
        "--language",
        "typescript",
        "--owner",
        "src/render.ts",
        "--read",
        "src/render.ts:10:20",
        "--op",
        "replace_statement",
        "--snippet",
        "return value;",
        ".",
    ]))
    .expect("template packet");

    assert_eq!(
        packet["schemaId"],
        "agent.semantic-protocols.semantic-ast-patch"
    );
    assert_eq!(packet["languageId"], "typescript");
    assert_eq!(packet["providerId"], "ts-harness");
    assert_eq!(packet["binary"], "ts-harness");
    assert_eq!(packet["projectRoot"], ".");
    assert_eq!(packet["target"]["ownerPath"], "src/render.ts");
    assert_eq!(packet["target"]["locator"], "src/render.ts:10:20");
    assert_eq!(packet["target"]["read"], "src/render.ts:10:20");
    assert_eq!(packet["target"]["location"]["path"], "src/render.ts");
    assert_eq!(packet["target"]["location"]["lineRange"], "10:20");
    assert_eq!(packet["operation"]["op"], "replace_statement");
    assert_eq!(packet["operation"]["snippet"], "return value;");
    assert!(
        packet["verificationHints"][2]
            .as_str()
            .unwrap()
            .contains("asp typescript ast-patch dry-run --packet <semantic-ast-patch.json> .")
    );
}

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
    assert!(
        receipt
            .next
            .contains("asp typescript ast-patch dry-run --packet semantic-ast-patch.json .")
    );
    assert!(receipt
        .next
        .contains("asp typescript query --from-hook direct-source-read --selector src/render.ts:10:20 --code ."));
    assert!(receipt.next.contains("Codex apply_patch"));
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
            .next
            .contains("asp ast-patch template --language <language> --owner <owner-path>")
    );
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
