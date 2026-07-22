use std::fs;
use std::path::{Path, PathBuf};

use agent_semantic_config::{
    CLIENT_HOOK_CONFIG_SCHEMA_ID, HookClientConfigFile, HookClientResidentAgentConfig,
    default_hook_client_config_file, default_hook_client_config_template,
    hook_client_contract_fingerprint, load_asp_project_config_file, load_hook_client_config_file,
    merge_asp_project_hook_config,
};

fn resident_agent<'a>(
    config: &'a HookClientConfigFile,
    name: &str,
) -> &'a HookClientResidentAgentConfig {
    config
        .agents
        .resident_agents
        .iter()
        .find(|agent| agent.name == name)
        .expect("resident agent")
}

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
        template = default_hook_client_config_template(),
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

    let asp_explore = resident_agent(&config, "asp-explore");
    assert_eq!(asp_explore.codex_agent_name, "asp_explorer");

    let asp_testing = resident_agent(&config, "asp-testing");
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
    fs::write(&config_path, default_hook_client_config_template()).expect("write config");

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
    assert!(
        config
            .agent_session_guide
            .register()
            .as_deref()
            .is_some_and(|guide| guide.contains("asp agent session register guide"))
    );
    assert!(
        config
            .agent_session_guide
            .register()
            .as_deref()
            .is_some_and(|guide| guide.contains("asp agent session bootstrap"))
    );
    assert!(
        config.agent_session_guide.status().as_deref().is_some_and(
            |guide| guide.contains("bootstrap --name <residentChildName-from-hook-decision>")
        )
    );
    assert!(config.agent_session_guide.reuse().is_none());
    assert!(
        config
            .agent_session_messages
            .missing_resident_explore
            .as_deref()
            .is_some_and(|message| message.contains("choose one number"))
    );
    assert!(
        config
            .agent_session_messages
            .missing_resident_explore
            .as_deref()
            .is_some_and(|message| !message.contains("register --guide"))
    );
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
            .is_some_and(|message| message.contains("send the blocked ASP command"))
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
    let asp_explore = resident_agent(&config, "asp-explore");
    assert!(asp_explore.enabled);
    assert_eq!(asp_explore.name, "asp-explore");
    assert_eq!(asp_explore.codex_agent_name, "asp_explorer");
    let asp_testing = resident_agent(&config, "asp-testing");
    assert_eq!(asp_testing.codex_agent_name, "asp_testing");
    assert_eq!(config.agents.resident_agents.len(), 2);
    let testing_dispatch = config
        .rules
        .iter()
        .find(|rule| rule.id == "resident-testing-dispatch")
        .and_then(|rule| rule.dispatch.as_ref())
        .expect("testing resident dispatch");
    assert_eq!(testing_dispatch.resident_name, "asp-testing");
    assert_eq!(testing_dispatch.receipt_kind, "asp-testing-execution-v1");
    assert_eq!(
        config
            .rules
            .iter()
            .find(|rule| rule.id == "resident-testing-dispatch")
            .expect("testing dispatch rule")
            .match_config
            .argv_prefix_any,
        vec![
            vec!["cargo", "test"],
            vec!["cargo", "check"],
            vec!["cargo", "build"],
            vec!["pytest"],
            vec!["uv", "run", "pytest"],
            vec!["just", "test"],
            vec!["rs-harness"],
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
    assert_eq!(config.rules.len(), 14);
    assert_eq!(
        config
            .rules
            .iter()
            .map(|rule| rule.id.as_str())
            .collect::<Vec<_>>(),
        [
            "registered-asp-reasoning-search",
            "resident-testing-dispatch",
            "deny-raw-registered-source-action",
            "deny-agent-search-json",
            "materialize-apply-patch-policy",
            "materialize-source-access-policy",
            "deny-uncontrolled-source-search-commands",
            "allow-bounded-json-projection",
            "allow-bounded-toml-projection",
            "deny-unbounded-structured-projection",
            "deny-uncontrolled-source-materialization-commands",
            "deny-uncontrolled-python-inline-source-materialization",
            "deny-uncontrolled-javascript-inline-source-materialization",
            "deny-uncontrolled-git-source-reads",
        ]
    );
    let rendered = default_hook_client_config_template();
    for removed_key in [
        "aspCommandIntentPolicy",
        "mainAllowedAspCommandPrefixes",
        "lifecycle =",
        "prompt-search-strategy",
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
        toml::from_str(&default_hook_client_config_template()).expect("parse default TOML");
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
        format!("{}{}", default_hook_client_config_template(), legacy_rule),
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
            default_hook_client_config_template()
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
name = "asp-explore"
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

[agentSessionGuide]
register = "register guide"
list = "list guide"
show = "show guide"
reuse = "reuse guide"
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
    assert_eq!(
        config.agent_session_guide.register().as_deref(),
        Some("register guide")
    );
    assert_eq!(
        config.agent_session_guide.list().as_deref(),
        Some("list guide")
    );
    assert_eq!(
        config.agent_session_guide.show().as_deref(),
        Some("show guide")
    );
    assert_eq!(
        config.agent_session_guide.reuse().as_deref(),
        Some("reuse guide")
    );
    let asp_explore = resident_agent(&config, "asp-explore");
    assert!(asp_explore.enabled);
    assert_eq!(asp_explore.name, "asp-explore");
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
    fs::write(&config_path, default_hook_client_config_template()).expect("write config");

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

#[test]
fn missing_config_is_rejected() {
    let root = temp_root("hook-client-missing");
    let config_path = root.join("missing.toml");
    let error = load_hook_client_config_file(&config_path).expect_err("missing config must fail");

    assert!(error.contains("hook client config does not exist"));
    assert!(error.contains(&config_path.display().to_string()));
    let _ = fs::remove_dir_all(root);
}

#[test]
fn existing_config_requires_resident_identity_table() {
    let root = temp_root("hook-client-missing-control-plane");
    let config_path = root.join("config.toml");
    fs::write(
        &config_path,
        r#"
[[rules]]
id = "deny-rust-read"
decision = "deny"
"#,
    )
    .expect("write incomplete config");

    let error = load_hook_client_config_file(&config_path)
        .expect_err("config without agents and execution lanes must fail");

    assert!(error.contains("missing field"));
    assert!(error.contains("agents"));

    let _ = fs::remove_dir_all(root);
}

#[test]
fn agent_org_artifacts_config_requires_state_core_paths_when_block_present() {
    let root = temp_root("hook-client-agent-org-artifacts-defaults");
    let config_path = root.join("config.toml");
    fs::write(
        &config_path,
        r#"
schemaId = "agent.semantic-protocols.hook.client-config"
schemaVersion = "1"
protocolId = "agent.semantic-protocols.hook"
protocolVersion = "1"

[agentOrgArtifacts]
enabled = true
"#,
    )
    .expect("write config");

    let error = load_hook_client_config_file(&config_path).expect_err("missing State Core paths");
    assert!(
        error.contains("artifactsPath"),
        "expected artifactsPath error, got {error}"
    );
    let _ = fs::remove_dir_all(root);
}

#[test]
fn agent_org_artifacts_rejects_empty_paths_and_zero_minutes() {
    let root = temp_root("hook-client-agent-org-artifacts-invalid");
    let config_path = root.join("config.toml");
    write_canonical_config_overlay(
        &config_path,
        r#"
[agentOrgArtifacts]
inactiveAfterMinutes = 0
artifactsPath = ""
entrySkillPath = ""

[agentOrgArtifacts.archiveWarning]
activeOrgFileThreshold = 0
archivesDir = ""
maxReportedFiles = 0
"#,
    );

    let error =
        load_hook_client_config_file(&config_path).expect_err("reject invalid agent org artifacts");
    assert!(
        error.contains("agentOrgArtifacts.inactiveAfterMinutes must be greater than 0"),
        "{error}"
    );
    let _ = fs::remove_dir_all(root);
}

#[test]
fn invalid_route_kind_is_rejected_by_config_layer() {
    let root = temp_root("hook-client-invalid-route");
    let config_path = root.join("config.toml");
    fs::write(
        &config_path,
        r#"
schemaId = "agent.semantic-protocols.hook.client-config"
schemaVersion = "1"
protocolId = "agent.semantic-protocols.hook"
protocolVersion = "1"

[[rules]]
id = "deny-rust-read"
decision = "deny"

[[rules.routes]]
providerId = "rs-harness"
kind = "route-text"
argv = ["asp", "rust"]
"#,
    )
    .expect("write config");

    let error = load_hook_client_config_file(&config_path).expect_err("invalid route kind");

    assert!(error.contains("route-text"), "{error}");
    let _ = fs::remove_dir_all(root);
}

#[test]
fn invalid_decision_materializer_is_rejected_by_config_layer() {
    let root = temp_root("hook-client-invalid-materializer");
    let config_path = root.join("config.toml");
    write_canonical_config_overlay(
        &config_path,
        r#"
[[rules]]
id = "deny-source-access"
decision = "deny"
decisionMaterializer = "legacy-source-classifier"
"#,
    );

    let error = load_hook_client_config_file(&config_path).expect_err("invalid materializer");

    assert!(error.contains("legacy-source-classifier"), "{error}");
    let _ = fs::remove_dir_all(root);
}

#[test]
fn decision_materializer_cannot_compete_with_static_routes() {
    let root = temp_root("hook-client-materializer-routes");
    let config_path = root.join("config.toml");
    write_canonical_config_overlay(
        &config_path,
        r#"
[[rules]]
id = "deny-source-access"
decision = "deny"
decisionMaterializer = "source-access"

[[rules.routes]]
providerId = "rs-harness"
kind = "query"
argv = ["asp", "rust", "query"]
"#,
    );

    let error = load_hook_client_config_file(&config_path).expect_err("ambiguous materializer");

    assert!(
        error.contains("cannot combine decisionMaterializer"),
        "{error}"
    );
    let _ = fs::remove_dir_all(root);
}

#[test]
fn argv_source_match_fields_round_trip_through_config_parser() {
    let root = temp_root("hook-client-argv-source");
    let config_path = root.join("config.toml");
    write_canonical_config_overlay(
        &config_path,
        r#"
[[rules]]
id = "deny-argv-source"
decision = "deny"

[rules.match]
commandAny = ["wl"]
argvSourceAny = ["src/main.ts"]
argvSourceGlobAny = ["*.ts"]
argvSourceExcludeFlagAny = ["--output"]
"#,
    );

    let config = load_hook_client_config_file(&config_path).expect("load config");
    let rule = config.rules.first().expect("config rule");

    assert_eq!(rule.match_config.argv_source_any, ["src/main.ts"]);
    assert_eq!(rule.match_config.argv_source_glob_any, ["*.ts"]);
    assert_eq!(rule.match_config.argv_source_exclude_flag_any, ["--output"]);
    let _ = fs::remove_dir_all(root);
}

#[test]
fn bounded_json_projector_round_trips_as_one_lazy_capability_contract() {
    let root = temp_root("hook-client-bounded-json-projector");
    let config_path = root.join("config.toml");
    write_canonical_config_overlay(
        &config_path,
        r#"
[[rules]]
id = "allow-bounded-json"
decision = "allow"

[rules.match]
argvWorkspaceRegularFile = true

[rules.match.structuredProjection]
binary = "project-json"
documentFormat = "json"
filterGrammar = "bounded-path-v1"
optionAny = ["--compact"]
optionValueArity = { "--arg" = 2 }
"#,
    );

    let config = load_hook_client_config_file(&config_path).expect("load config");
    let rule = config.rules.first().expect("config rule");
    let projection = rule
        .match_config
        .structured_projection
        .as_ref()
        .expect("projection matcher");
    assert_eq!(projection.binary, "project-json");
    assert_eq!(
        projection.filter_grammar,
        agent_semantic_config::HookClientStructuredFilterGrammar::BoundedPathV1
    );
    let _ = fs::remove_dir_all(root);
}

#[test]
fn bounded_projection_rejects_invalid_declarative_models() {
    for (name, match_config, expected) in [
        (
            "binary-path",
            r#"
argvWorkspaceRegularFile = true

[rules.match.structuredProjection]
binary = "../project-json"
documentFormat = "json"
filterGrammar = "bounded-path-v1"
"#,
            "invalid rules[].match.structuredProjection.binary",
        ),
        (
            "zero-option-arity",
            r#"
argvWorkspaceRegularFile = true

[rules.match.structuredProjection]
binary = "project-json"
documentFormat = "json"
filterGrammar = "bounded-path-v1"
optionValueArity = { "--arg" = 0 }
"#,
            "must start with `-` and have positive arity",
        ),
    ] {
        let root = temp_root(name);
        let config_path = root.join("config.toml");
        write_canonical_config_overlay(
            &config_path,
            &format!(
                r#"
[[rules]]
id = "allow-bounded-json"
decision = "allow"

[rules.match]
{match_config}
"#
            ),
        );
        let error = load_hook_client_config_file(&config_path).expect_err("invalid projector");
        assert!(error.contains(expected), "{error}");
        let _ = fs::remove_dir_all(root);
    }
}

#[test]
fn project_hook_declarations_replace_rules_by_id_and_residents_by_name() {
    let root = temp_root("project-hook-stable-identity-merge");
    let config_path = root.join(".agents/asp.toml");
    fs::create_dir_all(config_path.parent().expect("config parent")).expect("config dir");
    fs::write(
        &config_path,
        r#"
[[hook.agents.residentAgents]]
enabled = true
name = "asp-explore"
role = "project_search"
roles = ["subagent", "search"]
permissions = ["read-only"]
codexAgentName = "project_search"
sessionLifetime = "resident"

[[hook.rules]]
id = "deny-agent-search-json"
priority = 1200
intent = "project-json-policy"
decision = "allow"
message = "Project policy replaces the complete managed rule."

[hook.rules.match]
commandContainsAny = ["--json"]
"#,
    )
    .expect("write project config");

    let project = load_asp_project_config_file(&config_path).expect("load project hook config");
    let merged = merge_asp_project_hook_config(
        default_hook_client_config_file().expect("default config"),
        project,
    )
    .expect("merge project declarations");

    assert_eq!(merged.agents.resident_agents.len(), 2);
    let explore = resident_agent(&merged, "asp-explore");
    assert_eq!(explore.role, "project_search");
    assert_eq!(explore.codex_agent_name, "project_search");
    let replaced = merged
        .rules
        .iter()
        .filter(|rule| rule.id == "deny-agent-search-json")
        .collect::<Vec<_>>();
    assert_eq!(replaced.len(), 1);
    assert_eq!(replaced[0].intent.as_deref(), Some("project-json-policy"));
    assert!(replaced[0].decision_materializer.is_none());

    let _ = fs::remove_dir_all(root);
}

#[test]
fn project_hook_rejects_duplicate_policy_identities() {
    let root = temp_root("project-hook-duplicate-identities");
    let config_path = root.join(".agents/asp.toml");
    fs::create_dir_all(config_path.parent().expect("config parent")).expect("config dir");
    fs::write(
        &config_path,
        r#"
[[hook.rules]]
id = "project-search"
decision = "allow"

[hook.rules.match]
commandAny = ["asp"]

[[hook.rules]]
id = "project-search"
decision = "deny"

[hook.rules.match]
commandAny = ["cargo"]
"#,
    )
    .expect("write project config");

    let project = load_asp_project_config_file(&config_path).expect("load project hook config");
    let error = merge_asp_project_hook_config(
        default_hook_client_config_file().expect("default config"),
        project,
    )
    .expect_err("duplicate rule identity must be rejected");
    assert_eq!(
        error,
        "project hook declares rule `project-search` more than once"
    );

    let _ = fs::remove_dir_all(root);
}

fn write_canonical_config_overlay(path: &std::path::Path, overlay: &str) {
    let mut config = toml::from_str::<toml::Value>(&default_hook_client_config_template())
        .expect("parse canonical hook config");
    let overlay = toml::from_str::<toml::Value>(overlay).expect("parse hook config overlay");
    merge_toml_value(&mut config, overlay);
    fs::write(
        path,
        toml::to_string_pretty(&config).expect("render hook config overlay"),
    )
    .expect("write hook config overlay");
}

fn merge_toml_value(base: &mut toml::Value, overlay: toml::Value) {
    match (base, overlay) {
        (toml::Value::Table(base), toml::Value::Table(overlay)) => {
            for (key, value) in overlay {
                if let Some(existing) = base.get_mut(&key) {
                    merge_toml_value(existing, value);
                } else {
                    base.insert(key, value);
                }
            }
        }
        (base, overlay) => *base = overlay,
    }
}

fn temp_root(label: &str) -> PathBuf {
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("system time")
        .as_nanos();
    let root = std::env::temp_dir().join(format!("agent-semantic-config-{label}-{nonce}"));
    fs::create_dir_all(&root).expect("create temp root");
    canonical(&root)
}

fn canonical(path: &Path) -> PathBuf {
    path.canonicalize().unwrap_or_else(|_| path.to_path_buf())
}
