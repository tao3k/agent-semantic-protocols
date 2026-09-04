use super::common::ClientHookConfig;
use super::common::DecisionKind;
use super::common::Duration;
use super::common::HookClassificationRequest;
use super::common::bind_confirmed_reader;
use super::common::classify_hook_with_config;
use super::common::fs;
use super::common::json;
use super::common::load_client_config;
use super::common::registry;
use super::common::temp_root;

#[test]
fn repository_git_history_command_set_routes_only_history_inspection_to_testing() {
    let root = temp_root("git-history-command-set");
    let config_path = root.join("config.toml");
    fs::write(
        &config_path,
        agent_semantic_hook::default_client_config_template(),
    )
    .expect("write config");
    let config = load_client_config(&config_path).expect("load default config");
    let runtime = registry();

    for command in [
        "git log --oneline -50",
        "rtk --ultra-compact git show HEAD",
        "git blame src/lib.rs",
    ] {
        let decision = classify_hook_with_config(HookClassificationRequest {
            registry: &runtime,
            config: &config,
            platform: "codex",
            event: "pre-tool",
            payload: &json!({
                "tool_name": "Bash",
                "tool_input": { "command": command }
            }),
        });
        assert_eq!(decision.decision, DecisionKind::Deny, "{command}");
        assert_eq!(
            decision
                .fields
                .get("configRuleId")
                .and_then(serde_json::Value::as_str),
            Some("git-history-inspection-dispatch"),
            "{command}"
        );
        assert_eq!(
            decision
                .fields
                .get("targetAgent")
                .and_then(serde_json::Value::as_str),
            Some("asp_testing"),
            "{command}"
        );
    }

    for command in ["git status --short", "git grep hook", "git ls-files"] {
        let decision = classify_hook_with_config(HookClassificationRequest {
            registry: &runtime,
            config: &config,
            platform: "codex",
            event: "pre-tool",
            payload: &json!({
                "tool_name": "Bash",
                "tool_input": { "command": command }
            }),
        });
        assert_ne!(
            decision
                .fields
                .get("configRuleId")
                .and_then(serde_json::Value::as_str),
            Some("git-history-inspection-dispatch"),
            "{command}"
        );
    }

    let _ = fs::remove_dir_all(root);
}

#[test]
fn terminal_allow_rule_uses_parser_owned_process_environment_assignment() {
    let root = temp_root("process-environment-assignment");
    let config_path = root.join("config.toml");
    fs::write(
        &config_path,
        super::common::with_direct_dispatch_roles(
            r#"
schemaId = "agent.semantic-protocols.hook.client-config"
schemaVersion = "1"
protocolId = "agent.semantic-protocols.hook"
protocolVersion = "1"

[[rules]]
id = "allow-explicit-ci-mode"
priority = 200000
decision = "allow"
terminal = true

[rules.match]
processEnvironmentAssignmentAny = ["CI_MODE=1"]

[[rules]]
id = "deny-shell-fallback"
decision = "deny"

[rules.match]
toolAny = ["Bash", "functions.exec_command"]
"#,
        ),
    )
    .expect("write config");
    let config = load_client_config(&config_path).expect("load client config");
    let runtime = crate::classifier::registry_without_providers();

    for command in [
        "CI_MODE=1 cargo test",
        "TRACE=1 CI_MODE=1 cargo test",
        "env CI_MODE=1 cargo test",
        "export CI_MODE=1; exec cargo test",
        "export CI_MODE=1; cargo test",
    ] {
        let decision = classify_hook_with_config(HookClassificationRequest {
            registry: &runtime,
            config: &config,
            platform: "codex",
            event: "pre-tool",
            payload: &json!({
                "tool_name": "functions.exec_command",
                "tool_input": { "cmd": command }
            }),
        });
        assert_eq!(decision.decision, DecisionKind::Allow, "{command}");
        assert_eq!(
            decision
                .fields
                .get("configRuleId")
                .and_then(serde_json::Value::as_str),
            Some("allow-explicit-ci-mode")
        );
    }

    for command in [
        "NOT_CI_MODE=1 cargo test",
        "printf warmup && CI_MODE=1 cargo test",
        "bash -lc 'CI_MODE=1 cargo test'",
    ] {
        let decision = classify_hook_with_config(HookClassificationRequest {
            registry: &runtime,
            config: &config,
            platform: "codex",
            event: "pre-tool",
            payload: &json!({
                "tool_name": "functions.exec_command",
                "tool_input": { "cmd": command }
            }),
        });
        assert_eq!(decision.decision, DecisionKind::Deny, "{command}");
    }

    let _ = fs::remove_dir_all(root);
}

