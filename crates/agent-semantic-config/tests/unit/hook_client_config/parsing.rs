use super::{
    CLIENT_HOOK_CONFIG_SCHEMA_ID, HookClientResidentAgentConfig, hook_client_contract_fingerprint,
    load_asp_project_config_file, load_hook_client_config_file, projected_default_template,
    resident_agent, temp_root, write_canonical_config_overlay,
};
use std::fs;

#[test]
fn default_template_round_trips_with_third_lint_resident() {
    let root = temp_root("hook-client-template-third-resident");
    let config_path = root.join("hooks").join("config.toml");
    fs::create_dir_all(config_path.parent().expect("config parent")).expect("config dir");

    let fixture = format!(
        r#"{template}

[[agents.residentAgents]]
enabled = true
name = "asp-lint"
role = "asp_lint"
roles = ["subagent", "lint"]
permissions = ["workspace-write"]
codexAgentName = "asp_lint"
sessionLifetime = "resident"
"#,
        template = projected_default_template(),
    );
    fs::write(&config_path, fixture).expect("write config");

    let config = load_hook_client_config_file(&config_path).expect("load config");

    assert_eq!(
        config.schema_id.as_deref(),
        Some(CLIENT_HOOK_CONFIG_SCHEMA_ID)
    );
    assert_eq!(
        config.contract_fingerprint.as_deref(),
        Some(hook_client_contract_fingerprint().as_str())
    );
    assert_eq!(config.agents.resident_agents.len(), 3);

    let asp_explore = resident_agent(&config, "asp_explorer");
    assert_eq!(asp_explore.codex_agent_name, "asp_explorer");

    let asp_testing = resident_agent(&config, "asp_testing");
    assert_eq!(asp_testing.codex_agent_name, "asp_testing");

    let asp_lint = resident_agent(&config, "asp-lint");
    assert!(asp_lint.enabled);
    assert_eq!(asp_lint.name, "asp-lint");
    assert_eq!(asp_lint.role, "asp_lint");
    assert_eq!(asp_lint.codex_agent_name, "asp_lint");
    assert_eq!(asp_lint.session_lifetime, "resident");
    assert_ne!(asp_explore.name, asp_lint.name);
    assert_ne!(asp_testing.name, asp_lint.name);

    let _ = fs::remove_dir_all(root);
}

