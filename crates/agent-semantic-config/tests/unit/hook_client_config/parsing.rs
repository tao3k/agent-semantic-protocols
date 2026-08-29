use super::{
    CLIENT_HOOK_CONFIG_SCHEMA_ID, canonical_default_template, hook_client_contract_fingerprint,
    load_asp_project_config_file, load_hook_client_config_file, temp_root,
    write_canonical_config_overlay,
};
use std::fs;

#[test]
fn default_template_round_trips_through_config_parser() {
    let root = temp_root("hook-client-template");
    let config_path = root.join("hooks").join("config.toml");
    fs::create_dir_all(config_path.parent().expect("config parent")).expect("config dir");
    fs::write(&config_path, canonical_default_template()).expect("write config");

    let config = load_hook_client_config_file(&config_path).expect("load config");

    assert_eq!(
        config.schema_id.as_deref(),
        Some(CLIENT_HOOK_CONFIG_SCHEMA_ID)
    );
    assert_eq!(
        config.contract_fingerprint.as_deref(),
        Some(hook_client_contract_fingerprint().as_str())
    );
    assert!(config.experimental.is_empty());
    assert!(config.agent_org_artifacts.is_none());
    assert!(config.recovery_prompt.template.is_none());
    assert!(config.recovery_prompt.codex_agent_flow.is_none());
    assert!(config.recovery_prompt.claude_agent_flow.is_none());
    assert!(config.recovery_prompt.default_agent_flow.is_none());
    assert_eq!(config.provider_routes.len(), 7);
    for (language_id, provider_id) in [("org", "asp-org"), ("md", "asp-md")] {
        assert!(
            config.provider_routes.iter().any(|route| {
                route.language_id == language_id && route.provider_id == provider_id
            }),
            "canonical Provider Register route missing from Hook projection: {language_id}/{provider_id}"
        );
    }
    let rendered = canonical_default_template();
    for legacy in [
        "choice pane",
        "bootstrap pane",
        "asp agent session bootstrap",
    ] {
        assert!(
            !rendered.contains(legacy),
            "legacy Rust ChoicePlane escaped: {legacy}"
        );
    }
    let testing_dispatch = config
        .rules
        .iter()
        .find(|rule| rule.id == "testing-role-dispatch")
        .and_then(|rule| rule.dispatch.as_ref())
        .expect("testing resident dispatch");
    assert_eq!(testing_dispatch.agent.as_str(), "asp_testing");
    assert_eq!(
        testing_dispatch.receipt_kind.as_str(),
        "asp-testing-execution-v1"
    );
    for rule_id in ["registered-asp-reasoning-search"] {
        let dispatch = config
            .rules
            .iter()
            .find(|rule| rule.id == rule_id)
            .and_then(|rule| rule.dispatch.as_ref())
            .unwrap_or_else(|| panic!("{rule_id} must declare an Agent route"));
        assert_eq!(dispatch.agent.as_str(), "asp_explorer");
    }
    assert_eq!(
        config
            .rules
            .iter()
            .find(|rule| rule.id == "testing-role-dispatch")
            .expect("testing dispatch rule")
            .match_config
            .command_profile_any
            .iter()
            .map(|profile| (profile.profile.as_str(), profile.category.as_str()))
            .collect::<Vec<_>>(),
        vec![
            ("rust-cargo", "testing"),
            ("typescript-node", "testing"),
            ("python-uv", "testing"),
            ("julia-pkg", "testing"),
            ("c-cmake", "testing"),
            ("gerbil-gxpkg", "testing"),
            ("lean-lake", "testing"),
        ]
    );
    let bounded_json = config
        .rules
        .iter()
        .find(|rule| rule.id == "allow-bounded-json-projection")
        .expect("bounded JSON projection rule");
    assert!(matches!(
        bounded_json.decision,
        agent_semantic_config::HookClientConfigDecision::Allow
    ));
    assert!(bounded_json.match_config.argv_workspace_regular_file);
    let json_projection = bounded_json
        .match_config
        .structured_projection
        .as_ref()
        .expect("JSON projection matcher");
    assert_eq!(json_projection.binary, "jq");
    assert_eq!(
        json_projection.document_format,
        agent_semantic_config::HookClientStructuredFormat::Json
    );
    assert_eq!(
        bounded_json
            .fields
            .get("capabilityActivation")
            .map(String::as_str),
        Some("lazy-executable")
    );
    let bounded_toml = config
        .rules
        .iter()
        .find(|rule| rule.id == "allow-bounded-toml-projection")
        .expect("bounded TOML projection rule");
    let toml_projection = bounded_toml
        .match_config
        .structured_projection
        .as_ref()
        .expect("TOML projection matcher");
    assert_eq!(toml_projection.binary, "yq");
    assert_eq!(toml_projection.optional_subcommand_any, ["eval", "e"]);
    let git_history = config
        .command_sets
        .iter()
        .find(|command_set| command_set.id == "git-history-inspection")
        .expect("repository-wide Git history command set");
    assert!(
        git_history
            .argv_prefix_any
            .contains(&vec!["git".to_owned(), "log".to_owned()])
    );
    assert!(
        !git_history
            .argv_prefix_any
            .contains(&vec!["git".to_owned(), "grep".to_owned()])
    );
    assert_eq!(config.rules.len(), 16);
    assert_eq!(
        config
            .rules
            .iter()
            .map(|rule| rule.id.as_str())
            .collect::<Vec<_>>(),
        [
            "allow-explicit-no-agent",
            "allow-owner-scoped-mutation",
            "registered-asp-reasoning-search",
            "registered-asp-structured-projection",
            "testing-role-dispatch",
            "rust-format-check-role-dispatch",
            "review-role-dispatch",
            "git-history-inspection-dispatch",
            "live-corpus-qualification-dispatch",
            "gerbil-build-role-dispatch",
            "deny-agent-search-json",
            "route-read-to-asp-languages",
            "route-shell-structured-document-read",
            "allow-bounded-json-projection",
            "allow-bounded-toml-projection",
            "deny-unbounded-structured-projection",
        ]
    );
    assert!(
        config
            .rules
            .iter()
            .all(|rule| rule.id != "deny-raw-registered-source-search-action"),
        "the retired command-name search rule must not re-enter the matcher DSL"
    );
    let rendered = canonical_default_template();
    for removed_key in [
        "aspCommandIntentPolicy",
        "mainAllowedAspCommandPrefixes",
        "lifecycle =",
        "prompt-search-strategy",
        "commandWrappers",
        "invocationShapeAny",
        "wrapperMatchAny",
        "flagPresenceAny",
    ] {
        assert!(
            !rendered.contains(removed_key),
            "legacy key remains: {removed_key}"
        );
    }
    let _ = fs::remove_dir_all(root);
}

