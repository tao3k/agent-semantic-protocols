use super::{
    default_hook_client_config_file, load_asp_project_config_file, load_hook_client_config_file,
    merge_asp_project_hook_config, projected_agent_routes, resident_agent, temp_root,
    write_canonical_config_overlay,
};
use std::fs;

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
fn existing_config_without_agent_dispatch_does_not_require_agent_table() {
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

    let config = load_hook_client_config_file(&config_path)
        .expect("rules without agent dispatch do not require an agent projection");
    assert!(config.agents.resident_agents.is_empty());

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
maxSliceItems = 64
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
maxSliceItems = 64
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
maxSliceItems = 64
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
fn project_hook_declarations_replace_rules_but_reject_agent_identity_overlays() {
    let root = temp_root("project-hook-stable-identity-merge");
    let config_path = root.join(".agents/asp.toml");
    fs::create_dir_all(config_path.parent().expect("config parent")).expect("config dir");
    fs::write(
        &config_path,
        r#"
[[hook.agents.residentAgents]]
enabled = true
name = "asp_explorer"
role = "project_search"
roles = ["subagent", "search"]
permissions = ["read-only"]
codexAgentName = "project_search"
sessionLifetime = "resident"
"#,
    )
    .expect("write project config");
    let error = load_asp_project_config_file(&config_path)
        .expect_err("project hook config cannot shadow the agent route registry");
    assert!(error.contains("unknown field"), "{error}");
    assert!(error.contains("agents"), "{error}");

    fs::write(
        &config_path,
        r#"
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
    .expect("write rule-only project config");

    let project = load_asp_project_config_file(&config_path).expect("load project hook config");
    let mut base = default_hook_client_config_file().expect("default config");
    base.agents = toml::from_str::<toml::Value>(&projected_agent_routes())
        .expect("agent route projection TOML")["agents"]
        .clone()
        .try_into()
        .expect("typed agent route projection");
    let merged = merge_asp_project_hook_config(base, project).expect("merge project declarations");

    assert_eq!(merged.agents.resident_agents.len(), 2);
    let explore = resident_agent(&merged, "asp_explorer");
    assert_eq!(explore.role, "asp_explorer");
    assert_eq!(explore.codex_agent_name, "asp_explorer");
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
