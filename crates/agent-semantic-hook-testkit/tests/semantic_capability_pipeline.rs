use agent_semantic_config::{LanguageId, ProviderId};
use agent_semantic_hook::{
    ClientHookConfig, CommandTemplate, HookPolicy, HookProviderProjection, HookRuntime,
};
use agent_semantic_hook_testkit::{classify_codex_plugin_scenario, classify_hook_scenario};
use serde_json::{Value, json};

fn runtime(project_root: &str) -> HookRuntime {
    HookRuntime {
        project_root: project_root.to_owned(),
        rankers: Vec::new(),
        providers: Vec::new(),
        policy_providers: Vec::new(),
    }
}

fn runtime_with_rust_policy_projection(project_root: &str) -> HookRuntime {
    let route = || CommandTemplate {
        argv: Vec::new(),
        stdin_mode: None,
    };
    HookRuntime {
        project_root: project_root.to_owned(),
        rankers: Vec::new(),
        providers: Vec::new(),
        policy_providers: vec![HookProviderProjection {
            language_id: LanguageId::new("rust"),
            provider_id: ProviderId::new("asp-rust"),
            package_roots: vec!["crates".to_owned()],
            source_extensions: vec![".rs".to_owned()],
            config_files: Vec::new(),
            policy: HookPolicy::default(),
            owner_route: route(),
            lexical_route: route(),
            ingest_route: route(),
        }],
    }
}

fn shell(command: &str) -> Value {
    json!({
        "tool_name": "Bash",
        "tool_input": { "command": command }
    })
}

fn registered_read_only_action(agent_name: &str, tool_name: &str, tool_input: Value) -> Value {
    let (roles, allowed_rule_intents) = match agent_name {
        "asp_explorer" => (
            json!(["explore", "explorer", "subagent"]),
            json!(["reasoning-search", "structured-projection"]),
        ),
        "asp_testing" => (
            json!(["build", "subagent", "testing"]),
            json!([
                "test-build-command",
                "review-command",
                "live-corpus-qualification",
                "git-history-inspection"
            ]),
        ),
        _ => (json!(["subagent"]), json!([])),
    };
    json!({
        "tool_name": tool_name,
        "tool_input": tool_input,
        "is_subagent": true,
        "agent_id": "registered-child-session",
        "agent_type": agent_name,
        "registration_verified": true,
        "registered_agent_name": agent_name,
        "registered_denied_actions": ["edit"],
        "registered_allowed_rule_intents": allowed_rule_intents,
        "agent_roles": roles,
    })
}

fn classify(runtime: &HookRuntime, command: &str) -> Value {
    classify_codex_plugin_scenario(
        runtime,
        &ClientHookConfig::default(),
        "pre-tool",
        &shell(command),
        Some("Bash"),
        None,
    )
    .expect("classify shell scenario")
}

fn managed_hook_config() -> ClientHookConfig {
    let root = tempfile::tempdir().expect("temporary managed Hook config root");
    let path = root.path().join("config.toml");
    std::fs::write(&path, agent_semantic_hook::default_client_config_template())
        .expect("write canonical managed Hook config");
    agent_semantic_hook::load_client_config(&path).expect("compile canonical managed Hook config")
}

fn capabilities(decision: &Value) -> Vec<Value> {
    decision["fields"]["agentAction"]["semanticCapabilities"]
        .as_array()
        .cloned()
        .unwrap_or_default()
}

#[test]
fn non_language_operands_do_not_invent_read_capabilities() {
    let decision = classify(&runtime("."), "unknown-consumer Cargo.lock");
    assert!(capabilities(&decision).iter().any(|capability| {
        capability["action"] == "execute" && capability["evidence"] == "host-matcher"
    }));
    assert!(
        !capabilities(&decision)
            .iter()
            .any(|capability| capability["action"] == "read")
    );
}

