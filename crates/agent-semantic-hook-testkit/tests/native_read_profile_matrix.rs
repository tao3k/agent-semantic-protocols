use agent_semantic_hook::{
    ClientHookConfig, HookDecision, HookRuntime, direct_read_source_key,
    rebind_direct_read_decision_to_payload,
};
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

fn native_read(path: &str) -> Value {
    json!({
        "tool_name": "Read",
        "tool_input": { "file_path": path }
    })
}

fn assert_profile_read_route(
    payload: Value,
    expected_tool_name: &str,
    language_id: &str,
    provider_id: &str,
) {
    let decision = classify_codex_plugin_scenario(
        &empty_runtime(),
        &ClientHookConfig::default(),
        "pre-tool",
        &payload,
        Some("Read"),
        None,
    )
    .expect("classify native read scenario");
    assert_eq!(decision["decision"], "deny", "language={language_id}");
    assert_eq!(
        decision["fields"]["agentAction"]["hostInvocation"]["action"], "read",
        "language={language_id}"
    );
    assert_eq!(
        decision["fields"]["agentAction"]["hostInvocation"]["toolName"], expected_tool_name,
        "language={language_id}"
    );
    assert_eq!(
        decision["fields"]["agentAction"]["semanticCapabilities"][0]["action"], "read",
        "language={language_id}"
    );
    assert_eq!(
        decision["fields"]["agentAction"]["hostInvocation"]["source"],
        Value::Null,
        "Hook payload must not invent Codex ToolInvocation.source"
    );
    assert_eq!(
        decision["fields"]["configRuleId"], "route-read-to-asp-languages",
        "language={language_id}"
    );
    assert_eq!(
        decision["routes"][0]["providerId"], provider_id,
        "language={language_id}"
    );
    assert_eq!(decision["routes"][0]["kind"], "owner");
    assert_eq!(decision["routes"][0]["argv"][0], "asp");
    assert_eq!(decision["routes"][0]["argv"][1], language_id);
    assert_eq!(
        decision["routes"][0]["argv"][4], decision["subject"]["paths"][0],
        "language={language_id}"
    );
    assert_eq!(decision["languageIds"][0], language_id);
}

fn shell_read(command: &str) -> Value {
    json!({
        "tool_name": "Bash",
        "tool_input": { "command": command }
    })
}

#[test]
fn native_language_reads_fail_closed_through_the_profile_rule() {
    for (language_id, provider_id, path) in [
        ("rust", "asp-rust", "src/lib.rs"),
        ("typescript", "asp-typescript", "src/app.ts"),
        ("python", "asp-python", "src/app.py"),
        ("julia", "asp-julia", "src/app.jl"),
        ("gerbil-scheme", "asp-gerbil-scheme", "src/app.ss"),
        ("org", "asp-org", "docs/plan.org"),
        ("md", "asp-md", "README.md"),
    ] {
        assert_profile_read_route(native_read(path), "Read", language_id, provider_id);
    }
}

#[test]
fn native_json_read_stays_owned_by_the_structured_document_rule() {
    let decision = classify_codex_plugin_scenario(
        &empty_runtime(),
        &ClientHookConfig::default(),
        "pre-tool",
        &native_read("fixtures/policy.json"),
        Some("Read"),
        None,
    )
    .expect("classify native JSON Read scenario");
    assert_eq!(decision["decision"], "deny");
    assert_eq!(
        decision["fields"]["configRuleId"],
        "route-structured-document-read"
    );
    assert_eq!(decision["reasonKind"], "structured-source-read");
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
            decision["fields"]["configRuleId"], "route-unresolved-source-access-to-asp-languages",
            "language={language_id}"
        );
        assert_eq!(decision["languageIds"][0], language_id);
    }
}

#[test]
fn source_operands_without_read_behavior_do_not_fabricate_read_permission() {
    for (language_id, command) in [
        (
            "rust",
            "future-source-consumer --opaque crates/agent-semantic-hook/src/action_ir/subject.rs",
        ),
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
        assert!(
            decision["languageIds"]
                .as_array()
                .is_some_and(Vec::is_empty)
        );
        assert!(
            decision["fields"]["agentAction"]["semanticCapabilities"]
                .as_array()
                .is_some_and(|capabilities| capabilities
                    .iter()
                    .all(|capability| capability["action"] != "read"))
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
    let decision = classify_codex_plugin_scenario(
        &empty_runtime(),
        &ClientHookConfig::default(),
        "pre-tool",
        &shell_read("git status --short"),
        Some("Bash"),
        None,
    )
    .expect("classify metadata command without a registered source operand");

    assert_eq!(decision["decision"], "allow");
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

#[test]
fn durable_direct_read_shard_rebinds_the_exact_host_invocation() {
    let config = ClientHookConfig::default();
    let shards = config
        .materialized_decision_shards()
        .expect("materialize direct-read shards");
    let (_, bytes) = shards
        .direct_read
        .iter()
        .find(|(extension, _)| extension == ".rs")
        .expect("Rust direct-read shard");
    let mut decision = HookDecision::from_compact_binary(bytes).expect("decode direct-read shard");
    let path = "crates/agent-semantic-hook/src/hook_config/core/implementation/compiled_rule.rs";
    assert!(decision.replace_template_marker("__ASP_DIRECT_READ_PATH__.rs", path));
    let payload = native_read(path);
    let key = direct_read_source_key(&payload).expect("native Read key");
    let decision = rebind_direct_read_decision_to_payload(decision, &payload, &key);

    assert_eq!(decision.subject.tool_name.as_deref(), Some("Read"));
    assert_eq!(
        decision.fields["agentAction"]["hostInvocation"]["toolName"],
        "Read"
    );
    assert_eq!(
        decision.fields["agentAction"]["hostInvocation"]["payload"]["file_path"],
        path
    );
    assert!(
        decision.fields["hookPolicySnapshotDigest"]
            .as_str()
            .is_some_and(|digest| digest.starts_with("blake3-256:")),
        "durable direct-read shards must retain the policy snapshot identity"
    );
}
