use agent_semantic_hook::{ClientHookConfig, HookRuntime};
use agent_semantic_hook_testkit::classify_codex_plugin_scenario;
use serde_json::{Value, json};

fn empty_runtime() -> HookRuntime {
    HookRuntime {
        project_root: "/workspace".to_string(),
        rankers: Vec::new(),
        providers: Vec::new(),
        policy_providers: Vec::new(),
    }
}

fn shell_read(command: &str) -> Value {
    json!({
        "tool_name": "Bash",
        "tool_input": { "command": command }
    })
}

#[test]
fn shell_org_and_markdown_reads_fail_closed_through_registered_profiles() {
    for (language_id, command) in [
        ("org", "future-source-consumer < docs/plan.org"),
        ("md", "future-source-consumer < README.md"),
    ] {
        let decision = classify_codex_plugin_scenario(
            &empty_runtime(),
            &ClientHookConfig::default(),
            "pre-tool",
            &shell_read(command),
            Some("Bash"),
            None,
        )
        .expect("classify shell document read");
        assert_eq!(decision["decision"], "deny", "language={language_id}");
        assert_eq!(
            decision["fields"]["agentAction"]["hostInvocation"]["action"], "execute",
            "the physical Bash matcher owns the Host action"
        );
        assert_eq!(
            decision["fields"]["configRuleId"], "route-read-to-asp-languages",
            "language={language_id}"
        );
        assert_eq!(decision["languageIds"][0], language_id);
    }
}

#[test]
fn unresolved_registered_source_operands_remain_allow_without_fabricating_read_permission() {
    for (language_id, command) in [
        (
            "rust",
            "head -n 1 crates/agent-semantic-hook/src/action_ir/subject.rs",
        ),
        ("typescript", "future-source-consumer src/app.ts"),
        ("python", "future-source-consumer src/app.py"),
        ("julia", "future-source-consumer src/app.jl"),
        ("gerbil-scheme", "future-source-consumer src/runtime.ss"),
        ("org", "another-tool docs/10-19-rfcs/10.05-example.org"),
        ("md", "third-party-reader README.md"),
    ] {
        let decision = classify_codex_plugin_scenario(
            &empty_runtime(),
            &ClientHookConfig::default(),
            "pre-tool",
            &shell_read(command),
            Some("Bash"),
            None,
        )
        .expect("classify an Execute action with an unproven source operand");
        assert_eq!(decision["decision"], "allow", "language={language_id}");
        assert_eq!(
            decision["fields"]["agentAction"]["hostInvocation"]["action"], "execute",
            "shell semantics must not rewrite the physical Bash Host action"
        );
        assert!(decision["fields"]["configRuleId"].is_null());
        assert_eq!(decision["languageIds"][0], language_id);
        assert!(
            decision["fields"]["agentAction"]["semanticCapabilities"]
                .as_array()
                .is_some_and(|capabilities| capabilities.iter().any(|capability| {
                    capability["action"] == "unknown"
                        && capability["evidence"] == "registered-source-operand"
                }))
        );
        assert!(
            decision["fields"]["agentAction"]["filesystemPermissions"]
                .as_array()
                .is_some_and(Vec::is_empty)
        );
    }
}

#[test]
fn metadata_command_without_a_registered_source_operand_remains_execute_only() {
    for command in ["git status --short", "just --list | rg 'hook'"] {
        let decision = classify_codex_plugin_scenario(
            &empty_runtime(),
            &ClientHookConfig::default(),
            "pre-tool",
            &shell_read(command),
            Some("Bash"),
            None,
        )
        .expect("classify metadata command without a registered source operand");

        assert_eq!(decision["decision"], "allow", "command={command}");
        assert_eq!(
            decision["fields"]["agentAction"]["hostInvocation"]["action"],
            "execute"
        );
        assert!(decision["fields"]["configRuleId"].is_null());
        assert_eq!(
            decision["fields"]["agentAction"]["semanticCapabilities"][0]["action"],
            "execute"
        );
    }
}

#[test]
fn registered_source_output_is_an_edit_action_not_a_guessed_read_action() {
    let decision = classify_codex_plugin_scenario(
        &empty_runtime(),
        &ClientHookConfig::default(),
        "pre-tool",
        &shell_read("future-source-producer > generated.rs"),
        Some("Bash"),
        None,
    )
    .expect("classify registered source output permission");

    assert_eq!(decision["decision"], "allow");
    assert_eq!(
        decision["fields"]["agentAction"]["hostInvocation"]["action"],
        "execute"
    );
    assert!(
        decision["fields"]["agentAction"]["filesystemPermissions"]
            .as_array()
            .is_some_and(|permissions| permissions.iter().any(|permission| {
                permission["permission"] == "write"
                    && permission["source"] == "shell-redirection"
                    && permission["subject"] == "generated.rs"
            }))
    );
    assert!(
        !decision["fields"]["agentAction"]["semanticCapabilities"]
            .as_array()
            .is_some_and(|capabilities| capabilities
                .iter()
                .any(|capability| capability["action"] == "read"))
    );
}