#[test]
fn command_contains_any_rejects_empty_patterns() {
    let root = temp_root("command-contains-empty-pattern");
    let config_path = root.join("config.toml");
    fs::write(
        &config_path,
        super::common::with_direct_dispatch_roles(
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
        super::common::with_direct_dispatch_roles(
            r#"
schemaId = "agent.semantic-protocols.hook.client-config"
schemaVersion = "1"
protocolId = "agent.semantic-protocols.hook"
protocolVersion = "1"

[[rules]]
id = "deny-case-insensitive-command-contains"
decision = "deny"
priority = 50000

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
        super::common::with_direct_dispatch_roles(
            r#"
schemaId = "agent.semantic-protocols.hook.client-config"
schemaVersion = "1"
protocolId = "agent.semantic-protocols.hook"
protocolVersion = "1"

[[rules]]
id = "deny-recursive-force-remove"
decision = "deny"
matcherPolicies = ["wrapped_command"]

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
        super::common::with_direct_dispatch_roles(
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
fn bash_confirmed_reader_projects_explore_choice_plane_guidance() {
    let config = ClientHookConfig::default();
    let registry = registry();
    let mut payload = json!({
        "tool_name": "Bash",
        "tool_input": {"command": "head src/app.ts"}
    });
    agent_semantic_hook::bind_plugin_host_matcher(&mut payload, "Bash")
        .expect("bind canonical Bash matcher");
    bind_confirmed_reader(&mut payload, "src/app.ts");
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
        Some("route-read-to-asp-languages")
    );
    assert_eq!(
        decision
            .fields
            .get("requiredAction")
            .and_then(serde_json::Value::as_str),
        Some("collaboration.spawn_agent")
    );
    assert_eq!(
        decision
            .fields
            .get("collaborationTool")
            .and_then(serde_json::Value::as_str),
        Some("spawn_agent")
    );
    assert_eq!(
        decision
            .fields
            .get("collaborationNamespace")
            .and_then(serde_json::Value::as_str),
        Some("collaboration")
    );
    assert_eq!(decision.fields["targetAgent"], "asp_explorer");
    assert_eq!(
        decision.fields["agentSessionAction"],
        "dispatch-registered-agent"
    );
    for forbidden in [
        "residentName",
        "residentChildName",
        "canonicalTarget",
        "targetAgentName",
        "targetAgentDescription",
    ] {
        assert!(
            !decision.fields.contains_key(forbidden),
            "Hook must not materialize Runtime lifecycle state through {forbidden}"
        );
    }
    assert_eq!(decision.subject.command.as_deref(), Some("head src/app.ts"));
    let decision_json = serde_json::to_value(&decision).expect("serialize hook decision");
    assert!(
        decision_json.get("interactiveCommand").is_none(),
        "Hook decisions must not serialize a Rust-owned lifecycle command"
    );
    assert_eq!(
        decision_json["fields"]["agentAction"]["hostInvocation"]["action"],
        "execute"
    );
    assert!(
        decision_json["fields"]["agentAction"]["semanticCapabilities"]
            .as_array()
            .is_some_and(|capabilities| capabilities.iter().any(|capability| {
                capability["action"] == "read" && capability["evidence"] == "reader-probe"
            }))
    );
    let decision_schema: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../../schemas/semantic-agent-hook-decision.v1.schema.json"
    ))
    .expect("parse hook decision schema");
    let agent_action_schema: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../../schemas/agent-action.v1.schema.json"
    ))
    .expect("parse agent action schema");
    let command_invocation_schema: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../../schemas/semantic-command-invocation.v1.schema.json"
    ))
    .expect("parse command invocation schema");
    let schema_registry = jsonschema::Registry::new()
        .add(
        "https://agent-semantic-protocols.dev/schemas/agent-action.v1.schema.json",
            agent_action_schema,
        )
        .expect("register agent action schema")
        .add(
            "https://agent-semantic-protocols.dev/schemas/semantic-command-invocation.v1.schema.json",
            command_invocation_schema,
        )
        .expect("register command invocation schema")
        .prepare()
        .expect("prepare hook decision schema registry");
    let validator = jsonschema::options()
        .with_registry(&schema_registry)
        .build(&decision_schema)
        .expect("compile hook decision schema");
    validator
        .validate(&decision_json)
        .expect("configured resident decision should satisfy the v1 schema");
    let mut stringified_actions = decision_json.clone();
    stringified_actions["fields"]["normalizedActions"] =
        serde_json::Value::String("[]".to_string());
    assert!(
        validator.validate(&stringified_actions).is_err(),
        "v1 normalizedActions must remain a typed object array"
    );
    let rendered = agent_semantic_hook::render_platform_response(&decision)
        .expect("render configured resident deny");
    assert!(
        rendered["hookSpecificOutput"]["permissionDecisionReason"]
            .as_str()
            .is_some_and(|message| {
                message.contains("`collaboration.spawn_agent`")
                    && message.contains("`collaboration.list_agents`")
            }),
        "configured deny must reference the native collaboration tools: {rendered}"
    );
    assert!(rendered.get("systemMessage").is_some());
}

