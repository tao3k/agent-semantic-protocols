use agent_semantic_hook::{DecisionKind, DecisionRouteKind, ReasonKind, classify_hook};
use serde_json::json;

use crate::classifier::registry;

#[test]
fn shell_apply_patch_to_source_requires_semantic_ast_patch() {
    let command = r#"apply_patch <<'PATCH'
*** Begin Patch
*** Update File: src/cli/agent-hooks.ts
@@
-const before = true;
+const after = true;
*** End Patch
PATCH"#;
    let decision = classify_hook(
        &registry(),
        "codex",
        "pre-tool",
        &json!({ "tool_name": "functions.exec_command", "tool_input": { "cmd": command } }),
    );

    assert_eq!(decision.decision, DecisionKind::Deny);
    assert_eq!(decision.reason_kind, ReasonKind::SemanticAstPatchRequired);
    assert_eq!(
        decision
            .fields
            .get("configRuleId")
            .and_then(|value| value.as_str()),
        Some("materialize-apply-patch-policy")
    );
    assert_eq!(decision.subject.paths, ["src/cli/agent-hooks.ts"]);
    assert_eq!(decision.routes[0].kind, DecisionRouteKind::Owner);
    assert_eq!(decision.routes[0].provider_id, "ts-harness");
    assert!(decision.message.contains("Locator route:"));
    assert!(decision.message.contains("path-only locator output"));
    assert!(
        decision
            .message
            .contains("query --selector <path:start:end> --workspace")
    );
    assert!(!decision.message.contains("--from-hook direct-source-read"));
    assert!(decision.message.contains("semantic-ast-patch.json"));
    assert!(decision.message.contains("handwritten source hunks"));
    assert!(decision.message.contains("provider-native"));
    assert!(decision.message.contains("ast-patch apply"));
    assert!(decision.message.contains("codex-text-fallback"));
    assert!(
        !decision
            .message
            .contains("only then retry Codex apply_patch")
    );

    let root = std::env::temp_dir().join(format!(
        "agent-semantic-hook-ast-patch-disabled-{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("clock")
            .as_nanos()
    ));
    std::fs::create_dir_all(&root).expect("create temp config root");
    let config_path = root.join("config.toml");
    std::fs::write(
        &config_path,
        r#"schemaId = "agent.semantic-protocols.hook.client-config"
schemaVersion = "1"
protocolId = "agent.semantic-protocols.hook"
protocolVersion = "1"

[experimental.semanticAstPatch]
enabled = false
"#,
    )
    .expect("write disabled ast patch config");
    let config = agent_semantic_hook::load_client_config(&config_path)
        .expect("load disabled ast patch config");
    let disabled_payload =
        json!({ "tool_name": "functions.exec_command", "tool_input": { "cmd": command } });
    let disabled_decision = agent_semantic_hook::classify_hook_with_config(
        agent_semantic_hook::HookClassificationRequest {
            registry: &registry(),
            config: &config,
            platform: "codex",
            event: "pre-tool",
            payload: &disabled_payload,
        },
    );
    assert_eq!(disabled_decision.decision, DecisionKind::Allow);
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn direct_apply_patch_tool_to_source_requires_semantic_ast_patch() {
    let patch = r#"*** Begin Patch
*** Update File: src/cli/agent-hooks.ts
@@
-const before = true;
+const after = true;
*** End Patch
"#;
    let decision = classify_hook(
        &registry(),
        "codex",
        "pre-tool",
        &json!({ "tool_name": "functions.apply_patch", "tool_input": { "patch": patch } }),
    );

    assert_eq!(decision.decision, DecisionKind::Deny);
    assert_eq!(decision.reason_kind, ReasonKind::SemanticAstPatchRequired);
    assert_eq!(
        decision
            .fields
            .get("configRuleId")
            .and_then(|value| value.as_str()),
        Some("materialize-apply-patch-policy")
    );
    assert_eq!(decision.subject.paths, ["src/cli/agent-hooks.ts"]);
    assert!(decision.message.contains("Locator route:"));
    assert!(decision.message.contains("path-only locator output"));
    assert!(
        decision
            .message
            .contains("query --selector <path:start:end> --workspace")
    );
    assert!(!decision.message.contains("--from-hook direct-source-read"));
    assert!(decision.message.contains("semantic-ast-patch.json"));
    assert!(decision.message.contains("handwritten source hunks"));
    assert!(decision.message.contains("provider-native"));
    assert!(decision.message.contains("ast-patch apply"));
    assert!(decision.message.contains("controlled maintenance policy"));
    assert!(
        !decision
            .message
            .contains("only then retry Codex apply_patch")
    );
}

#[test]
fn apply_patch_to_non_source_file_is_allowed() {
    let command = r#"apply_patch <<'PATCH'
*** Begin Patch
*** Update File: README.md
@@
-old
+new
*** End Patch
PATCH"#;
    let decision = classify_hook(
        &registry(),
        "codex",
        "pre-tool",
        &json!({
            "tool_name": "functions.exec_command",
            "tool_input": {"cmd": command}
        }),
    );
    assert_eq!(decision.decision, DecisionKind::Allow);
    assert_eq!(decision.subject.paths, ["README.md"]);
}

#[test]
fn structured_edit_tool_to_source_requires_semantic_ast_patch() {
    for (tool_name, tool_input, expected_path) in [
        (
            "Edit",
            json!({
                "file_path": "src/cli/agent-hooks.ts",
                "old_string": "old",
                "new_string": "new"
            }),
            "src/cli/agent-hooks.ts",
        ),
        (
            "MultiEdit",
            json!({
                "file_path": "src/cli/agent-hooks.ts",
                "edits": [{"old_string": "old", "new_string": "new"}]
            }),
            "src/cli/agent-hooks.ts",
        ),
        (
            "Write",
            json!({
                "file_path": "src/cli/agent-hooks.ts",
                "content": "export function generated() {\n  return 1;\n}\n"
            }),
            "src/cli/agent-hooks.ts",
        ),
        (
            "FsWriteFile",
            json!({
                "path": "src/cli/agent-hooks.ts",
                "content": "export function generated() {\n  return 1;\n}\n"
            }),
            "src/cli/agent-hooks.ts",
        ),
    ] {
        let decision = classify_hook(
            &registry(),
            "codex",
            "pre-tool",
            &json!({
                "tool_name": tool_name,
                "tool_input": tool_input
            }),
        );
        assert_eq!(decision.decision, DecisionKind::Deny, "{tool_name}");
        assert_eq!(
            decision.reason_kind,
            ReasonKind::SemanticAstPatchRequired,
            "{tool_name}"
        );
        assert_eq!(decision.subject.paths, [expected_path.to_string()]);
        assert!(decision.message.contains("asp ast-patch template"));
    }
}

#[test]
fn structured_write_to_new_source_path_requires_semantic_ast_patch() {
    for (tool_name, tool_input, expected_path) in [
        (
            "Write",
            json!({
                "file_path": "src/cli/generated.ts",
                "content": "export function generated() {\n  return 1;\n}\n",
            }),
            "src/cli/generated.ts",
        ),
        (
            "FsWriteFile",
            json!({
                "path": "src/cli/generated.ts",
                "content": "export function generated() {\n  return 1;\n}\n",
            }),
            "src/cli/generated.ts",
        ),
    ] {
        let payload = json!({ "tool_name": tool_name, "tool_input": tool_input });
        let decision = classify_hook(&registry(), "codex", "pre-tool", &payload);
        assert_eq!(decision.decision, DecisionKind::Deny, "{tool_name}");
        assert_eq!(
            decision.reason_kind,
            ReasonKind::SemanticAstPatchRequired,
            "{tool_name}"
        );
        assert_eq!(decision.subject.paths, [expected_path.to_string()]);
        assert!(decision.message.contains("asp ast-patch template"));
    }
}

#[test]
fn nested_parallel_edit_tool_to_source_requires_semantic_ast_patch() {
    let decision = classify_hook(
        &registry(),
        "codex",
        "pre-tool",
        &json!({
            "tool_name": "multi_tool_use.parallel",
            "tool_input": {
                "tool_uses": [{
                    "recipient_name": "functions.edit",
                    "parameters": {
                        "file_path": "src/cli/agent-hooks.ts",
                        "old_string": "old",
                        "new_string": "new"
                    }
                }]
            }
        }),
    );
    assert_eq!(decision.decision, DecisionKind::Deny);
    assert_eq!(decision.reason_kind, ReasonKind::SemanticAstPatchRequired);
    assert_eq!(
        decision.subject.tool_name.as_deref(),
        Some("functions.edit")
    );
    assert_eq!(decision.subject.paths, ["src/cli/agent-hooks.ts"]);
}

#[test]
fn authorized_direct_apply_patch_to_source_is_allowed_by_patch_digest() {
    let patch = r#"*** Begin Patch
*** Update File: src/cli/agent-hooks.ts
@@
+export const marker = true;
*** End Patch
"#;
    let patch_digest = format!(
        "{:x}",
        <sha2::Sha256 as sha2::Digest>::digest(patch.as_bytes())
    );
    let root = std::env::temp_dir().join(format!(
        "asp-hook-source-apply-patch-{}-{patch_digest}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&root);
    let authorization_dir = root
        .join(".cache")
        .join("agent-semantic-protocol")
        .join("hooks")
        .join("source-apply-patch");
    std::fs::create_dir_all(&authorization_dir).expect("create authorization dir");
    std::fs::write(authorization_dir.join(format!("{patch_digest}.json")), "{}")
        .expect("write authorization file");

    let mut registry = registry();
    registry.project_root = root.to_string_lossy().into_owned();
    let decision = classify_hook(
        &registry,
        "codex",
        "pre-tool",
        &json!({ "tool_name": "functions.apply_patch", "tool_input": { "patch": patch } }),
    );
    std::fs::remove_dir_all(&root).expect("remove authorization root");

    assert_eq!(decision.decision, DecisionKind::Allow);
    assert_eq!(decision.reason_kind, ReasonKind::None);
    assert_eq!(decision.language_ids, ["typescript"]);
    assert_eq!(decision.subject.paths, ["src/cli/agent-hooks.ts"]);
    assert_eq!(
        decision
            .fields
            .get("maintenancePolicy")
            .and_then(|value| value.as_str()),
        Some("source-apply-patch-authorization")
    );
    assert_eq!(
        decision
            .fields
            .get("patchDigest")
            .and_then(|value| value.as_str()),
        Some(patch_digest.as_str())
    );
    assert!(
        decision
            .message
            .contains("controlled maintenance authorization")
    );
}