#[test]
fn shell_path_operand_mutation_is_not_a_structured_document_read() {
    let decision = classify(&runtime("."), "git add -u -- policy.toml");
    assert_ne!(
        decision["fields"]["configRuleId"],
        "route-shell-structured-document-read"
    );
    assert!(
        !capabilities(&decision)
            .iter()
            .any(|capability| capability["action"] == "read")
    );
}

#[test]
fn shell_redirections_are_parser_facts_not_executable_name_rules() {
    for (command, expected) in [
        ("unknown-consumer < src/lib.rs", "read"),
        ("unknown-consumer > generated.rs", "edit"),
        ("unknown-consumer <> state.rs", "read"),
        ("unknown-consumer <> state.rs", "edit"),
    ] {
        let decision = classify(&runtime("."), command);
        assert!(
            capabilities(&decision).iter().any(|capability| {
                capability["action"] == expected && capability["evidence"] == "shell-redirection"
            }),
            "command={command} expected={expected}"
        );
    }
    let heredoc = classify(&runtime("."), "unknown-consumer <<EOF");
    assert!(!capabilities(&heredoc).iter().any(|capability| {
        capability["action"] == "read" && capability["evidence"] == "shell-redirection"
    }));
}

#[test]
fn bounded_structured_projection_dominates_generic_source_access_deny() {
    let root = tempfile::tempdir().expect("temporary workspace");
    std::fs::write(root.path().join("policy.json"), b"{\"name\":\"asp\"}")
        .expect("structured fixture");
    let decision = classify(
        &runtime(root.path().to_str().expect("utf-8 root")),
        "jq .name policy.json",
    );
    assert_eq!(decision["decision"], "allow");
    assert_eq!(
        decision["fields"]["configRuleId"],
        "allow-bounded-json-projection"
    );
}

#[test]
fn raw_structured_shell_read_is_denied_by_action_and_path_rule() {
    let root = tempfile::tempdir().expect("temporary workspace");
    std::fs::write(root.path().join("policy.json"), b"{\"name\":\"asp\"}")
        .expect("structured fixture");
    let decision = classify(
        &runtime(root.path().to_str().expect("utf-8 root")),
        "unknown-consumer < policy.json",
    );
    assert_eq!(decision["decision"], "deny", "decision={decision:#}");
    assert_eq!(
        decision["fields"]["configRuleId"],
        "route-shell-structured-document-read"
    );
    assert_eq!(decision["reasonKind"], "structured-source-read");
    assert!(capabilities(&decision).iter().any(|capability| {
        capability["action"] == "read" && capability["evidence"] == "shell-redirection"
    }));
}

#[test]
fn emitted_action_ir_conforms_to_the_v1_schema() {
    let decision = classify(&runtime("."), "unknown-consumer Cargo.lock");
    let schema: Value =
        serde_json::from_str(include_str!("../../../schemas/agent-action.v1.schema.json"))
            .expect("parse Agent Action v1 schema");
    let validator = jsonschema::validator_for(&schema).expect("compile Agent Action v1 schema");
    let action = &decision["fields"]["agentAction"];
    let errors = validator
        .iter_errors(action)
        .map(|error| error.to_string())
        .collect::<Vec<_>>();
    assert!(errors.is_empty(), "action={action:#} errors={errors:#?}");
}

#[test]
fn registered_search_denial_emits_role_receipt_without_legacy_target_identity() {
    let decision = classify(
        &runtime("."),
        "asp typescript search lexical projectRoot owner tests .",
    );
    assert_eq!(decision["decision"], "deny");
    assert_eq!(
        decision["fields"]["configRuleId"],
        "registered-asp-reasoning-search"
    );
    assert_eq!(
        decision["fields"]["agentSessionAction"],
        "dispatch-registered-agent"
    );
    assert_eq!(decision["fields"]["transport"], "host-agent");
    assert_eq!(
        decision["fields"]["targetAgent"].as_str(),
        Some("asp_explorer")
    );
    assert_eq!(
        decision["fields"]["targetAgentSymbol"].as_str(),
        Some("@asp_explorer")
    );
    assert!(
        decision["fields"]["receiptKind"]
            .as_str()
            .is_some_and(|value| !value.is_empty())
    );
    for legacy in ["residentName", "residentChildName", "targetAgentName"] {
        assert!(
            decision["fields"].get(legacy).is_none(),
            "legacy field={legacy}"
        );
    }
}

