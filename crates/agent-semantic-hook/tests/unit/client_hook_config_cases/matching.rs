use super::common::{
    ClientHookConfig, DecisionKind, HookClassificationRequest, classify_hook_with_config, fs, json,
    load_client_config, load_client_config_for_project, registry, temp_root,
};

#[test]
fn project_hook_rule_replaces_managed_rule_as_one_policy_unit() {
    let root = temp_root("project-hook-rule-merge");
    let config_path = root.join("managed-config.toml");
    fs::write(
        &config_path,
        agent_semantic_config::default_hook_client_config_template(),
    )
    .expect("write managed config");
    let project_config = root.join(".agents/asp.toml");
    fs::create_dir_all(project_config.parent().expect("project config parent"))
        .expect("project config dir");
    fs::write(
        &project_config,
        r#"
[[hook.rules]]
id = "registered-asp-reasoning-search"
priority = 100000
intent = "project-reasoning-search"
decision = "allow"
event = "pre-tool"
message = "Project replacement owns the complete policy unit."

[hook.rules.match]
argvPatternAny = [["asp", "<registered-language>", "search"]]
"#,
    )
    .expect("write project hook config");

    let config =
        load_client_config_for_project(&config_path, &root).expect("load merged hook config");
    let decision = classify_hook_with_config(HookClassificationRequest {
        registry: &registry(),
        config: &config,
        platform: "codex",
        event: "pre-tool",
        payload: &json!({
            "tool_name": "Bash",
            "tool_input": {"command": "asp rust search pipe ownership --workspace . --view seeds"}
        }),
    });

    assert_eq!(decision.decision, DecisionKind::Allow);
    assert_eq!(
        decision
            .fields
            .get("configRuleId")
            .and_then(|id| id.as_str()),
        Some("registered-asp-reasoning-search")
    );
    assert_eq!(
        decision
            .fields
            .get("intent")
            .and_then(|value| value.as_str()),
        Some("project-reasoning-search")
    );
    assert!(!decision.fields.contains_key("residentName"));

    let _ = fs::remove_dir_all(root);
}

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

[agents]

[[agents.residentAgents]]
enabled = true
name = "asp-explore"
role = "asp_explorer"
roles = []
permissions = []
codexAgentName = "asp_explorer"
sessionLifetime = "resident"

[[agents.residentAgents]]
enabled = true
name = "asp-testing"
role = "asp_testing"
roles = []
permissions = []
codexAgentName = "asp_testing"
sessionLifetime = "resident"

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
fn registered_reasoning_search_dispatches_before_raw_search_rules_and_lazy_loads_by_language() {
    let root = temp_root("builtin-source-argv-command-name");
    let config = ClientHookConfig::default();
    let registry = registry();

    let asp_search_decision = classify_hook_with_config(HookClassificationRequest {
        registry: &registry,
        config: &config,
        platform: "codex",
        event: "pre-tool",
        payload: &json!({
            "tool_name": "Bash",
            "tool_input": {"command": "asp rust search pipe 'HookDecision' --workspace . --view seeds"}
        }),
    });

    assert_eq!(asp_search_decision.decision, DecisionKind::Deny);
    assert_eq!(
        asp_search_decision
            .fields
            .get("configRuleId")
            .and_then(|id| id.as_str()),
        Some("registered-asp-reasoning-search")
    );
    assert_eq!(
        asp_search_decision
            .fields
            .get("intent")
            .and_then(|value| value.as_str()),
        Some("reasoning-search")
    );
    assert_eq!(
        asp_search_decision
            .fields
            .get("residentName")
            .and_then(|value| value.as_str()),
        Some("asp-explore")
    );
    assert_eq!(
        asp_search_decision
            .fields
            .get("registeredLanguageId")
            .and_then(|value| value.as_str()),
        Some("rust")
    );
    assert_eq!(
        asp_search_decision
            .fields
            .get("providerActivationRefresh")
            .and_then(|value| value.as_str()),
        Some("hook-auto")
    );
    assert_eq!(
        asp_search_decision
            .fields
            .get("providerLazyLoadCommand")
            .and_then(|value| value.as_str()),
        Some("asp install language rust")
    );
    assert!(
        asp_search_decision
            .fields
            .values()
            .all(|value| value.as_str() != Some("asp sync"))
    );

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

    for command in [
        "asp help",
        "asp rust query --selector 'rust://src/lib.rs#item/function/run' --code",
    ] {
        let non_reasoning_decision = classify_hook_with_config(HookClassificationRequest {
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
            non_reasoning_decision.decision,
            DecisionKind::Allow,
            "non-reasoning command must stay outside resident search dispatch: {command}"
        );
        assert!(!non_reasoning_decision.fields.contains_key("residentName"));
    }

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
fn action_first_rule_denies_projected_reads_before_shell_expansion() {
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
            .get("hostAction")
            .expect("typed host action receipt");
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
    assert_eq!(native_read.fields["hostAction"]["action"], "read");
    assert_eq!(native_read.fields["hostAction"]["effect"], "read");
    assert_eq!(
        native_read.fields["hostAction"]["authority"],
        "raw-host-action"
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
