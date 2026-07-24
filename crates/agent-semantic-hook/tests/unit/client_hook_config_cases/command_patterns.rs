use super::common::{
    ClientHookConfig, DecisionKind, Duration, HookClassificationRequest, Instant,
    classify_hook_with_config, fs, json, load_client_config, registry, temp_root,
};

#[test]
fn command_contains_any_rejects_empty_patterns() {
    let root = temp_root("command-contains-empty-pattern");
    let config_path = root.join("config.toml");
    fs::write(
        &config_path,
        super::common::with_required_resident_agents(
            r#"
schemaId = "agent.semantic-protocols.hook.client-config"
schemaVersion = "1"
protocolId = "agent.semantic-protocols.hook"
protocolVersion = "1"

[[rules]]
id = "deny-empty-command-contains"
decision = "deny"

[rules.match]
tool = "Bash"
commandContainsAny = [""]
"#,
        ),
    )
    .expect("write config");

    let error = load_client_config(&config_path).expect_err("reject empty commandContainsAny");
    assert!(
        error.contains("rules[].match.commandContainsAny[] must not be empty"),
        "{error}"
    );

    let _ = fs::remove_dir_all(root);
}

#[test]
fn command_contains_any_matches_ascii_case_insensitively() {
    let root = temp_root("command-contains-case-insensitive");
    let config_path = root.join("config.toml");
    fs::write(
        &config_path,
        super::common::with_required_resident_agents(
            r#"
schemaId = "agent.semantic-protocols.hook.client-config"
schemaVersion = "1"
protocolId = "agent.semantic-protocols.hook"
protocolVersion = "1"

[[rules]]
id = "deny-case-insensitive-command-contains"
decision = "deny"
priority = 20000

[rules.match]
tool = "Bash"
commandContainsAny = ["HOOKDECISION"]
"#,
        ),
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
                "command": "rg hookdecision src/cli/agent-hooks.ts"
            }
        }),
    });

    assert_eq!(decision.decision, DecisionKind::Deny);
    assert_eq!(
        decision
            .fields
            .get("configRuleId")
            .and_then(|id| id.as_str()),
        Some("deny-case-insensitive-command-contains")
    );

    let _ = fs::remove_dir_all(root);
}

#[test]
fn argv_prefix_any_matches_a_nested_command_stage_without_matching_nearby_forms() {
    let root = temp_root("argv-prefix");
    let config_path = root.join("config.toml");
    fs::write(
        &config_path,
        super::common::with_required_resident_agents(
            r#"
schemaId = "agent.semantic-protocols.hook.client-config"
schemaVersion = "1"
protocolId = "agent.semantic-protocols.hook"
protocolVersion = "1"

[[rules]]
id = "deny-recursive-force-remove"
decision = "deny"

[rules.match]
tool = "Bash"
argvPrefixAny = [["rm", "-rf"]]
"#,
        ),
    )
    .expect("write config");
    let config = load_client_config(&config_path).expect("load client config");
    let registry = registry();

    let denied = classify_hook_with_config(HookClassificationRequest {
        registry: &registry,
        config: &config,
        platform: "codex",
        event: "pre-tool",
        payload: &json!({
            "tool_name": "Bash",
            "tool_input": {
                "command": "printf warmup && rm -rf ./generated"
            }
        }),
    });
    assert_eq!(denied.decision, DecisionKind::Deny);
    assert_eq!(
        denied.fields.get("configRuleId").and_then(|id| id.as_str()),
        Some("deny-recursive-force-remove")
    );

    let allowed = classify_hook_with_config(HookClassificationRequest {
        registry: &registry,
        config: &config,
        platform: "codex",
        event: "pre-tool",
        payload: &json!({
            "tool_name": "Bash",
            "tool_input": {
                "command": "rm -r ./generated"
            }
        }),
    });
    assert_eq!(allowed.decision, DecisionKind::Allow);

    let _ = fs::remove_dir_all(root);
}

#[test]
fn argv_prefix_any_rejects_empty_patterns() {
    let root = temp_root("argv-prefix-empty");
    let config_path = root.join("config.toml");
    fs::write(
        &config_path,
        super::common::with_required_resident_agents(
            r#"
schemaId = "agent.semantic-protocols.hook.client-config"
schemaVersion = "1"
protocolId = "agent.semantic-protocols.hook"
protocolVersion = "1"

[[rules]]
id = "invalid-empty-prefix"
decision = "deny"

[rules.match]
argvPrefixAny = [[]]
"#,
        ),
    )
    .expect("write config");

    let error = load_client_config(&config_path).expect_err("empty prefix must be rejected");
    assert!(
        error.contains("rules[].match.argvPrefixAny[0] must not be empty"),
        "{error}"
    );

    let _ = fs::remove_dir_all(root);
}

