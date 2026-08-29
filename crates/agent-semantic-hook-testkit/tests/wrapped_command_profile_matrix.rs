use agent_semantic_hook::{ClientHookConfig, HookRuntime};
use agent_semantic_hook_testkit::classify_hook_scenario;
use serde_json::{Value, json};

fn empty_runtime() -> HookRuntime {
    HookRuntime {
        project_root: "/workspace".to_string(),
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

fn gerbil_build_scenario() -> toml::Value {
    let matrix = toml::from_str::<toml::Value>(include_str!(
        "../../agent-semantic-config/templates/hooks/config-test.toml"
    ))
    .expect("Hook scenario matrix");
    matrix["rules"]
        .as_array()
        .and_then(|rules| {
            rules
                .iter()
                .find(|rule| rule["id"].as_str() == Some("gerbil-build-role-dispatch"))
        })
        .cloned()
        .expect("Gerbil build scenario")
}

#[test]
fn wrapped_gxi_ss_build_routes_to_the_testing_lane() {
    let scenario = gerbil_build_scenario();
    let command = scenario["positiveCommands"]
        .as_array()
        .and_then(|commands| commands.first())
        .and_then(toml::Value::as_str)
        .expect("Gerbil positive command");
    let decision = classify_hook_scenario(
        &empty_runtime(),
        &ClientHookConfig::default(),
        "codex",
        "pre-tool",
        &shell(command),
    )
    .expect("classify wrapped Gerbil build");

    assert_eq!(decision["decision"], "deny");
    assert_eq!(
        decision["fields"]["configRuleId"],
        "gerbil-build-role-dispatch"
    );
    assert_eq!(decision["fields"]["targetAgent"], "asp_testing");
    assert_eq!(decision["fields"]["targetAgentSymbol"], "@asp_testing");
    assert_eq!(decision["fields"]["blockedCommandClass"], "building");
}

#[test]
fn wrapped_gxi_profile_requires_an_ss_operand() {
    let scenario = gerbil_build_scenario();
    for command in scenario["negativeCommands"]
        .as_array()
        .expect("Gerbil negative commands")
        .iter()
        .map(|command| command.as_str().expect("Gerbil negative command"))
    {
        let decision = classify_hook_scenario(
            &empty_runtime(),
            &ClientHookConfig::default(),
            "codex",
            "pre-tool",
            &shell(command),
        )
        .expect("classify non-matching Gerbil command");
        assert_ne!(
            decision["fields"]["configRuleId"], "gerbil-build-role-dispatch",
            "command={command}"
        );
    }
}

#[test]
fn process_bound_no_agent_assignment_bypasses_all_hook_policy() {
    let config = ClientHookConfig::default();
    for command in [
        "ASP_NO_AGENT=1 cargo test -p agent-semantic-hook",
        "/usr/bin/env ASP_NO_AGENT=1 asp rust search pipe owner",
        "export ASP_NO_AGENT=1; asp rust search pipe owner",
        "export ASP_NO_AGENT=1; exec asp rust query --selector rust://owner",
    ] {
        let decision = classify_hook_scenario(
            &empty_runtime(),
            &config,
            "codex",
            "pre-tool",
            &shell(command),
        )
        .expect("classify explicit no-Agent escape");

        assert_eq!(decision["decision"], "allow", "command={command}");
        assert_eq!(
            decision["fields"]["configRuleId"], "allow-explicit-no-agent",
            "command={command}"
        );
        assert_eq!(
            decision["fields"]["bypassScope"], "host-policy",
            "command={command}"
        );
    }
}

#[test]
fn text_or_late_assignment_cannot_claim_no_agent_authority() {
    let config = ClientHookConfig::default();
    for command in [
        "printf 'ASP_NO_AGENT=1 cargo test'",
        "printf warmup; ASP_NO_AGENT=1 cargo test -p agent-semantic-hook",
    ] {
        let decision = classify_hook_scenario(
            &empty_runtime(),
            &config,
            "codex",
            "pre-tool",
            &shell(command),
        )
        .expect("classify non-authoritative no-Agent text");
        assert_ne!(
            decision["fields"]["configRuleId"], "allow-explicit-no-agent",
            "command={command}"
        );
    }
}
