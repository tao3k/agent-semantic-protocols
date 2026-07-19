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
            "tool_input": {"command": "asp rg -query 'HookDecision' src/cli/agent-hooks.ts"}
        }),
    ];
    let samples = 4;
    let iterations = 20_000;
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

    assert_eq!(best_denied, iterations / 2);
    assert!(
        best_elapsed < Duration::from_millis(5_000),
        "configurable hook classification regressed: {best_elapsed:?} for {iterations} iterations"
    );

    let _ = fs::remove_dir_all(root);
}