#[test]
fn agent_search_json_denial_is_owned_by_the_declared_rule() {
    let decision = classify(
        &runtime("."),
        "asp typescript search lexical projectRoot owner tests --json .",
    );
    assert_eq!(decision["decision"], "deny");
    assert_eq!(decision["fields"]["configRuleId"], "deny-agent-search-json");
}

#[test]
fn bash_source_access_plus_language_profile_does_not_depend_on_executable_names() {
    for command in [
        "just --list | rg hook",
        "rg hook",
        "rg HookDecision Cargo.lock",
    ] {
        let decision = classify(&runtime("."), command);
        assert_ne!(decision["decision"], "deny", "command={command}");
    }

    let registered_source_read = classify(
        &runtime("."),
        "future-unknown-consumer < crates/agent-semantic-hook/src/protocol.rs",
    );
    assert_eq!(registered_source_read["decision"], "deny");
    assert_eq!(
        registered_source_read["fields"]["configRuleId"],
        "route-unresolved-source-access-to-asp-languages"
    );
    assert!(
        capabilities(&registered_source_read)
            .iter()
            .any(|capability| {
                capability["action"] == "read" && capability["evidence"] == "shell-redirection"
            })
    );
}

#[test]
fn codex_post_tool_policy_denial_materializes_as_observational_json() {
    let config = managed_hook_config();
    let mut runtime = runtime_with_rust_policy_projection(".");
    config
        .apply_language_provider_projection(&mut runtime)
        .expect("apply canonical managed language/provider projection");
    let decision = classify_codex_plugin_scenario(
        &runtime,
        &config,
        "pre-tool",
        &shell("unknown-consumer < crates/agent-semantic-hook/src/protocol.rs"),
        Some("Bash"),
        None,
    )
    .expect("classify Codex PostToolUse source-read policy");
    assert_eq!(decision["decision"], "deny", "decision={decision:#}");

    let mut decision: agent_semantic_hook::HookDecision =
        serde_json::from_value(decision).expect("decode typed Hook decision");
    decision.event = "post-tool".to_owned();
    let rendered = agent_semantic_hook::render_platform_response(&decision)
        .expect("render Codex PostToolUse output");
    let hook_output = rendered
        .get("hookSpecificOutput")
        .and_then(Value::as_object)
        .expect("PostToolUse output object");

    assert_eq!(
        hook_output.get("hookEventName").and_then(Value::as_str),
        Some("PostToolUse")
    );
    assert!(hook_output.get("permissionDecision").is_none());
    assert!(hook_output.get("permissionDecisionReason").is_none());
    assert!(
        hook_output
            .get("additionalContext")
            .and_then(Value::as_str)
            .is_some_and(|context| context.starts_with("[agent-hook-decision] "))
    );
}

