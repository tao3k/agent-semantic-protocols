use super::{
    ClientHookConfig, DecisionKind, HookClassificationRequest, classify_hook_with_config, json,
};

#[test]
fn builtin_inline_materialization_rules_use_config_and_source_paths() {
    let config = ClientHookConfig::default();
    let registry = crate::classifier::rust_registry();

    for (command, expected_rule) in [
        (
            "python -c \"open('crates/agent-semantic-hook/src/hook_config/core/implementation.rs').read()\"",
            "materialize-source-access-policy",
        ),
        (
            "node -e \"require('fs').readFileSync('crates/agent-semantic-hook/src/hook_config/core/implementation.rs')\"",
            "materialize-source-access-policy",
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
    let mut registry = crate::classifier::builtin_programming_runtime();
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
            "resident TypeScript build and node test dispatch",
            json!({
                "tool_name": "Bash",
                "tool_input": {
                    "command": "npm run build && node --test dist/tests/unit/cli_semantic_search.test.js"
                }
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
                "command": "asp-typescript search lexical HookDecision owner tests --json ."
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
            "route-read-to-asp-languages",
        ),
        (
            "native registered source pattern read",
            json!({
                "tool_name": "Read",
                "tool_input": {"path": "*.rs"}
            }),
            DecisionKind::Deny,
            "route-read-to-asp-languages",
        ),
        (
            "native Python source pattern read",
            json!({
                "tool_name": "Read",
                "tool_input": {"path": "*.py"}
            }),
            DecisionKind::Deny,
            "route-read-to-asp-languages",
        ),
        (
            "native TypeScript source pattern read",
            json!({
                "tool_name": "Read",
                "tool_input": {"path": "*.ts"}
            }),
            DecisionKind::Deny,
            "route-read-to-asp-languages",
        ),
        (
            "native Julia source pattern read",
            json!({
                "tool_name": "Read",
                "tool_input": {"path": "*.jl"}
            }),
            DecisionKind::Deny,
            "route-read-to-asp-languages",
        ),
        (
            "native Gerbil source pattern read",
            json!({
                "tool_name": "Read",
                "tool_input": {"path": "*.ss"}
            }),
            DecisionKind::Deny,
            "route-read-to-asp-languages",
        ),
        (
            "native structured document read",
            json!({
                "tool_name": "Read",
                "tool_input": {"file_path": json_document}
            }),
            DecisionKind::Deny,
            "materialize-structured-document-read-action",
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
            "route-read-to-asp-languages",
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
            "materialize-source-access-policy",
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
            "materialize-source-access-policy",
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
        (
            "git metadata read",
            json!({
                "tool_name": "Bash",
                "tool_input": {"command": "git diff --stat"}
            }),
            DecisionKind::Deny,
            "deny-uncontrolled-git-metadata-reads",
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
        if label == "native structured document read" {
            assert_eq!(
                decision
                    .fields
                    .get("targetAgentRole")
                    .and_then(serde_json::Value::as_str),
                Some("asp_explorer"),
                "{label}: native JSON/TOML Read must dispatch to the registered Explorer: {decision:?}"
            );
            assert!(
                decision.message.contains("jq for JSON and yq for TOML"),
                "{label}: bounded structured-reader guidance is missing: {decision:?}"
            );
        }
    }
}
