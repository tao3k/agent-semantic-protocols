use super::common::{
    ClientHookConfig, DecisionKind, HookClassificationRequest, classify_hook_with_config, fs, json,
    load_client_config, registry, temp_root,
};

#[test]
fn argv_source_glob_rule_matches_source_argument_after_flags() {
    let root = temp_root("argv-source-glob");
    let config_path = root.join("config.toml");
    fs::write(
        &config_path,
        r#"
schemaId = "agent.semantic-protocols.hook.client-config"
schemaVersion = "1"
protocolId = "agent.semantic-protocols.hook"
protocolVersion = "1"

[[rules]]
id = "deny-wl-source-argv"
decision = "deny"
message = "matched configured argv source"

[rules.match]
tool = "Bash"
commandAny = ["wl"]
argvSourceGlobAny = ["*.ts"]
argvSourceExcludeFlagAny = ["--output"]
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
            "tool_input": {"command": "wl --flag2 flag3 *.ts"}
        }),
    });

    assert_eq!(decision.decision, DecisionKind::Deny);
    assert_eq!(
        decision
            .fields
            .get("configRuleId")
            .and_then(|id| id.as_str()),
        Some("deny-wl-source-argv")
    );

    for command in [
        "wl --flag2 flag3 README",
        "wl --output *.ts README",
        "wl --output=*.ts README",
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

        assert_eq!(decision.decision, DecisionKind::Allow, "{command}");
    }

    let decision = classify_hook_with_config(HookClassificationRequest {
        registry: &registry,
        config: &config,
        platform: "codex",
        event: "pre-tool",
        payload: &json!({
            "tool_name": "Bash",
            "tool_input": {"command": "wl --output ignored.txt source.ts"}
        }),
    });

    assert_eq!(decision.decision, DecisionKind::Deny);
    let _ = fs::remove_dir_all(root);
}

#[test]
fn builtin_source_argv_rule_matches_command_names_not_harness_subcommands() {
    let root = temp_root("builtin-source-argv-command-name");
    let config = ClientHookConfig::default();
    let registry = registry();

    let asp_rg_decision = classify_hook_with_config(HookClassificationRequest {
        registry: &registry,
        config: &config,
        platform: "codex",
        event: "pre-tool",
        payload: &json!({
            "tool_name": "Bash",
            "tool_input": {"command": "asp rg -query 'HookDecision' src/cli/agent-hooks.ts"}
        }),
    });

    assert_eq!(asp_rg_decision.decision, DecisionKind::Allow);

    let direct_rg_decision = classify_hook_with_config(HookClassificationRequest {
        registry: &registry,
        config: &config,
        platform: "codex",
        event: "pre-tool",
        payload: &json!({
            "session_id": "session-ABC_123",
            "tool_name": "Bash",
            "tool_input": {"command": "rg HookDecision src/cli/agent-hooks.ts"}
        }),
    });

    assert_eq!(direct_rg_decision.decision, DecisionKind::Deny);
    assert_eq!(
        direct_rg_decision
            .fields
            .get("configRuleId")
            .and_then(|id| id.as_str()),
        Some("deny-uncontrolled-source-search-commands")
    );

    let _ = fs::remove_dir_all(root);
}

#[test]
fn builtin_materialization_rule_is_permanent_and_source_scoped() {
    let config = ClientHookConfig::default();
    let registry = registry();
    let source_payload = json!({
        "session_id": "permanent-source-deny",
        "transcript_path": "/tmp/permanent-source-deny.jsonl",
        "tool_name": "Bash",
        "tool_input": {
            "command": "sed -n 1p crates/agent-semantic-hook/src/hook_config/core.rs"
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
        assert_eq!(decision.decision, DecisionKind::Deny);
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
fn builtin_inline_materialization_rules_use_config_and_source_paths() {
    let config = ClientHookConfig::default();
    let registry = registry();

    for (command, expected_rule) in [
        (
            "python -c \"open('crates/agent-semantic-hook/src/hook_config/core.rs').read()\"",
            "deny-uncontrolled-python-inline-source-materialization",
        ),
        (
            "node -e \"require('fs').readFileSync('crates/agent-semantic-hook/src/hook_config/core.rs')\"",
            "deny-uncontrolled-javascript-inline-source-materialization",
        ),
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
        assert_eq!(decision.decision, DecisionKind::Deny, "{command}");
        assert_eq!(
            decision
                .fields
                .get("configRuleId")
                .and_then(|id| id.as_str()),
            Some(expected_rule),
            "{command}"
        );
    }

    let non_source_decision = classify_hook_with_config(HookClassificationRequest {
        registry: &registry,
        config: &config,
        platform: "codex",
        event: "pre-tool",
        payload: &json!({
            "tool_name": "Bash",
            "tool_input": {"command": "python -c \"open('/dev/null').read()\""}
        }),
    });
    assert_eq!(non_source_decision.decision, DecisionKind::Allow);
}
