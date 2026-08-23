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
fn opaque_shell_source_access_is_governed_without_command_effect_guessing() {
    let decision = classify(&runtime("."), "unknown-consumer src/lib.rs");
    assert_eq!(decision["decision"], "deny");
    assert!(capabilities(&decision).iter().any(|capability| {
        capability["action"] == "execute" && capability["evidence"] == "host-invocation"
    }));
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
