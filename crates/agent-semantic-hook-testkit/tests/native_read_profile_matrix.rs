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

fn native_read(path: &str) -> Value {
    json!({
        "tool_name": "Read",
        "tool_input": { "file_path": path }
    })
}

#[test]
fn native_language_reads_fail_closed_through_the_profile_rule() {
    let runtime = empty_runtime();
    let config = ClientHookConfig::default();
    for (language_id, provider_id, path) in [
        ("rust", "asp-rust", "src/lib.rs"),
        ("typescript", "asp-typescript", "src/app.ts"),
        ("python", "asp-python", "src/app.py"),
        ("julia", "asp-julia", "src/app.jl"),
        ("gerbil-scheme", "asp-gerbil-scheme", "src/app.ss"),
    ] {
        let decision =
            classify_hook_scenario(&runtime, &config, "codex", "pre-tool", &native_read(path))
                .expect("classify native Read scenario");
        assert_eq!(decision["decision"], "deny", "language={language_id}");
        assert_eq!(
            decision["fields"]["configRuleId"], "route-read-to-asp-languages",
            "language={language_id}"
        );
        assert_eq!(
            decision["fields"]["providerAvailability"], "unavailable",
            "language={language_id}"
        );
        assert_eq!(
            decision["fields"]["providerId"], provider_id,
            "language={language_id}"
        );
        assert_eq!(
            decision["fields"]["routeStatus"], "unavailable",
            "language={language_id}"
        );
        assert_eq!(decision["languageIds"][0], language_id);
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
        "materialize-structured-document-read-action"
    );
    assert_eq!(decision["reasonKind"], "structured-source-read");
}