#[test]
fn managed_config_without_copied_provider_routes_uses_build_admitted_register() {
    let root = temp_root("hook-client-provider-register-admission");
    let config_path = root.join("config.toml");
    fs::write(
        &config_path,
        r#"
schemaId = "agent.semantic-protocols.hook.client-config"
schemaVersion = "1"
protocolId = "agent.semantic-protocols.hook"
protocolVersion = "1"
"#,
    )
    .expect("write route-free managed config");

    let config = load_hook_client_config_file(&config_path)
        .expect("canonical Provider Register must be admitted without copied route tables");

    assert!(
        config
            .provider_routes
            .iter()
            .any(|route| { route.language_id == "org" && route.provider_id == "asp-org" })
    );
    assert!(
        config
            .provider_routes
            .iter()
            .any(|route| { route.language_id == "md" && route.provider_id == "asp-md" })
    );
    let _ = fs::remove_dir_all(root);
}

#[test]
fn default_template_contains_no_legacy_asp_facade_rule() {
    let value: toml::Value =
        toml::from_str(&canonical_default_template()).expect("parse default TOML");
    let rules = value
        .get("rules")
        .and_then(toml::Value::as_array)
        .expect("default rules");
    let legacy = rules.iter().find(|rule| {
        rule.get("id").and_then(toml::Value::as_str) == Some("deny-invalid-asp-facade")
    });
    assert!(
        legacy.is_none(),
        "legacy ASP facade rule remains: {legacy:#?}"
    );
}

#[test]
fn unknown_rule_field_is_rejected() {
    let root = temp_root("legacy-invalid-asp-facade-materializer");
    let config_path = root.join("hooks/config.toml");
    fs::create_dir_all(config_path.parent().expect("config parent")).expect("config dir");
    let legacy_rule = r#"
[[rules]]
id = "legacy-invalid-asp-facade"
priority = 1
decision = "deny"
obsoleteRuleField = "invalid-asp-facade"
message = "legacy"
"#;
    fs::write(
        &config_path,
        format!("{}{}", canonical_default_template(), legacy_rule),
    )
    .expect("write legacy config");

    let error = load_hook_client_config_file(&config_path).expect_err("legacy value must fail");
    assert!(
        error.contains("obsoleteRuleField"),
        "unexpected error: {error}"
    );
    let _ = fs::remove_dir_all(root);
}

