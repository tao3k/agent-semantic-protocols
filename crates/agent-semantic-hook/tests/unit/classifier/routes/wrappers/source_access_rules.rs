use agent_semantic_hook::{DecisionKind, ReasonKind, classify_hook};
use serde_json::json;

use crate::classifier::registry;

#[test]
fn wrapper_source_access_rule_routes_to_provider_query_when_supported() {
    for command in [
        "DIRENV_SILENCE=1 direnv exec . rg -n WorkflowExecution src/lib.rs",
        "direnv exec . rg -n WorkflowExecution src/lib.rs",
        "rtk proxy rg -n WorkflowExecution src/lib.rs",
        "rtk run -c \"rg -n WorkflowExecution src/lib.rs\"",
        "bash -lc \"rg -n WorkflowExecution src/lib.rs\"",
        "zsh -lc \"rg -n WorkflowExecution src/lib.rs\"",
    ] {
        let decision = classify_hook(
            &registry(),
            "codex",
            "pre-tool",
            &json!({ "tool_name": "functions.exec_command", "tool_input": { "cmd": command } }),
        );
        assert_eq!(decision.decision, DecisionKind::Deny, "{command}");
        assert_eq!(
            decision.reason_kind,
            ReasonKind::RawBroadSearch,
            "{command}"
        );
    }
}

#[test]
fn generic_wrapper_testing_resident_dispatch_matches_git_snapshot() {
    let scenario = toml::from_str::<toml::Value>(include_str!(
        "../../../../fixtures/scenarios/generic_wrapper_testing_resident_dispatch/scenario.toml"
    ))
    .expect("hook match scenario snapshot");
    let scenario_schema = serde_json::from_str::<serde_json::Value>(include_str!(
        "../../../../../../../schemas/semantic-hook-match-scenario.v1.schema.json"
    ))
    .expect("hook match scenario schema");
    let scenario_json = serde_json::to_value(&scenario).expect("scenario JSON projection");
    assert!(
        jsonschema::validator_for(&scenario_schema)
            .expect("hook match scenario validator")
            .is_valid(&scenario_json),
        "{scenario_json}"
    );
    let expected = &scenario["expected"];
    for command in scenario["commands"]
        .as_array()
        .expect("scenario commands")
        .iter()
        .map(|command| command.as_str().expect("scenario command"))
    {
        let decision = classify_hook(
            &crate::classifier::registry_without_providers(),
            "codex",
            "pre-tool",
            &json!({ "tool_name": "functions.exec_command", "tool_input": { "cmd": command } }),
        );

        assert_eq!(decision.decision, DecisionKind::Deny, "{command}");
        assert_eq!(
            decision.reason_kind,
            ReasonKind::SubagentReceiptRequired,
            "{command}"
        );
        assert_eq!(
            decision
                .fields
                .get("configRuleId")
                .and_then(serde_json::Value::as_str),
            expected["ruleId"].as_str(),
            "{command}"
        );
        assert_eq!(
            decision.fields["choicePlaneOwner"].as_str(),
            Some("org-contract:agent-interactive"),
            "{command}"
        );
        assert_eq!(
            decision.fields["agentWindowCommand"].as_str(),
            Some("asp session --agents choice-plane"),
            "{command}"
        );
        for forbidden in ["residentName", "targetAgentName", "receiptKind"] {
            assert!(
                !decision.fields.contains_key(forbidden),
                "{command}: {forbidden}"
            );
        }
        assert!(
            !decision.message.contains("asp session @"),
            "classifier must not preselect a ChoicePlane role: {command}"
        );
    }
}
