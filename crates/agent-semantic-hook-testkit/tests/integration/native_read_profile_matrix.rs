// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

use agent_semantic_hook::ClientHookConfig;
use agent_semantic_hook::HookRuntime;
use agent_semantic_hook_testkit::classify_codex_plugin_scenario;
use serde_json::Value;
use serde_json::json;

fn empty_runtime() -> HookRuntime {
    HookRuntime {
        project_root: "/workspace".to_string(),
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
fn shell_registered_source_reads_fail_closed_through_provider_profiles() {
    for (language_id, command) in [
        ("rust", "future-source-consumer < src/lib.rs"),
        ("typescript", "future-source-consumer < src/index.ts"),
        ("python", "future-source-consumer < src/main.py"),
        ("julia", "future-source-consumer < src/main.jl"),
        ("gerbil-scheme", "future-source-consumer < src/main.ss"),
        ("org", "future-source-consumer < docs/plan.org"),
        ("md", "future-source-consumer < README.md"),
    ] {
        let decision = classify_codex_plugin_scenario(
            &empty_runtime(),
            &ClientHookConfig::default(),
            "pre-tool",
            &shell_read(command),
            "Bash",
        )
        .expect("classify shell document read");
        assert_eq!(decision["decision"], "deny", "language={language_id}");
        assert_eq!(
            decision["fields"]["agentAction"]["hostInvocation"]["action"], "execute",
            "the physical Bash matcher owns the Host action"
        );
        assert_eq!(
            decision["fields"]["configRuleId"],
            match language_id {
                "org" => "route-org-document-read-to-asp-explorer",
                "md" => "route-markdown-document-read-to-asp-explorer",
                _ => "route-read-to-asp-languages",
            },
            "language={language_id}"
        );
        assert_eq!(decision["languageIds"][0], language_id);
    }
}

#[test]
fn document_reads_route_without_fabricating_a_lazy_runtime_provider() {
    for (language_id, command) in [
        ("org", "future-source-consumer < docs/plan.org"),
        ("md", "future-source-consumer < README.md"),
    ] {
        let decision = classify_codex_plugin_scenario(
            &empty_runtime(),
            &ClientHookConfig::default(),
            "pre-tool",
            &shell_read(command),
            "Bash",
        )
        .expect("classify document read without a lazy Runtime provider");
        assert_eq!(decision["decision"], "deny", "command={command}");
        assert_eq!(
            decision["fields"]["configRuleId"],
            match language_id {
                "org" => "route-org-document-read-to-asp-explorer",
                "md" => "route-markdown-document-read-to-asp-explorer",
                _ => unreachable!("document matrix has only org and md"),
            }
        );
        assert_eq!(decision["languageIds"][0], language_id);
    }
}

#[test]
fn unresolved_registered_source_operands_remain_allow_without_fabricating_read_permission() {
    for (language_id, command) in [
        ("rust", "future-source-consumer src/app.rs"),
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
            "Bash",
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
    {
        let command = "git status --short";
        let decision = classify_codex_plugin_scenario(
            &empty_runtime(),
            &ClientHookConfig::default(),
            "pre-tool",
            &shell_read(command),
            "Bash",
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
fn shell_search_without_a_registered_source_operand_routes_to_search_playbook() {
    let decision = classify_codex_plugin_scenario(
        &empty_runtime(),
        &ClientHookConfig::default(),
        "pre-tool",
        &shell_read("just --list | rg 'hook'"),
        "Bash",
    )
    .expect("classify shell search");
    assert_eq!(decision["decision"], "deny");
    assert_eq!(
        decision["fields"]["configRuleId"],
        "deny-shell-search-before-execution"
    );
}

#[test]
fn registered_source_output_is_an_edit_action_not_a_guessed_read_action() {
    let decision = classify_codex_plugin_scenario(
        &empty_runtime(),
        &ClientHookConfig::default(),
        "pre-tool",
        &shell_read("future-source-producer > generated.rs"),
        "Bash",
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
