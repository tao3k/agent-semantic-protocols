use agent_semantic_hook::{
    ClientHookConfig, HookDecision, HookRuntime, direct_read_source_key,
    rebind_direct_read_decision_to_payload,
};
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

fn native_read(path: &str) -> Value {
    json!({
        "tool_name": "Read",
        "tool_input": { "file_path": path }
    })
}

fn codex_fs_read_file(path: &str) -> Value {
    json!({
        "tool_name": "fs/readFile",
        "tool_input": { "path": path }
    })
}

fn assert_profile_read_route(
    payload: Value,
    expected_tool_name: &str,
    language_id: &str,
    provider_id: &str,
) {
    let decision = classify_hook_scenario(
        &empty_runtime(),
        &ClientHookConfig::default(),
        "codex",
        "pre-tool",
        &payload,
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
fn codex_filesystem_org_and_markdown_reads_fail_closed_through_the_profile_rule() {
    for (language_id, provider_id, path) in [
        ("org", "asp-org", "docs/plan.org"),
        ("md", "asp-md", "README.md"),
    ] {
        assert_profile_read_route(
            codex_fs_read_file(path),
            "fs/readFile",
            language_id,
            provider_id,
        );
    }
}

#[test]
fn native_json_read_stays_owned_by_the_structured_document_rule() {
    let decision = classify_hook_scenario(
        &empty_runtime(),
        &ClientHookConfig::default(),
        "codex",
        "pre-tool",
        &native_read("fixtures/policy.json"),
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
        let decision = classify_hook_scenario(
            &empty_runtime(),
            &ClientHookConfig::default(),
            "codex",
            "pre-tool",
            &shell_read(command),
        )
        .expect("classify shell document read");
        assert_eq!(decision["decision"], "deny", "language={language_id}");
        assert_eq!(
            decision["fields"]["configRuleId"], "route-read-to-asp-languages",
            "language={language_id}"
        );
        assert_eq!(decision["languageIds"][0], language_id);
    }
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
}
