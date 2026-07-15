use std::fs;

use agent_semantic_hook::{
    DecisionKind, HookClassificationRequest, asp_invocation_indices, classify_hook_with_config,
    load_client_config, semantic_shell_tokens,
};
use serde_json::json;

use crate::classifier::registry;

#[test]
fn asp_path_inspection_arguments_are_not_binary_invocations() {
    for command in [
        "command -v asp",
        "which asp",
        "type -a asp",
        "readlink asp",
        "shasum -a 256 asp",
        "command -v asp; readlink \"$(command -v asp)\"; shasum -a 256 \"$(command -v asp)\"",
    ] {
        let tokens = semantic_shell_tokens(command);
        assert!(
            asp_invocation_indices(&tokens).is_empty(),
            "{command}: {tokens:?}"
        );
    }
}

#[test]
fn asp_binary_invocations_remain_parser_owned_at_command_positions() {
    for command in [
        "asp rust guide",
        "printf ready; asp rust guide",
        "printf ready | asp rust guide",
        "FOO=1 asp rust guide",
        "command asp rust guide",
        "exec asp rust guide",
        "nohup asp rust guide",
    ] {
        let tokens = semantic_shell_tokens(command);
        assert_eq!(
            asp_invocation_indices(&tokens).len(),
            1,
            "{command}: {tokens:?}"
        );
    }
}

#[test]
fn asp_command_intent_policy_from_toml_changes_exact_evidence_matching() {
    let root = std::env::temp_dir().join(format!(
        "agent-semantic-hook-asp-command-intent-policy-{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system time")
            .as_nanos()
    ));
    fs::create_dir_all(&root).expect("create temp root");
    let config_path = root.join("config.toml");
    fs::write(
        &config_path,
        r#"
schemaId = "agent.semantic-protocols.hook.client-config"
schemaVersion = "1"
protocolId = "agent.semantic-protocols.hook"
protocolVersion = "1"

[aspCommandIntentPolicy.exactEvidence]
selectorKinds = ["symbol"]
"#,
    )
    .expect("write config");
    let config = load_client_config(&config_path).expect("load client config");
    let registry = registry();
    let decision = classify_hook_with_config(HookClassificationRequest {
        registry: &registry,
        config: &config,
        platform: "codex",
        event: "pre-tool",
        payload: &json!({
            "tool_name": "Bash",
            "tool_input": {
                "command": "asp typescript query --selector typescript://src/lib.ts#item/function/run --code"
            }
        }),
    });

    assert_eq!(decision.decision, DecisionKind::Deny);
    assert_eq!(
        decision
            .fields
            .get("aspCommandIntent")
            .and_then(|intent| intent.as_str()),
        Some("invalid-evidence"),
        "{decision:#?}"
    );
    let _ = fs::remove_dir_all(root);
}