#[test]
fn claude_platform_uses_configured_native_agent_symbol() {
    let config = ClientHookConfig::default();
    let runtime = registry();
    let mut payload = json!({
        "tool_name": "Bash",
        "tool_input": {"command": "head src/app.ts"}
    });
    agent_semantic_hook::bind_plugin_host_matcher(&mut payload, "Bash")
        .expect("bind canonical Bash matcher");
    bind_confirmed_reader(&mut payload, "src/app.ts");
    let decision = classify_hook_with_config(HookClassificationRequest {
        registry: &runtime,
        config: &config,
        platform: "claude",
        event: "pre-tool",
        payload: &payload,
    });

    assert_eq!(decision.decision, DecisionKind::Deny);
    assert_eq!(decision.fields["targetAgent"], "asp_explorer");
    assert_eq!(decision.fields["targetAgentSymbol"], "@agent-asp-explorer");
    assert!(decision.message.contains("`@agent-asp-explorer`"));
}

#[test]
fn configurable_hook_default_rule_classification_stays_fast() {
    let root = temp_root("default-source-argv-perf");
    let config = ClientHookConfig::default();
    let registry = registry();
    let payloads = [
        json!({
            "tool_name": "Bash",
            "tool_input": {"command": "head src/cli/agent-hooks.ts"}
        }),
        json!({
            "tool_name": "Bash",
            "tool_input": {"command": "head src/cli/agent-hooks.ts"}
        }),
        json!({
            "tool_name": "Bash",
            "tool_input": {"command": "head README.md"}
        }),
        json!({
            "tool_name": "Bash",
            "tool_input": {"command": "asp search playbook --language rust 'agent hooks' --workspace ."}
        }),
    ]
    .map(|mut payload| {
        agent_semantic_hook::bind_plugin_host_matcher(&mut payload, "Bash")
            .expect("bind canonical Bash matcher");
        if let Some(subject) = payload["tool_input"]["command"]
            .as_str()
            .filter(|command| command.starts_with("head "))
            .and_then(|command| command.split_whitespace().last())
            .map(str::to_owned)
        {
            bind_confirmed_reader(&mut payload, &subject);
        }
        payload
    });
    // Keep the total decision count high while using short samples so unrelated
    // parallel tests cannot dominate every measurement with scheduler stalls.
    // The shipped performance contract is exercised by the release black-box
    // matrix. Keep ordinary debug `cargo test` bounded so parallel functional
    // tests cannot turn this microbenchmark into a multi-minute CPU convoy.
    let (samples, iterations) = if cfg!(debug_assertions) {
        (4, 250)
    } else {
        (50, 1_000)
    };
    let mut best_elapsed = Duration::MAX;
    let mut best_denied = 0usize;

    for _ in 0..samples {
        let start = thread_cpu_time();
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
        let elapsed = thread_cpu_time().saturating_sub(start);
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
    for (payload_index, payload) in payloads.iter().enumerate() {
        let started = thread_cpu_time();
        for _ in 0..200 {
            let _ = classify_hook_with_config(HookClassificationRequest {
                registry: &registry,
                config: &config,
                platform: "codex",
                event: "pre-tool",
                payload,
            });
        }
        eprintln!(
            "configurable_hook_default_rule_payload_perf index={payload_index} ns_per_decision={}",
            thread_cpu_time().saturating_sub(started).as_nanos() / 200
        );
    }

    assert_eq!(best_denied, iterations);
    // Debug builds exercise the functional path but include instrumentation and
    // allocator noise that are not representative of the shipped hook binary.
    // Keep the production performance gate strict in release builds.
    let threshold_ns = if cfg!(debug_assertions) {
        1_000_000
    } else {
        250_000
    };
    assert!(
        per_decision < threshold_ns,
        "configurable hook classification regressed: {per_decision}ns per decision (threshold={threshold_ns}ns)"
    );

    let _ = fs::remove_dir_all(root);
}

#[cfg(unix)]
fn thread_cpu_time() -> Duration {
    let mut value = libc::timespec {
        tv_sec: 0,
        tv_nsec: 0,
    };
    let status = unsafe { libc::clock_gettime(libc::CLOCK_THREAD_CPUTIME_ID, &mut value) };
    assert_eq!(status, 0, "read thread CPU clock");
    Duration::new(value.tv_sec as u64, value.tv_nsec as u32)
}

#[cfg(not(unix))]
fn thread_cpu_time() -> Duration {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("system time after Unix epoch")
}

#[test]
fn break_glass_command_value_is_not_reclassified_as_registered_search() {
    let config = ClientHookConfig::default();
    let registry = registry();
    for protected_command in [
        "asp rust search --workspace . --treesitter-query '(identifier) @id'",
        "jq . /tmp/runtime-server-owner-spawn.v1.json",
        "git diff -- crates/agent-semantic-hook/src/tool_action.rs",
    ] {
        let payload = json!({
            "tool_name": "Bash",
            "tool_input": {
                "command": format!(
                    "asp hook break-glass mint --defect-kind local-hook-policy-authority-unavailable --command \"{protected_command}\" ."
                )
            }
        });

        let decision = classify_hook_with_config(HookClassificationRequest {
            registry: &registry,
            config: &config,
            platform: "codex",
            event: "pre-tool",
            payload: &payload,
        });

        assert_eq!(
            decision.decision,
            DecisionKind::Allow,
            "{protected_command}"
        );
        assert!(decision.subject.paths.is_empty(), "{protected_command}");
    }
}