#[test]
fn default_template_round_trips_through_config_parser() {
    let root = temp_root("hook-client-template");
    let config_path = root.join("hooks").join("config.toml");
    fs::create_dir_all(config_path.parent().expect("config parent")).expect("config dir");
    fs::write(&config_path, projected_default_template()).expect("write config");

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
    assert_eq!(
        config.wrapper_match,
        agent_semantic_config::WrapperMatchMode::Enable
    );
    assert!(config.agent_org_artifacts.is_none());
    assert!(config.recovery_prompt.template.is_none());
    assert!(config.recovery_prompt.codex_agent_flow.is_none());
    assert!(config.recovery_prompt.claude_agent_flow.is_none());
    assert!(config.recovery_prompt.default_agent_flow.is_none());
    assert!(
        config
            .agent_session_messages
            .missing_resident_explore
            .as_deref()
            .is_some_and(|message| message.contains("canonical Org-backed Agent window"))
    );
    let rendered = projected_default_template();
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
    assert!(
        config
            .agent_session_messages
            .missing_resident_explore
            .as_deref()
            .is_some_and(|message| !message.contains("agent session reuse"))
    );
    assert!(
        config
            .agent_session_messages
            .main_restricted_without_child
            .as_deref()
            .is_some_and(|message| message.contains("canonical Org-backed Agent window"))
    );
    assert!(
        config
            .agent_session_messages
            .source_access_compact_subagent
            .is_none()
    );
    let invalid_child_message = config
        .agent_session_messages
        .binary_gate_invalid_child
        .as_deref()
        .expect("binary gate invalid child message");
    assert!(invalid_child_message.contains("validation-warning-or-non-routable-child"));
    let legacy_close_delete = ["close", "/", "delete"].concat();
    assert!(!invalid_child_message.contains(&legacy_close_delete));
    assert!(!invalid_child_message.contains("destroy-invalid-child-and-create-configured-child"));
    let asp_explore = resident_agent(&config, "asp_explorer");
    assert!(asp_explore.enabled);
    assert_eq!(asp_explore.name, "asp_explorer");
    assert_eq!(asp_explore.codex_agent_name, "asp_explorer");
    let asp_testing = resident_agent(&config, "asp_testing");
    assert_eq!(asp_testing.codex_agent_name, "asp_testing");
    assert_eq!(config.agents.resident_agents.len(), 2);
    let testing_dispatch = config
        .rules
        .iter()
        .find(|rule| rule.id == "resident-testing-dispatch")
        .and_then(|rule| rule.dispatch.as_ref())
        .expect("testing resident dispatch");
    assert_eq!(testing_dispatch.role.as_str(), "testing");
    assert_eq!(
        testing_dispatch.receipt_kind.as_str(),
        "asp-testing-execution-v1"
    );
    for rule_id in [
        "registered-asp-reasoning-search",
        "deny-raw-registered-source-search-action",
        "deny-raw-registered-source-action",
        "deny-uncontrolled-source-materialization-commands",
    ] {
        let dispatch = config
            .rules
            .iter()
            .find(|rule| rule.id == rule_id)
            .and_then(|rule| rule.dispatch.as_ref())
            .unwrap_or_else(|| panic!("{rule_id} must declare a registry role selector"));
        assert_eq!(dispatch.role.as_str(), "explore");
        assert_eq!(
            config
                .agents
                .placeholders
                .get(dispatch.role.as_str())
                .map(String::as_str),
            Some("asp_explorer")
        );
    }
    assert_eq!(
        config
            .rules
            .iter()
            .find(|rule| rule.id == "resident-testing-dispatch")
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
    assert_eq!(config.rules.len(), 16);
    assert_eq!(
        config
            .rules
            .iter()
            .map(|rule| rule.id.as_str())
            .collect::<Vec<_>>(),
        [
            "registered-asp-reasoning-search",
            "resident-testing-dispatch",
            "deny-raw-registered-source-search-action",
            "deny-raw-registered-source-action",
            "deny-agent-search-json",
            "materialize-apply-patch-policy",
            "materialize-registered-source-read-action",
            "materialize-structured-document-read-action",
            "materialize-source-access-policy",
            "deny-uncontrolled-source-search-commands",
            "allow-bounded-json-projection",
            "allow-bounded-toml-projection",
            "deny-unbounded-structured-projection",
            "deny-uncontrolled-source-materialization-commands",
            "deny-uncontrolled-git-metadata-reads",
            "deny-uncontrolled-git-source-reads",
        ]
    );
    let rendered = projected_default_template();
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
fn default_template_contains_no_legacy_asp_facade_rule() {
    let value: toml::Value =
        toml::from_str(&projected_default_template()).expect("parse default TOML");
    let rules = value
        .get("rules")
        .and_then(toml::Value::as_array)
        .expect("default rules");
    let legacy = rules.iter().find(|rule| {
        rule.get("id").and_then(toml::Value::as_str) == Some("deny-invalid-asp-facade")
            || rule
                .get("decisionMaterializer")
                .and_then(toml::Value::as_str)
                == Some("invalid-asp-facade")
    });
    assert!(
        legacy.is_none(),
        "legacy ASP facade rule remains: {legacy:#?}"
    );
}

#[test]
fn legacy_invalid_asp_facade_materializer_is_rejected() {
    let root = temp_root("legacy-invalid-asp-facade-materializer");
    let config_path = root.join("hooks/config.toml");
    fs::create_dir_all(config_path.parent().expect("config parent")).expect("config dir");
    let legacy_rule = r#"
[[rules]]
id = "legacy-invalid-asp-facade"
priority = 1
decision = "deny"
decisionMaterializer = "invalid-asp-facade"
message = "legacy"
"#;
    fs::write(
        &config_path,
        format!("{}{}", projected_default_template(), legacy_rule),
    )
    .expect("write legacy config");

    let error = load_hook_client_config_file(&config_path).expect_err("legacy value must fail");
    assert!(
        error.contains("invalid-asp-facade"),
        "unexpected error: {error}"
    );
    let _ = fs::remove_dir_all(root);
}

#[test]
fn legacy_intent_policy_and_resident_route_fields_are_rejected() {
    let root = temp_root("legacy-intent-policy");
    let config_path = root.join("hooks/config.toml");
    fs::create_dir_all(config_path.parent().expect("config parent")).expect("config dir");
    fs::write(
        &config_path,
        format!(
            "{}\n[aspCommandIntentPolicy.controlPlane]\nrootCommands = [\"sync\"]\n",
            projected_default_template()
        ),
    )
    .expect("write legacy config");
    let policy_error =
        load_hook_client_config_file(&config_path).expect_err("legacy policy must fail");
    assert!(
        policy_error.contains("aspCommandIntentPolicy"),
        "unexpected error: {policy_error}"
    );

    for legacy_field in [
        "lifecycle = \"asp-command\"",
        "mainAllowedAspCommandPrefixes = [\"help\"]",
    ] {
        let resident = format!(
            r#"
enabled = true
name = "asp_explorer"
role = "asp_explorer"
roles = ["subagent", "search"]
permissions = ["read-only"]
codexAgentName = "asp_explorer"
sessionLifetime = "resident"
{legacy_field}
"#
        );
        let error = toml::from_str::<HookClientResidentAgentConfig>(&resident)
            .expect_err("legacy resident route field must fail");
        assert!(
            error
                .to_string()
                .contains(legacy_field.split_whitespace().next().unwrap()),
            "unexpected error: {error}"
        );
    }

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
    let asp_explore = resident_agent(&config, "asp_explorer");
    assert!(asp_explore.enabled);
    assert_eq!(asp_explore.name, "asp_explorer");
    assert_eq!(asp_explore.codex_agent_name, "asp_explorer");
    let _ = fs::remove_dir_all(root);
}

#[test]
fn client_config_rejects_legacy_flat_subagent_receipt_message() {
    let root = temp_root("hook-client-legacy-subagent-message");
    let config_path = root.join("config.toml");
    write_canonical_config_overlay(
        &config_path,
        r#"
[agentSessionMessages]
sourceAccessCompactSubagent = "Use ASP query/search routes and return selector-only `[asp-search-subagent]` evidence with owner/read/next."
"#,
    );

    let error = load_hook_client_config_file(&config_path).expect_err("legacy message rejected");

    assert!(
        error.contains("legacy flat subagent receipt contract"),
        "{error}"
    );
    assert!(
        error.contains("schema/intent/route/state/evidence/next"),
        "{error}"
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
fn template_uses_workspace_regular_files_without_extension_authority() {
    let root = temp_root("hook-client-template-workspace-files");
    let config_path = root.join("hooks").join("config.toml");
    fs::create_dir_all(config_path.parent().expect("config parent")).expect("config dir");
    fs::write(&config_path, projected_default_template()).expect("write config");

    let config = load_hook_client_config_file(&config_path).expect("load config");
    let materialization_rule = config
        .rules
        .iter()
        .find(|rule| rule.id == "deny-uncontrolled-source-materialization-commands")
        .expect("materialization rule");
    assert!(
        !materialization_rule
            .match_config
            .argv_workspace_regular_file
    );
    assert!(materialization_rule.match_config.argv_source_any.is_empty());
    assert!(
        materialization_rule
            .match_config
            .argv_source_glob_any
            .is_empty()
    );
    let bounded = config
        .rules
        .iter()
        .position(|rule| rule.id == "allow-bounded-json-projection")
        .expect("bounded projector rule");
    let bounded_toml = config
        .rules
        .iter()
        .position(|rule| rule.id == "allow-bounded-toml-projection")
        .expect("bounded TOML projector rule");
    let unbounded = config
        .rules
        .iter()
        .position(|rule| rule.id == "deny-unbounded-structured-projection")
        .expect("unbounded projector rule");
    let raw = config
        .rules
        .iter()
        .position(|rule| rule.id == "deny-uncontrolled-source-materialization-commands")
        .expect("raw materialization rule");
    assert!(bounded < bounded_toml && bounded_toml < unbounded && unbounded < raw);
    let _ = fs::remove_dir_all(root);
}