#[test]
fn verified_explorer_search_is_authorized_once_and_post_tool_remains_observational() {
    let config = managed_hook_config();
    let runtime = runtime(".");
    let payloads = [json!({
        "is_subagent": true,
        "agent_id": "child-session",
        "agent_type": "asp_explorer",
        "registration_verified": true,
        "registered_agent_name": "asp_explorer",
        "registered_denied_actions": ["edit"],
        "registered_allowed_rule_intents": ["reasoning-search", "structured-projection"],
        "tool_name": "Bash",
        "tool_input": {
            "command": "rtk --ultra-compact err asp rust search pipe 'HookDecision' --workspace . --view seeds"
        }
    })];

    for payload in &payloads {
        let pre_tool = classify_codex_plugin_scenario(
            &runtime,
            &config,
            "pre-tool",
            payload,
            Some("Bash"),
            None,
        )
        .expect("classify registered Explorer search authorization");
        assert_eq!(pre_tool["decision"], "allow", "decision={pre_tool:#}");
        assert_eq!(
            pre_tool["fields"]["dispatchAdmission"], "verified-registration-intent",
            "decision={pre_tool:#}"
        );

        let post_tool = classify_hook_scenario(&runtime, &config, "codex", "post-tool", payload)
            .expect("classify registered Explorer observational output");
        assert_eq!(post_tool["decision"], "allow", "decision={post_tool:#}");
        assert_eq!(post_tool["reasonKind"], "none", "decision={post_tool:#}");
        assert!(
            post_tool["fields"].get("attemptedRuleIntent").is_none(),
            "PostToolUse must not be relabeled as an unscoped command: {post_tool:#}"
        );
    }
}

#[test]
fn registered_read_only_native_edit_is_physically_denied_with_agent_message() {
    for agent_name in ["asp_explorer", "asp_testing"] {
        let payload = registered_read_only_action(
            agent_name,
            "apply_patch",
            json!({
                "command": "*** Begin Patch\n*** Update File: src/lib.rs\n@@\n-old\n+new\n*** End Patch"
            }),
        );
        let decision = classify_codex_plugin_scenario(
            &runtime("."),
            &ClientHookConfig::default(),
            "pre-tool",
            &payload,
            Some("apply_patch"),
            None,
        )
        .expect("classify registered canonical apply_patch scenario");

        assert_eq!(decision["decision"], "deny", "agent={agent_name}");
        assert_eq!(
            decision["reasonKind"], "read-only-subagent-write",
            "agent={agent_name}"
        );
        assert_eq!(
            decision["message"],
            format!("编辑模式必须在主线程完成，当前的 {agent_name} 不允许执行 edit action。")
        );
        assert_eq!(decision["fields"]["permissionAction"], "edit");
    }
}

#[test]
fn registered_read_only_shell_behavior_edit_uses_the_same_dominance_gate() {
    let payload = registered_read_only_action(
        "asp_explorer",
        "Bash",
        json!({"command": "unknown-producer > generated.rs"}),
    );
    let decision = classify_codex_plugin_scenario(
        &runtime("."),
        &ClientHookConfig::default(),
        "pre-tool",
        &payload,
        Some("Bash"),
        None,
    )
    .expect("classify registered shell Edit scenario");

    assert_eq!(decision["decision"], "deny");
    assert_eq!(decision["reasonKind"], "read-only-subagent-write");
    assert!(capabilities(&decision).iter().any(|capability| {
        capability["action"] == "edit" && capability["evidence"] == "shell-redirection"
    }));
}

#[test]
fn registered_agent_profiles_admit_only_their_declared_rule_intents() {
    let cases = [
        (
            "asp_explorer",
            "asp typescript search lexical projectRoot owner tests .",
            "allow",
            "reasoning-search",
        ),
        (
            "asp_explorer",
            "asp rust query --selector rust://crates/example.rs#item/function/example --workspace . --projection source",
            "allow",
            "structured-projection",
        ),
        (
            "asp_explorer",
            "cargo test -p agent-semantic-hook",
            "deny",
            "test-build-command",
        ),
        (
            "asp_testing",
            "cargo test -p agent-semantic-hook",
            "allow",
            "test-build-command",
        ),
        (
            "asp_testing",
            "asp typescript search lexical projectRoot owner tests .",
            "deny",
            "reasoning-search",
        ),
    ];

    for (agent_name, command, expected_decision, intent) in cases {
        let payload = registered_read_only_action(agent_name, "Bash", json!({"command": command}));
        let decision = classify_codex_plugin_scenario(
            &runtime("."),
            &ClientHookConfig::default(),
            "pre-tool",
            &payload,
            Some("Bash"),
            None,
        )
        .expect("classify registered Agent scope scenario");

        assert_eq!(
            decision["decision"], expected_decision,
            "agent={agent_name} command={command} decision={decision:#}"
        );
        assert_eq!(decision["fields"]["intent"], intent);
        if expected_decision == "deny" {
            assert_eq!(decision["reasonKind"], "subagent-capability-denied");
            assert_eq!(
                decision["fields"]["capabilityAdmission"],
                "agent-profile-rule-intent-scope"
            );
        } else {
            assert_eq!(decision["fields"]["dispatchSatisfied"], true);
        }
    }
}

