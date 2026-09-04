use super::DecisionKind;
use super::HookClassificationRequest;
use super::classify_hook_with_config;
use super::fs;
use super::json;
use super::load_client_config;
use super::temp_root;
use super::with_direct_dispatch_roles;

#[test]
fn nested_payload_shape_does_not_invent_a_native_read_matcher() {
    let root = temp_root("blocking-action-dominates-allow");
    let config_path = root.join("config.toml");
    fs::write(
        &config_path,
        with_direct_dispatch_roles(
            r#"
schemaId = "agent.semantic-protocols.hook.client-config"
schemaVersion = "1"
protocolId = "agent.semantic-protocols.hook"
protocolVersion = "1"

[[rules]]
id = "allow-parallel-envelope"
priority = 50000
decision = "allow"
message = "The outer transport envelope is allowed."

[rules.match]
toolAny = ["multi_tool_use.parallel"]
"#,
        ),
    )
    .expect("write config");
    let config = load_client_config(&config_path).expect("load client config");
    let registry = crate::classifier::rust_registry();
    let payload = json!({
        "tool_name": "multi_tool_use.parallel",
        "tool_input": {
            "tool_uses": [
                {
                    "recipient_name": "functions.read_file",
                    "parameters": {"path": "hook_runtime.rs"}
                }
            ]
        }
    });

    let decision = classify_hook_with_config(HookClassificationRequest {
        registry: &registry,
        config: &config,
        platform: "codex",
        event: "pre-tool",
        payload: &payload,
    });

    assert_eq!(decision.decision, DecisionKind::Allow, "{decision:?}");
    assert_eq!(decision.fields["configRuleId"], "allow-parallel-envelope");
    assert_eq!(
        decision.fields["normalizedActions"]
            .as_array()
            .map(Vec::len),
        Some(2)
    );

    let _ = fs::remove_dir_all(root);
}
