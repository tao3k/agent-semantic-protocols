use super::{
    ClientHookConfig, DecisionKind, HookClassificationRequest, classify_hook_with_config, fs, json,
    load_client_config, temp_root, with_required_resident_agents,
};

#[test]
fn builtin_materialization_rule_is_permanent_and_source_scoped() {
    let config = ClientHookConfig::default();
    let registry = crate::classifier::rust_registry();
    let source_payload = json!({
        "session_id": "permanent-source-deny",
        "transcript_path": "/tmp/permanent-source-deny.jsonl",
        "tool_name": "Bash",
        "tool_input": {
            "command": "sed -n 1p crates/agent-semantic-hook/src/hook_config/core/implementation.rs"
        }
    });

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
            Some("deny-uncontrolled-source-materialization-commands")
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
fn action_first_rule_denies_inferred_reads_before_shell_expansion() {
    let config = ClientHookConfig::default();
    let registry = crate::classifier::rust_registry();

    for command in [
        "rtk read crates/agent-semantic-hook/src/tool_action.rs",
        "rtk read *.rs",
    ] {
        let decision = classify_hook_with_config(HookClassificationRequest {
            registry: &registry,
            config: &config,
            platform: "codex",
            event: "pre-tool",
            payload: &json!({
                "tool_name": "Bash",
                "tool_input": {"command": command}
            }),
        });
        assert_eq!(
            decision.decision,
            DecisionKind::Deny,
            "{command}: {decision:?}"
        );
        assert_eq!(
            decision
                .fields
                .get("configRuleId")
                .and_then(|id| id.as_str()),
            Some("deny-raw-registered-source-action"),
            "{command}"
        );
        let host_action = decision
            .fields
            .get("agentAction")
            .expect("typed agent action receipt");
        assert_eq!(host_action["action"], "execute", "{command}");
        assert_eq!(host_action["effect"], "read", "{command}");
        assert_eq!(host_action["authority"], "raw-shell", "{command}");
        assert!(
            matches!(
                host_action["subjects"][0]["kind"].as_str(),
                Some("registered-language-source" | "registered-language-source-pattern")
            ),
            "{command}: {host_action}"
        );
    }

    for command in ["unregistered-reader *.rs", "git restore source.rs"] {
        let decision = classify_hook_with_config(HookClassificationRequest {
            registry: &registry,
            config: &config,
            platform: "codex",
            event: "pre-tool",
            payload: &json!({
                "tool_name": "Bash",
                "tool_input": {"command": command}
            }),
        });
        assert_eq!(
            decision.decision,
            DecisionKind::Allow,
            "unprojected execute must not be treated as a semantic read: {command}: {decision:?}"
        );
    }

    let native_read = classify_hook_with_config(HookClassificationRequest {
        registry: &registry,
        config: &config,
        platform: "codex",
        event: "pre-tool",
        payload: &json!({
            "tool_name": "Read",
            "tool_input": {"file_path": "crates/agent-semantic-hook/src/tool_action.rs"}
        }),
    });
    assert_eq!(native_read.decision, DecisionKind::Deny);
    assert_eq!(native_read.fields["agentAction"]["action"], "read");
    assert_eq!(native_read.fields["agentAction"]["effect"], "read");
    assert_eq!(
        native_read.fields["agentAction"]["authority"],
        "raw-host-action"
    );
    assert_eq!(
        native_read.fields["configRuleId"],
        "materialize-registered-source-read-action"
    );
    assert_eq!(
        native_read.message,
        "Registered {{languageId}} source reads are denied. Use the parser-owned ASP route below instead of raw Read."
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
            "tool_input": {"command": "rtk read README.md"}
        }),
    });
    assert_eq!(non_source.decision, DecisionKind::Allow);
}

#[test]
fn registered_source_read_action_matches_real_payload_field_variants() {
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
            "desktop-camel-case",
            json!({
                "toolName": "functions.read_file",
                "toolInput": {"path": "hook_runtime.rs"}
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
            decision.fields["configRuleId"], "materialize-registered-source-read-action",
            "{label}: payload={payload}"
        );
        assert_eq!(
            decision.fields["agentAction"]["action"], "read",
            "{label}: payload={payload}"
        );
        assert_eq!(
            decision.fields["agentAction"]["effect"], "read",
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
fn later_denied_action_wins_over_earlier_allowed_envelope() {
    let root = temp_root("blocking-action-dominates-allow");
    let config_path = root.join("config.toml");
    fs::write(
        &config_path,
        with_required_resident_agents(
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

    assert_eq!(decision.decision, DecisionKind::Deny, "{decision:?}");
    assert_eq!(
        decision.fields["configRuleId"],
        "materialize-registered-source-read-action"
    );
    assert_eq!(
        decision.subject.tool_name.as_deref(),
        Some("functions.read_file")
    );
    assert_eq!(
        decision.fields["normalizedActions"]
            .as_array()
            .map(Vec::len),
        Some(2)
    );

    let _ = fs::remove_dir_all(root);
}