#[test]
fn registered_testing_profile_denies_unscoped_execution() {
    let payload = registered_read_only_action(
        "asp_testing",
        "Bash",
        json!({"command": "printf not-a-test"}),
    );
    let decision = classify_codex_plugin_scenario(
        &runtime("."),
        &ClientHookConfig::default(),
        "pre-tool",
        &payload,
        Some("Bash"),
        None,
    )
    .expect("classify unscoped testing Agent command");

    assert_eq!(decision["decision"], "deny");
    assert_eq!(decision["reasonKind"], "subagent-capability-denied");
    assert_eq!(
        decision["fields"]["attemptedRuleIntent"],
        "unscoped-command"
    );
}

#[test]
fn registered_source_root_and_parser_read_behavior_compose_into_the_source_search_rule() {
    let config = managed_hook_config();
    let mut runtime = runtime_with_rust_policy_projection(".");
    config
        .apply_language_provider_projection(&mut runtime)
        .expect("apply canonical managed language/provider projection");
    assert!(runtime.policy_providers.iter().any(|provider| {
        provider.language_id.as_str() == "rust"
            && provider.package_roots.iter().any(|root| root == "crates")
    }));
    let decision = classify_codex_plugin_scenario(
        &runtime,
        &config,
        "pre-tool",
        &shell("unknown-consumer < crates/agent-semantic-runtime/src/lib.rs"),
        Some("Bash"),
        None,
    )
    .expect("classify managed source-root parser-read scenario");
    assert_eq!(decision["decision"], "deny", "decision={decision:#}");
    assert_eq!(
        decision["fields"]["configRuleId"],
        "route-unresolved-source-access-to-asp-languages"
    );
    assert!(
        decision["routes"]
            .as_array()
            .is_some_and(|routes| !routes.is_empty())
    );
}

#[test]
fn recipe_name_filter_does_not_invent_a_registered_source_subject() {
    let decision = classify(&runtime("."), "just --list | rg hook");
    assert_ne!(
        decision["fields"]["configRuleId"],
        "deny-uncontrolled-source-search-commands"
    );
    assert_ne!(decision["reasonKind"], "raw-broad-search");
}

#[test]
fn registered_source_root_without_read_behavior_does_not_trigger_source_search() {
    let decision = classify(
        &runtime("."),
        "unknown-consumer crates/agent-semantic-runtime",
    );
    assert_ne!(
        decision["fields"]["configRuleId"],
        "route-unresolved-source-access-to-asp-languages"
    );
    assert!(
        capabilities(&decision)
            .iter()
            .all(|capability| capability["action"] != "read")
    );
}

#[test]
fn registered_testing_profile_admits_wrapped_live_corpus_qualification() {
    let payload = registered_read_only_action(
        "asp_testing",
        "Bash",
        json!({"command": "rtk --ultra-compact err asp live-corpus qualify"}),
    );
    let decision = classify_codex_plugin_scenario(
        &runtime("."),
        &ClientHookConfig::default(),
        "pre-tool",
        &payload,
        Some("Bash"),
        None,
    )
    .expect("classify wrapped Live Corpus qualification");

    assert_eq!(decision["decision"], "allow");
    assert_eq!(decision["fields"]["intent"], "live-corpus-qualification");
    assert_eq!(decision["fields"]["dispatchSatisfied"], true);
    assert_eq!(
        decision["fields"]["dispatchAdmission"],
        "verified-registration-intent"
    );
}