#[test]
fn legacy_intent_policy_is_rejected() {
    let root = temp_root("legacy-intent-policy");
    let config_path = root.join("hooks/config.toml");
    fs::create_dir_all(config_path.parent().expect("config parent")).expect("config dir");
    fs::write(
        &config_path,
        format!(
            "{}\n[aspCommandIntentPolicy.controlPlane]\nrootCommands = [\"sync\"]\n",
            canonical_default_template()
        ),
    )
    .expect("write legacy config");
    let policy_error =
        load_hook_client_config_file(&config_path).expect_err("legacy policy must fail");
    assert!(
        policy_error.contains("aspCommandIntentPolicy"),
        "unexpected error: {policy_error}"
    );

    let _ = fs::remove_dir_all(root);
}

#[test]
fn client_config_loads_recovery_prompt_template() {
    let root = temp_root("hook-client-recovery-prompt");
    let config_path = root.join("config.toml");
    write_canonical_config_overlay(
        &config_path,
        r#"
[recoveryPrompt]
template = "reason={reason}\nflow={agent_flow}\nroutes={routes}"
codexAgentFlow = "codex flow from config"
claudeAgentFlow = "claude flow from config"
defaultAgentFlow = "default flow from config"
"#,
    );

    let config = load_hook_client_config_file(&config_path).expect("load config");

    assert_eq!(
        config.recovery_prompt.template.as_deref(),
        Some("reason={reason}\nflow={agent_flow}\nroutes={routes}")
    );
    assert_eq!(
        config.recovery_prompt.codex_agent_flow.as_deref(),
        Some("codex flow from config")
    );
    assert_eq!(
        config.recovery_prompt.claude_agent_flow.as_deref(),
        Some("claude flow from config")
    );
    assert_eq!(
        config.recovery_prompt.default_agent_flow.as_deref(),
        Some("default flow from config")
    );
    let _ = fs::remove_dir_all(root);
}

#[test]
fn project_config_rejects_hook_agent_org_artifacts() {
    let root = temp_root("asp-project-config-agent-org-artifacts");
    let config_path = root.join(".agents").join("asp.toml");
    fs::create_dir_all(config_path.parent().expect("config parent")).expect("config dir");
    fs::write(
        &config_path,
        r#"
[skills.agent-semantic-protocols]
template = "SKILL.org"

[hook.agentOrgArtifacts]
enabled = false
inactiveAfterMinutes = 45
artifactsPath = "/tmp/asp-state/projects/by-id/repo-test/workspaces/workspace-test/artifacts/org"
entrySkillPath = "/tmp/asp-state/org/templates/ASP_ORG_SKILL.org"
"#,
    )
    .expect("write asp config");

    let err = load_asp_project_config_file(&config_path).expect_err("reject asp config");
    assert!(err.contains("agentOrgArtifacts"), "{err}");
    assert!(err.contains("unknown field"), "{err}");
    let _ = fs::remove_dir_all(root);
}

#[test]
fn template_routes_confirmed_bash_read_through_declared_action() {
    let root = temp_root("hook-client-template-workspace-files");
    let config_path = root.join("hooks").join("config.toml");
    fs::create_dir_all(config_path.parent().expect("config parent")).expect("config dir");
    fs::write(&config_path, canonical_default_template()).expect("write config");

    let config = load_hook_client_config_file(&config_path).expect("load config");
    let read_route = config
        .rules
        .iter()
        .find(|rule| rule.id == "route-read-to-asp-languages")
        .expect("confirmed Bash Read route");
    assert_eq!(read_route.matcher.as_deref(), Some("Bash"));
    assert_eq!(
        read_route.actions,
        [agent_semantic_config::HookClientActionKind::Read]
    );
    assert!(!read_route.profiles_list.is_empty());
    let _ = fs::remove_dir_all(root);
}

#[test]
fn template_declares_the_canonical_apply_patch_matcher() {
    let root = temp_root("hook-client-template-native-matchers");
    let config_path = root.join("hooks").join("config.toml");
    fs::create_dir_all(config_path.parent().expect("config parent")).expect("config dir");
    fs::write(&config_path, canonical_default_template()).expect("write config");

    let config = load_hook_client_config_file(&config_path).expect("load config");
    let edit_rule = config
        .rules
        .iter()
        .find(|rule| rule.id == "allow-owner-scoped-mutation")
        .expect("native Edit rule");
    assert_eq!(edit_rule.matcher.as_deref(), Some("^apply_patch$"));

    let _ = fs::remove_dir_all(root);
}
