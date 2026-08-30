use super::{
    default_hook_client_config_file, load_asp_project_config_file, load_hook_client_config_file,
    merge_asp_project_hook_config, temp_root, write_canonical_config_overlay,
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
fn existing_config_without_dispatch_accepts_direct_rules() {
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
    assert_eq!(config.rules.len(), 1);

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
providerId = "asp-rust"
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
fn unknown_rule_fields_are_rejected_by_config_layer() {
    let root = temp_root("hook-client-invalid-materializer");
    let config_path = root.join("config.toml");
    write_canonical_config_overlay(
        &config_path,
        r#"
[[rules]]
id = "deny-source-access"
decision = "deny"
obsoleteRuleField = "legacy-source-classifier"
"#,
    );

    let error = load_hook_client_config_file(&config_path).expect_err("invalid materializer");

    assert!(error.contains("obsoleteRuleField"), "{error}");
    let _ = fs::remove_dir_all(root);
}

#[test]
fn unknown_rule_fields_are_rejected_before_route_validation() {
    let root = temp_root("hook-client-materializer-routes");
    let config_path = root.join("config.toml");
    write_canonical_config_overlay(
        &config_path,
        r#"
[[rules]]
id = "deny-source-access"
decision = "deny"
obsoleteRuleField = "apply-patch"

[[rules.routes]]
providerId = "asp-rust"
kind = "query"
argv = ["asp", "rust", "query"]
"#,
    );

    let error = load_hook_client_config_file(&config_path).expect_err("ambiguous materializer");

    assert!(error.contains("obsoleteRuleField"), "{error}");
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
fn language_profile_rejects_non_relative_source_roots() {
    for (label, source_root) in [
        ("parent", "../crates"),
        ("absolute", "/workspace/crates"),
        ("glob", "crates/**"),
    ] {
        let root = temp_root(label);
        let config_path = root.join("config.toml");
        write_canonical_config_overlay(
            &config_path,
            &format!(
                r#"
[profiles.invalid-root]
languageId = "rust"
providerId = "asp-rust"
extensionAny = ["rs"]
sourceRootAny = ["{source_root}"]
"#
            ),
        );
        let error = load_hook_client_config_file(&config_path).expect_err("invalid source root");
        assert!(
            error.contains("sourceRootAny entries must be normalized relative source roots"),
            "{error}"
        );
        let _ = fs::remove_dir_all(root);
    }
}

#[test]
fn canonical_source_routing_uses_wrapped_profiles_without_legacy_search() {
    let config = default_hook_client_config_file().expect("canonical Hook config");
    assert!(
        config
            .rules
            .iter()
            .all(|rule| rule.id != "deny-uncontrolled-source-search-commands")
    );
    let route = config
        .rules
        .iter()
        .find(|rule| rule.id == "route-read-to-asp-languages")
        .expect("registered source routing rule");
    assert!(route.matcher.is_none());
    assert!(route.matcher_policies.is_empty());
    assert_eq!(
        route.actions,
        [agent_semantic_config::HookClientActionKind::Read]
    );
    assert!(route.match_config.capability_policy_all.is_empty());
    assert!(route.match_config.argv_prefix_any.is_empty());
    assert_eq!(
        route.profiles_list,
        [
            "rust",
            "typescript",
            "python",
            "julia",
            "gerbil-scheme",
            "org",
            "markdown",
        ]
    );
    for profile_id in &route.profiles_list {
        let profile = config
            .profiles
            .get(profile_id)
            .expect("route profile exists");
        assert!(
            !profile.source_root_any.is_empty(),
            "profile {profile_id} must declare sourceRootAny"
        );
    }
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

#[test]
fn rule_matcher_accepts_exact_host_aliases_and_rejects_regex_syntax() {
    for (name, matcher) in [
        ("canonical-apply-patch", "apply_patch"),
        ("official-edit-aliases", "Edit|Write"),
        ("exact-mcp", "mcp__filesystem__read_file"),
    ] {
        let root = temp_root(name);
        let config_path = root.join("config.toml");
        write_canonical_config_overlay(
            &config_path,
            &format!(
                r#"
[[rules]]
id = "native-action-{name}"
platform = "codex"
decision = "deny"
matcher = "{matcher}"
"#
            ),
        );

        load_hook_client_config_file(&config_path).expect("declarative Host matcher expression");

        let _ = fs::remove_dir_all(root);
    }

    let root = temp_root("unsupported-codex-matcher-regex");
    let config_path = root.join("config.toml");
    write_canonical_config_overlay(
        &config_path,
        r#"
[[rules]]
id = "unsupported-codex-matcher-regex"
platform = "codex"
matcher = "^mcp__.*$"
decision = "deny"
"#,
    );
    let error = load_hook_client_config_file(&config_path).expect_err("regex syntax must fail");
    assert!(error.contains("unsupported Host matcher"), "{error}");
    let _ = fs::remove_dir_all(root);
}
