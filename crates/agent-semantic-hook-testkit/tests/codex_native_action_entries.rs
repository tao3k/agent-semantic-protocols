use agent_semantic_hook::{ClientHookConfig, HookRuntime};
use agent_semantic_hook_testkit::{classify_codex_plugin_scenario, classify_hook_scenario};
use serde_json::Value;
use serde_json::json;

const CODEX_PLUGIN_HOOKS: &str = include_str!("../../../asp-codex-plugin/hooks/hooks.json");

#[test]
fn codex_pre_tool_entries_keep_host_identity_separate_from_action_dsl() {
    let document: Value = serde_json::from_str(CODEX_PLUGIN_HOOKS).expect("valid hooks.json");
    let entries = document["hooks"]["PreToolUse"]
        .as_array()
        .expect("PreToolUse entries");
    let matchers = entries
        .iter()
        .map(|entry| entry["matcher"].as_str().expect("native matcher"))
        .collect::<Vec<_>>();

    assert_eq!(
        matchers,
        vec![
            "Read",
            "apply_patch",
            "Write",
            "Edit",
            "NotebookEdit",
            "Bash",
            "spawn_agent",
            "^mcp__.*$",
        ]
    );
    assert!(!matchers.contains(&"*"));
    assert!(!matchers.contains(&"Agent"));

    for (entry, expected_signal) in entries.iter().zip([
        "--host-match Read",
        "--host-match apply_patch",
        "--host-match Write",
        "--host-match Edit",
        "--host-match NotebookEdit",
        "--host-match Bash",
        "--host-match spawn_agent",
        "--host-match-prefix mcp__",
    ]) {
        let handlers = entry["hooks"].as_array().expect("one action handler");
        assert_eq!(handlers.len(), 1);
        assert_eq!(handlers[0]["type"].as_str(), Some("command"));
        assert_eq!(handlers[0]["timeout"].as_u64(), Some(1));
        assert!(
            handlers[0]["command"]
                .as_str()
                .is_some_and(|command| command.ends_with(expected_signal)),
            "{entry}"
        );
    }
}

#[test]
fn native_host_matcher_projects_edit_aliases_into_one_edit_action() {
    let runtime = HookRuntime {
        project_root: ".".to_owned(),
        rankers: Vec::new(),
        providers: Vec::new(),
        policy_providers: Vec::new(),
    };
    let config = ClientHookConfig::default();
    let fixtures = [
        (
            "apply_patch",
            json!({"patch": "*** Begin Patch\n*** Update File: owned.rs\n@@\n-old\n+new\n*** End Patch"}),
        ),
        ("Write", json!({"file_path": "owned.rs", "content": "new"})),
        (
            "Edit",
            json!({"file_path": "owned.rs", "old_string": "old", "new_string": "new"}),
        ),
        (
            "NotebookEdit",
            json!({"notebook_path": "owned.ipynb", "new_source": "new"}),
        ),
    ];

    for (tool_name, tool_input) in fixtures {
        let decision = classify_codex_plugin_scenario(
            &runtime,
            &config,
            "pre-tool",
            &json!({"tool_name": tool_name, "tool_input": tool_input}),
            Some(tool_name),
            None,
        )
        .expect("classify native Action alias");
        assert_eq!(
            decision["fields"]["configRuleId"].as_str(),
            Some("allow-owner-scoped-mutation"),
            "{tool_name}"
        );
        assert!(
            decision["fields"]["agentAction"]["semanticCapabilities"]
                .as_array()
                .is_some_and(|capabilities| capabilities.iter().any(|capability| {
                    capability["action"] == "edit" && capability["evidence"] == "host-matcher"
                })),
            "{tool_name}"
        );
    }
}

#[test]
fn edit_alias_without_a_host_path_does_not_fabricate_edit_permission() {
    let runtime = HookRuntime {
        project_root: ".".to_owned(),
        rankers: Vec::new(),
        providers: Vec::new(),
        policy_providers: Vec::new(),
    };
    let config = ClientHookConfig::default();
    let decision = classify_codex_plugin_scenario(
        &runtime,
        &config,
        "pre-tool",
        &json!({
            "tool_name": "NotebookEdit",
            "tool_input": { "notebook_path_hint": "not-a-host-path-field.ipynb" }
        }),
        Some("NotebookEdit"),
        None,
    )
    .expect("classify pathless native Action alias");

    assert_ne!(
        decision["fields"]["configRuleId"].as_str(),
        Some("allow-owner-scoped-mutation")
    );
    assert!(
        decision["fields"]["agentAction"]["filesystemPermissions"]
            .as_array()
            .is_none_or(|permissions| permissions.iter().all(|fact| fact["permission"] != "write")),
        "{decision}"
    );
}

#[test]
fn host_matcher_is_consumed_only_by_the_bound_host_adapter() {
    let runtime = HookRuntime {
        project_root: ".".to_owned(),
        rankers: Vec::new(),
        providers: Vec::new(),
        policy_providers: Vec::new(),
    };
    let config = ClientHookConfig::default();

    let codex_read = classify_codex_plugin_scenario(
        &runtime,
        &config,
        "pre-tool",
        &json!({ "tool_name": "Read", "tool_input": { "file_path": "owned.rs" } }),
        Some("Read"),
        None,
    )
    .expect("classify Codex Read");
    assert_eq!(
        codex_read["fields"]["configRuleId"].as_str(),
        Some("route-read-to-asp-languages")
    );
    assert!(
        codex_read["fields"]["agentAction"]["semanticCapabilities"]
            .as_array()
            .is_some_and(|capabilities| capabilities.iter().any(|capability| {
                capability["action"] == "read" && capability["evidence"] == "host-matcher"
            }))
    );

    let foreign_edit = classify_hook_scenario(
        &runtime,
        &config,
        "claude",
        "pre-tool",
        &json!({ "tool_name": "Edit", "tool_input": { "file_path": "owned.rs" } }),
    )
    .expect("classify foreign native Action");
    assert!(
        foreign_edit["fields"]["agentAction"]["semanticCapabilities"]
            .as_array()
            .is_none_or(|capabilities| capabilities
                .iter()
                .all(|capability| { capability["evidence"] != "host-matcher" }))
    );
    assert_ne!(
        foreign_edit["fields"]["configRuleId"].as_str(),
        Some("allow-owner-scoped-mutation")
    );
}
