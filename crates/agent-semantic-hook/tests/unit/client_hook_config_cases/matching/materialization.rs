use super::{
    ClientHookConfig, DecisionKind, HookClassificationRequest, classify_hook_with_config, fs, json,
    load_client_config, temp_root, with_direct_dispatch_roles,
};

fn bound_read(mut payload: serde_json::Value) -> serde_json::Value {
    agent_semantic_hook::bind_plugin_host_matcher(&mut payload, Some("Read"), None)
        .expect("bind native Read matcher");
    payload
}

#[test]
fn builtin_materialization_rule_is_permanent_and_source_scoped() {
    let config = ClientHookConfig::default();
    let registry = crate::classifier::rust_registry();
    let source_payload = bound_read(json!({
        "session_id": "permanent-source-deny",
        "transcript_path": "/tmp/permanent-source-deny.jsonl",
        "tool_name": "Read",
        "tool_input": {
            "file_path": "crates/agent-semantic-hook/src/hook_config/core/implementation.rs"
        }
    }));

    for _ in 0..2 {
        let decision = classify_hook_with_config(HookClassificationRequest {
            registry: &registry,
            config: &config,
            platform: "codex",
            event: "pre-tool",
            payload: &source_payload,
        });
        assert_eq!(
            decision.decision,
            DecisionKind::Deny,
            "source materialization must remain denied: {decision:?}"
        );
        assert_eq!(
            decision
                .fields
                .get("configRuleId")
                .and_then(|id| id.as_str()),
            Some("route-read-to-asp-languages")
        );
        assert_eq!(decision.language_ids, ["rust"]);
        assert_eq!(decision.routes.len(), 1);
        assert_eq!(decision.routes[0].provider_id, "asp-rust");
        assert_eq!(
            &decision.routes[0].argv[..3],
            ["asp", "rust", "search"],
            "wrapped source reads must return an executable parser route"
        );
    }

    let non_source_decision = classify_hook_with_config(HookClassificationRequest {
        registry: &registry,
        config: &config,
        platform: "codex",
        event: "pre-tool",
        payload: &json!({
            "tool_name": "Bash",
            "tool_input": {"command": "sed -n 1p /dev/null"}
        }),
    });
    assert_eq!(non_source_decision.decision, DecisionKind::Allow);
}

#[test]
fn native_read_projects_registered_source_without_executable_name_tables() {
    let config = ClientHookConfig::default();
    let registry = crate::classifier::rust_registry();

    let native_read = classify_hook_with_config(HookClassificationRequest {
        registry: &registry,
        config: &config,
        platform: "codex",
        event: "pre-tool",
        payload: &bound_read(json!({
            "tool_name": "Read",
            "tool_input": {"file_path": "crates/agent-semantic-hook/src/tool_action.rs"}
        })),
    });
    assert_eq!(native_read.decision, DecisionKind::Deny);
    assert_eq!(
        native_read.fields["agentAction"]["hostInvocation"]["action"],
        "read"
    );
    assert_eq!(
        native_read.fields["agentAction"]["semanticCapabilities"][0]["evidence"],
        "host-matcher"
    );
    assert_eq!(
        native_read.fields["configRuleId"],
        "route-read-to-asp-languages"
    );
    assert!(
        native_read.message.contains(
            "Registered rust source reads are denied. Use the parser-owned ASP route below instead of raw Read."
        ),
        "{}",
        native_read.message
    );
    assert!(
        native_read
            .message
            .contains("asp rust search owner crates/agent-semantic-hook/src/tool_action.rs items"),
        "{}",
        native_read.message
    );
    assert_eq!(
        native_read.fields["normalizedActions"][0]["operationIntent"],
        "direct-read"
    );

    let non_source = classify_hook_with_config(HookClassificationRequest {
        registry: &registry,
        config: &config,
        platform: "codex",
        event: "pre-tool",
        payload: &json!({
            "tool_name": "Bash",
            "tool_input": {"command": "rtk read Cargo.lock"}
        }),
    });
    assert_eq!(non_source.decision, DecisionKind::Allow);
}

#[test]
fn canonical_read_matcher_accepts_supported_payload_field_variants() {
    let config = ClientHookConfig::default();
    let registry = crate::classifier::rust_registry();

    let cases = [
        (
            "codex-snake-case",
            json!({
                "tool_name": "Read",
                "tool_input": {
                    "file_path": "crates/agent-semantic-protocol/src/command/hook_runtime.rs"
                }
            }),
        ),
        (
            "arguments-envelope-glob",
            json!({
                "toolName": "Read",
                "arguments": {"path": "*.rs"}
            }),
        ),
    ];

    for (label, payload) in cases {
        let payload = bound_read(payload);
        let decision = classify_hook_with_config(HookClassificationRequest {
            registry: &registry,
            config: &config,
            platform: "codex",
            event: "pre-tool",
            payload: &payload,
        });

        assert_eq!(
            decision.decision,
            DecisionKind::Deny,
            "{label}: {decision:?}"
        );
        assert_eq!(
            decision.fields["configRuleId"], "route-read-to-asp-languages",
            "{label}: payload={payload}"
        );
        assert_eq!(
            decision.fields["agentAction"]["hostInvocation"]["action"], "read",
            "{label}: payload={payload}"
        );
        assert_eq!(
            decision.fields["agentAction"]["semanticCapabilities"][0]["action"], "read",
            "{label}: payload={payload}"
        );
        assert_eq!(
            decision.fields["normalizedActions"][0]["operationIntent"], "direct-read",
            "{label}: payload={payload}"
        );
        assert!(
            decision.fields["normalizedActions"][0]["paths"]
                .as_array()
                .is_some_and(|paths| !paths.is_empty()),
            "{label}: payload={payload}: {decision:?}"
        );
    }
}

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
