use super::common::{
    ClientHookConfig, DecisionKind, HookClassificationRequest, classify_hook_with_config, fs, json,
    load_client_config, load_client_config_for_project, registry, temp_root,
    with_required_resident_agents,
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

[agents.placeholders]
explore = "asp-explore"
testing = "asp-testing"

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

    let legacy_query_decision = classify_hook_with_config(HookClassificationRequest {
        registry: &registry,
        config: &config,
        platform: "codex",
        event: "pre-tool",
        payload: &json!({
            "tool_name": "Bash",
            "tool_input": {"command": "asp rust query --term HookDecision --workspace . --code"}
        }),
    });
    assert_eq!(legacy_query_decision.decision, DecisionKind::Deny);
    assert_eq!(
        legacy_query_decision
            .fields
            .get("configRuleId")
            .and_then(|id| id.as_str()),
        Some("registered-asp-reasoning-search")
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

    for (label, payload) in [
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
    ] {
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
            "{label}"
        );
        assert_eq!(decision.fields["agentAction"]["action"], "read", "{label}");
        assert_eq!(decision.fields["agentAction"]["effect"], "read", "{label}");
        assert_eq!(
            decision.fields["normalizedActions"][0]["operationIntent"], "direct-read",
            "{label}"
        );
        assert!(
            decision.fields["normalizedActions"][0]["paths"]
                .as_array()
                .is_some_and(|paths| !paths.is_empty()),
            "{label}: {decision:?}"
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

#[test]
fn builtin_inline_materialization_rules_use_config_and_source_paths() {
    let config = ClientHookConfig::default();
    let registry = crate::classifier::rust_registry();

    for (command, expected_rule) in [
        (
            "python -c \"open('crates/agent-semantic-hook/src/hook_config/core/implementation.rs').read()\"",
            "deny-uncontrolled-python-inline-source-materialization",
        ),
        (
            "node -e \"require('fs').readFileSync('crates/agent-semantic-hook/src/hook_config/core/implementation.rs')\"",
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

#[test]
fn default_config_deny_rules_have_end_to_end_match_witnesses() {
    let config = ClientHookConfig::default();
    let mut registry = crate::classifier::rust_registry();
    registry
        .providers
        .push(crate::classifier::typescript_provider());
    registry.project_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("workspace root")
        .display()
        .to_string();
    let source = "crates/agent-semantic-hook/src/tool_action.rs";
    let json_document = "schemas/semantic-agent-hook-decision.v1.schema.json";

    let cases = [
        (
            "registered reasoning search",
            json!({
                "tool_name": "Bash",
                "tool_input": {
                    "command": "asp rust search pipe HookDecision --workspace . --view seeds"
                }
            }),
            DecisionKind::Deny,
            "registered-asp-reasoning-search",
        ),
        (
            "resident testing dispatch",
            json!({
                "tool_name": "Bash",
                "tool_input": {"command": "cargo test -p agent-semantic-hook"}
            }),
            DecisionKind::Deny,
            "resident-testing-dispatch",
        ),
        (
            "native registered source search",
            json!({
                "tool_name": "Grep",
                "tool_input": {"pattern": "HookDecision", "path": "*.rs"}
            }),
            DecisionKind::Deny,
            "deny-raw-registered-source-search-action",
        ),
        (
            "wrapped registered source read",
            json!({
                "tool_name": "Bash",
                "tool_input": {"command": format!("rtk read {source}")}
            }),
            DecisionKind::Deny,
            "deny-raw-registered-source-action",
        ),
        (
            "provider search json",
            json!({
                "tool_name": "Bash",
                "tool_input": {
                    "command": "ts-harness search lexical HookDecision owner tests --json ."
                }
            }),
            DecisionKind::Deny,
            "deny-agent-search-json",
        ),
        (
            "source apply patch",
            json!({
                "tool_name": "apply_patch",
                "tool_input": {
                    "patch": format!(
                        "*** Begin Patch\n*** Update File: {source}\n@@\n-//! Normalizes\n+//! Normalizes payloads\n*** End Patch\n"
                    )
                }
            }),
            DecisionKind::Deny,
            "materialize-apply-patch-policy",
        ),
        (
            "native registered source read",
            json!({
                "tool_name": "Read",
                "tool_input": {"file_path": source}
            }),
            DecisionKind::Deny,
            "materialize-registered-source-read-action",
        ),
        (
            "native registered source pattern read",
            json!({
                "tool_name": "Read",
                "tool_input": {"path": "*.rs"}
            }),
            DecisionKind::Deny,
            "materialize-registered-source-read-action",
        ),
        (
            "nested registered source read",
            json!({
                "tool_name": "multi_tool_use.parallel",
                "tool_input": {
                    "tool_uses": [{
                        "recipient_name": "functions.read_file",
                        "parameters": {"path": source}
                    }]
                }
            }),
            DecisionKind::Deny,
            "materialize-registered-source-read-action",
        ),
        (
            "generic configured source access",
            json!({
                "tool_name": "Bash",
                "tool_input": {
                    "command": format!("reader '.read_text(' {source}")
                }
            }),
            DecisionKind::Deny,
            "materialize-source-access-policy",
        ),
        (
            "raw source search command",
            json!({
                "tool_name": "Bash",
                "tool_input": {"command": "rg HookDecision crates"}
            }),
            DecisionKind::Deny,
            "deny-uncontrolled-source-search-commands",
        ),
        (
            "unbounded structured projection",
            json!({
                "tool_name": "Bash",
                "tool_input": {
                    "command": format!(
                        "jq '{{properties: (.properties | to_entries[:32] | map({{key, type: .value.type}})), required: (.required[:32] // [])}}' {json_document}"
                    )
                }
            }),
            DecisionKind::Deny,
            "deny-unbounded-structured-projection",
        ),
        (
            "source materialization command",
            json!({
                "tool_name": "Bash",
                "tool_input": {"command": format!("sed -n '1,8p' {source}")}
            }),
            DecisionKind::Deny,
            "deny-uncontrolled-source-materialization-commands",
        ),
        (
            "python inline source materialization",
            json!({
                "tool_name": "Bash",
                "tool_input": {
                    "command": format!("python -c \"open('{source}').read()\"")
                }
            }),
            DecisionKind::Deny,
            "deny-uncontrolled-python-inline-source-materialization",
        ),
        (
            "javascript inline source materialization",
            json!({
                "tool_name": "Bash",
                "tool_input": {
                    "command": format!("node -e \"require('fs').readFileSync('{source}')\"")
                }
            }),
            DecisionKind::Deny,
            "deny-uncontrolled-javascript-inline-source-materialization",
        ),
        (
            "git source read",
            json!({
                "tool_name": "Bash",
                "tool_input": {"command": format!("git show HEAD:{source}")}
            }),
            DecisionKind::Deny,
            "deny-uncontrolled-git-source-reads",
        ),
    ];

    let configured_deny_rules = agent_semantic_config::default_hook_client_config_file()
        .expect("default hook config")
        .rules
        .into_iter()
        .filter(|rule| {
            rule.enabled
                && matches!(
                    rule.decision,
                    agent_semantic_config::HookClientConfigDecision::Deny
                )
        })
        .map(|rule| rule.id)
        .collect::<std::collections::BTreeSet<_>>();
    let witnessed_deny_rules = cases
        .iter()
        .map(|(_, _, _, rule_id)| (*rule_id).to_string())
        .collect::<std::collections::BTreeSet<_>>();
    assert_eq!(
        witnessed_deny_rules, configured_deny_rules,
        "every enabled default deny rule must have one end-to-end witness"
    );

    for (label, payload, expected_decision, expected_rule) in cases {
        let decision = classify_hook_with_config(HookClassificationRequest {
            registry: &registry,
            config: &config,
            platform: "codex",
            event: "pre-tool",
            payload: &payload,
        });

        assert_eq!(
            decision.decision, expected_decision,
            "{label}: {decision:?}"
        );
        assert_eq!(
            decision
                .fields
                .get("configRuleId")
                .and_then(serde_json::Value::as_str),
            Some(expected_rule),
            "{label}: {decision:?}"
        );
        if label == "unbounded structured projection" {
            assert_eq!(
                decision
                    .fields
                    .get("agentAction")
                    .and_then(|action| action.get("subjects"))
                    .and_then(serde_json::Value::as_array)
                    .map(|subjects| {
                        subjects
                            .iter()
                            .filter_map(|subject| subject.get("value"))
                            .filter_map(serde_json::Value::as_str)
                            .collect::<Vec<_>>()
                    }),
                Some(vec![json_document]),
                "{label}: filter program must not become a path subject: {decision:?}"
            );
        }
    }
}
