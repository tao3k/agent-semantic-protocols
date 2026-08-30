use super::{
    ClientHookConfig, DecisionKind, HookClassificationRequest, classify_hook_with_config, fs, json,
    load_client_config, load_client_config_for_project, registry, temp_root,
};

#[test]
fn project_hook_rule_replaces_managed_rule_as_one_policy_unit() {
    let root = temp_root("project-hook-rule-merge");
    let config_path = root.join("managed-config.toml");
    fs::write(
        &config_path,
        &agent_semantic_config::default_hook_client_config_template(),
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

[[rules]]
id = "deny-wl-source-argv"
priority = 50000
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

    for command in ["wl --flag2 flag3 README"] {
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

    for command in ["wl --output *.ts README"] {
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
            "an output operand must not be reclassified as a source read: {command}"
        );
    }

    let inline_output = classify_hook_with_config(HookClassificationRequest {
        registry: &registry,
        config: &config,
        platform: "codex",
        event: "pre-tool",
        payload: &json!({
            "tool_name": "Bash",
            "tool_input": {"command": "wl --output=*.ts README"}
        }),
    });
    assert_eq!(inline_output.decision, DecisionKind::Allow);

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
fn same_action_collects_all_matching_rules_before_priority_selection() {
    let root = temp_root("same-action-priority-dominance");
    let config_path = root.join("config.toml");
    fs::write(
        &config_path,
        r#"
schemaId = "agent.semantic-protocols.hook.client-config"
schemaVersion = "1"
protocolId = "agent.semantic-protocols.hook"
protocolVersion = "1"

[[rules]]
id = "lower-priority-match"
priority = 100
decision = "deny"

[rules.match]
commandContainsAny = ["same-action-witness"]

[[rules]]
id = "higher-priority-match"
priority = 200
decision = "deny"

[rules.match]
commandContainsAny = ["same-action-witness"]
"#,
    )
    .expect("write config");
    let config_source = fs::read_to_string(&config_path).expect("read config");
    fs::write(
        &config_path,
        crate::client_hook_config::matching::with_direct_dispatch_roles(&config_source),
    )
    .expect("write config with direct dispatch roles");
    let config = load_client_config(&config_path).expect("load config");
    let runtime = registry();
    let decision = classify_hook_with_config(HookClassificationRequest {
        registry: &runtime,
        config: &config,
        platform: "codex",
        event: "pre-tool",
        payload: &json!({
            "tool_name": "Bash",
            "tool_input": {"command": "same-action-witness"}
        }),
    });

    assert_eq!(
        decision.fields.get("configRuleId"),
        Some(&json!("higher-priority-match")),
        "same-action matching must aggregate candidates before applying Rule DSL priority: {decision:?}"
    );
    let _ = fs::remove_dir_all(root);
}

#[test]
fn registered_reasoning_search_dispatches_before_raw_search_rules_and_lazy_loads_by_language() {
    let root = temp_root("builtin-source-argv-command-name");
    let config = ClientHookConfig::default();
    let mut registry = registry();
    let source_config = agent_semantic_config::default_hook_client_config_file()
        .expect("load canonical Hook client config");
    let source_rule = source_config
        .rules
        .iter()
        .find(|rule| rule.id == "registered-asp-reasoning-search")
        .expect("registered ASP reasoning-search rule");
    assert!(source_rule.matcher.is_none());
    assert_eq!(
        source_rule.matcher_policies,
        [agent_semantic_config::HookClientMatcherPolicy::WrappedCommand]
    );
    assert_eq!(
        source_rule.match_config.argv_pattern_any,
        [vec![
            "asp".to_owned(),
            "<registered-language>".to_owned(),
            "search".to_owned(),
        ]]
    );
    config
        .apply_language_provider_projection(&mut registry)
        .expect("apply canonical language/provider projections");
    assert!(
        registry
            .policy_providers
            .iter()
            .any(|provider| provider.language_id.as_str() == "rust"),
        "canonical projection must register Rust"
    );
    let search_command = "asp rust search pipe 'HookDecision' --workspace . --view seeds";
    let stages = agent_semantic_shell_parser::parse_bash_command_candidates(search_command)
        .expect("parse direct ASP search command");
    assert!(
        agent_semantic_shell_parser::command_stages_match_wrapped_prefix(
            &stages,
            &["asp".to_owned(), "rust".to_owned(), "search".to_owned()],
        )
        .routes_protected(),
        "direct ASP search must satisfy the wrapped-prefix matcher"
    );
    let asp_search_decision = classify_hook_with_config(HookClassificationRequest {
        registry: &registry,
        config: &config,
        platform: "codex",
        event: "pre-tool",
        payload: &json!({
            "tool_name": "Bash",
            "tool_input": {"command": search_command}
        }),
    });

    assert_eq!(
        asp_search_decision.decision,
        DecisionKind::Deny,
        "decision={asp_search_decision:#?}; config={config:#?}"
    );
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
    assert!(
        asp_search_decision
            .message
            .contains("Delegate the denied search to ASP Explorer"),
        "registered search dispatch must explain the Explorer evidence path: {}",
        asp_search_decision.message
    );
    assert!(
        asp_search_decision
            .message
            .contains("materialize only that selector from the main thread"),
        "registered search dispatch must return exact query materialization to the main thread: {}",
        asp_search_decision.message
    );
    assert!(
        asp_search_decision
            .message
            .contains("Use `--json` only for explicitly requested debugging"),
        "registered search dispatch must keep JSON outside normal Explorer search: {}",
        asp_search_decision.message
    );
    assert_eq!(
        asp_search_decision.fields["collaborationNamespace"].as_str(),
        Some("collaboration")
    );
    assert_eq!(
        asp_search_decision.fields["collaborationTool"].as_str(),
        Some("spawn_agent")
    );
    assert!(!asp_search_decision.fields.contains_key("residentName"));
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

    for (language_id, command) in [
        (
            "org",
            "asp org search owner docs/spec.org items --query capability --workspace . --view seeds",
        ),
        (
            "md",
            "asp md search owner README.md items --query runtime --workspace . --view seeds",
        ),
    ] {
        let provider_route_decision = classify_hook_with_config(HookClassificationRequest {
            registry: &registry,
            config: &config,
            platform: "codex",
            event: "pre-tool",
            payload: &json!({
                "tool_name": "Bash",
                "tool_input": {"command": command}
            }),
        });
        assert_eq!(provider_route_decision.decision, DecisionKind::Deny);
        assert_eq!(
            provider_route_decision
                .fields
                .get("configRuleId")
                .and_then(|id| id.as_str()),
            Some("registered-asp-reasoning-search")
        );
        assert_eq!(
            provider_route_decision
                .fields
                .get("registeredLanguageId")
                .and_then(|value| value.as_str()),
            Some(language_id)
        );
    }

    let structured_query_decision = classify_hook_with_config(HookClassificationRequest {
        registry: &registry,
        config: &config,
        platform: "codex",
        event: "pre-tool",
        payload: &json!({
            "session_id": "session-ABC_123",
            "tool_name": "Bash",
            "tool_input": {
                "command": "asp rust query --selector rust://src/lib.rs#item/function/run --workspace . --projection source"
            }
        }),
    });

    assert_eq!(
        structured_query_decision.decision,
        DecisionKind::Deny,
        "exact ASP query must dispatch as structured projection: {structured_query_decision:#?}"
    );
    assert_eq!(structured_query_decision.event, "pre-tool");
    assert_eq!(
        structured_query_decision
            .fields
            .get("configRuleId")
            .and_then(|value| value.as_str()),
        Some("registered-asp-structured-projection")
    );
    assert_eq!(
        structured_query_decision
            .fields
            .get("intent")
            .and_then(|value| value.as_str()),
        Some("structured-projection")
    );

    for command in ["asp help"] {
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
