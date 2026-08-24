use agent_semantic_hook::{ClientHookConfig, HookRuntime};
use agent_semantic_hook_testkit::classify_hook_scenario;
use serde_json::{Value, json};

fn runtime(project_root: &str) -> HookRuntime {
    HookRuntime {
        project_root: project_root.to_owned(),
        rankers: Vec::new(),
        providers: Vec::new(),
        policy_providers: Vec::new(),
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
        "asp_explorer" => (json!(["explore", "subagent"]), json!(["reasoning-search"])),
        "asp_testing" => (
            json!(["build", "subagent", "testing"]),
            json!(["test-build-command"]),
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
    classify_hook_scenario(
        runtime,
        &ClientHookConfig::default(),
        "codex",
        "pre-tool",
        &shell(command),
    )
    .expect("classify shell scenario")
}

fn capabilities(decision: &Value) -> Vec<Value> {
    decision["fields"]["agentAction"]["semanticCapabilities"]
        .as_array()
        .cloned()
        .unwrap_or_default()
}

#[test]
fn shell_path_operands_project_read_without_command_name_guessing() {
    let decision = classify(&runtime("."), "unknown-consumer src/lib.rs");
    assert_eq!(decision["decision"], "deny");
    assert!(capabilities(&decision).iter().any(|capability| {
        capability["action"] == "execute" && capability["evidence"] == "host-invocation"
    }));
    assert!(capabilities(&decision).iter().any(|capability| {
        capability["action"] == "read" && capability["evidence"] == "shell-path-operand"
    }));
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
        "unknown-consumer policy.json",
    );
    assert_eq!(decision["decision"], "deny");
    assert_eq!(
        decision["fields"]["configRuleId"],
        "route-shell-structured-document-read"
    );
    assert_eq!(decision["reasonKind"], "structured-source-read");
    assert!(capabilities(&decision).iter().any(|capability| {
        capability["action"] == "read" && capability["evidence"] == "shell-path-operand"
    }));
}

#[test]
fn emitted_action_ir_conforms_to_the_v1_schema() {
    let decision = classify(&runtime("."), "unknown-consumer src/lib.rs");
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
        "dispatch-choice-plane-role"
    );
    assert_eq!(decision["fields"]["transport"], "host-agent");
    assert!(
        decision["fields"]["targetAgentRole"]
            .as_str()
            .is_some_and(|value| !value.is_empty())
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
fn registered_read_only_native_edit_is_physically_denied_with_agent_message() {
    for agent_name in ["asp_explorer", "asp_testing"] {
        let payload = registered_read_only_action(
            agent_name,
            "Edit",
            json!({
                "file_path": "src/lib.rs",
                "old_string": "old",
                "new_string": "new"
            }),
        );
        let decision = classify_hook_scenario(
            &runtime("."),
            &ClientHookConfig::default(),
            "codex",
            "pre-tool",
            &payload,
        )
        .expect("classify registered native Edit scenario");

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
    let decision = classify_hook_scenario(
        &runtime("."),
        &ClientHookConfig::default(),
        "codex",
        "pre-tool",
        &payload,
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
        let decision = classify_hook_scenario(
            &runtime("."),
            &ClientHookConfig::default(),
            "codex",
            "pre-tool",
            &payload,
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
    let decision = classify_hook_scenario(
        &runtime("."),
        &ClientHookConfig::default(),
        "codex",
        "pre-tool",
        &payload,
    )
    .expect("classify unscoped testing Agent command");

    assert_eq!(decision["decision"], "deny");
    assert_eq!(decision["reasonKind"], "subagent-capability-denied");
    assert_eq!(
        decision["fields"]["attemptedRuleIntent"],
        "unscoped-command"
    );
}