#[test]
fn configured_git_diff_routes_to_testing_resident() {
    let config = ClientHookConfig::default();
    let registry = registry();
    let payload = json!({
        "tool_name": "functions.exec_command",
        "tool_input": {"cmd": "git diff --check"}
    });
    let decision = classify_hook_with_config(HookClassificationRequest {
        registry: &registry,
        config: &config,
        platform: "codex",
        event: "pre-tool",
        payload: &payload,
    });

    assert_eq!(decision.decision, DecisionKind::Deny);
    assert_eq!(
        decision
            .fields
            .get("configRuleId")
            .and_then(serde_json::Value::as_str),
        Some("deny-uncontrolled-git-source-reads")
    );
    assert_eq!(
        decision
            .fields
            .get("targetAgentName")
            .and_then(serde_json::Value::as_str),
        Some("asp_testing")
    );
    assert_eq!(
        decision
            .fields
            .get("requiredAction")
            .and_then(serde_json::Value::as_str),
        Some("route-exact-command-to-hook-selected-resident")
    );
    assert_eq!(
        decision.subject.command.as_deref(),
        Some("git diff --check")
    );
    let decision_json = serde_json::to_value(&decision).expect("serialize hook decision");
    assert_eq!(decision_json["interactiveCommand"]["schemaVersion"], "1");
    let receipt_kind = decision_json["fields"]["receiptKind"]
        .as_str()
        .expect("configured resident dispatch receipt kind");
    assert_eq!(
        decision_json["interactiveCommand"]["argv"],
        json!([
            "asp",
            "agent",
            "session",
            "bootstrap",
            "--name",
            "asp-testing",
            "--receipt-kind",
            receipt_kind,
            "--command-json",
            "[\"/bin/sh\",\"-c\",\"git diff --check\"]"
        ])
    );
    let decision_schema: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../../schemas/semantic-agent-hook-decision.v1.schema.json"
    ))
    .expect("parse hook decision schema");
    let agent_action_schema: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../../schemas/agent-action.v1.schema.json"
    ))
    .expect("parse agent action schema");
    let schema_registry = jsonschema::Registry::new()
        .add(
            "https://agent-semantic-protocols.local/schemas/agent-action.v1.schema.json",
            agent_action_schema,
        )
        .expect("register agent action schema")
        .prepare()
        .expect("prepare hook decision schema registry");
    let validator = jsonschema::options()
        .with_registry(&schema_registry)
        .build(&decision_schema)
        .expect("compile hook decision schema");
    validator
        .validate(&decision_json)
        .expect("configured resident decision should satisfy the v1 schema");
    let rendered = agent_semantic_hook::render_platform_response(&decision)
        .expect("render configured resident deny");
    assert_eq!(
        rendered["hookSpecificOutput"]["permissionDecisionReason"],
        "asp agent session bootstrap --name asp-testing --receipt-kind \
asp-testing-execution-v1 --command-json '[\"/bin/sh\",\"-c\",\"git diff --check\"]'"
    );
    assert!(
        rendered["hookSpecificOutput"]
            .get("additionalContext")
            .is_none()
    );
    assert!(rendered.get("systemMessage").is_none());
}

#[test]
fn configurable_hook_default_rule_classification_stays_fast() {
    let root = temp_root("default-source-argv-perf");
    let config = ClientHookConfig::default();
    let registry = registry();
    let payloads = [
        json!({
            "tool_name": "Bash",
            "tool_input": {"command": "rg HookDecision src/cli/agent-hooks.ts"}
        }),
        json!({
            "tool_name": "Bash",
            "tool_input": {"command": "sed -n '1,40p' src/cli/agent-hooks.ts"}
        }),
        json!({
            "tool_name": "Bash",
            "tool_input": {"command": "wl --output src/cli/agent-hooks.ts README.md"}
        }),
        json!({
            "tool_name": "Bash",
            "tool_input": {"command": "asp rust search pipe 'HookDecision' --workspace . --view seeds"}
        }),
    ];
    // Keep the total decision count high while using short samples so unrelated
    // parallel tests cannot dominate every measurement with scheduler stalls.
    let samples = 50;
    let iterations = 1_000;
    let mut best_elapsed = Duration::MAX;
    let mut best_denied = 0usize;

    for _ in 0..samples {
        let start = Instant::now();
        let mut denied = 0usize;
        for index in 0..iterations {
            let decision = classify_hook_with_config(HookClassificationRequest {
                registry: &registry,
                config: &config,
                platform: "codex",
                event: "pre-tool",
                payload: &payloads[index % payloads.len()],
            });
            if decision.decision == DecisionKind::Deny {
                denied += 1;
            }
        }
        let elapsed = start.elapsed();
        if elapsed < best_elapsed {
            best_elapsed = elapsed;
            best_denied = denied;
        }
    }

    let per_decision = best_elapsed.as_nanos() / iterations as u128;
    eprintln!(
        "configurable_hook_default_rule_perf samples={samples} iterations={iterations} best_elapsed_ms={} best_ns_per_decision={per_decision}",
        best_elapsed.as_millis()
    );

    assert_eq!(best_denied, iterations * 3 / 4);
    assert!(
        per_decision < 250_000,
        "configurable hook classification regressed: {per_decision}ns per decision"
    );

    let _ = fs::remove_dir_all(root);
}
