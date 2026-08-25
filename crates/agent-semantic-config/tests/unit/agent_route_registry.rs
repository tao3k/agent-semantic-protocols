use super::{
    AgentSessionLifetime, compile_agent_route, load_agent_route_registry,
    load_agent_route_registry_for_platform,
};

fn canonical_registry_path() -> std::path::PathBuf {
    std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../agents/config.toml")
}

#[test]
fn canonical_registry_compiles_host_routes() {
    let loaded = load_agent_route_registry(&canonical_registry_path()).expect("canonical registry");
    assert_eq!(
        loaded
            .compile_route_for_platform_host_agent_name("codex", "asp_explorer")
            .expect("compiled session lookup")
            .map(|route| route.route_key.0),
        Some("asp_explorer".to_owned())
    );
    assert!(
        loaded
            .compile_route_for_platform_host_agent_name("codex", "asp-explore")
            .expect("exact missing session lookup")
            .is_none(),
        "legacy punctuation aliases must not resolve"
    );
    let codex = compile_agent_route(&loaded, "asp_explorer", "codex").expect("Codex route");
    let claude = compile_agent_route(&loaded, "asp_explorer", "claude").expect("Claude route");
    assert_eq!(codex.route_key.as_str(), "asp_explorer");
    assert_eq!(claude.route_key.as_str(), "asp_explorer");
    assert_eq!(codex.session_lifetime, AgentSessionLifetime::Resident);
    assert_eq!(codex.focus_mode, super::AgentFocusMode::Leaf);
    assert_eq!(codex.platform.as_str(), "codex");
    assert_eq!(claude.platform.as_str(), "claude");
    assert_eq!(codex.platform_host_agent_name.as_str(), "asp_explorer");
    assert_eq!(claude.platform_host_agent_name.as_str(), "asp-explorer");
    assert_eq!(
        codex.host_invocation("@asp_explorer").symbol,
        "@asp_explorer"
    );
    assert_eq!(
        codex.host_invocation("@asp_explorer").syntax,
        "@asp_explorer"
    );
    assert_eq!(
        claude.host_invocation("@agent-asp-explorer").symbol,
        "@agent-asp-explorer"
    );
    assert_eq!(
        claude.host_invocation("@agent-asp-explorer").syntax,
        "@agent-asp-explorer"
    );
    assert_eq!(codex.model.as_deref(), Some("gpt-5.6-luna"));
    assert_eq!(claude.model.as_deref(), Some("haiku"));
    assert!(codex.profile_path.ends_with("asp_explorer_codex.toml"));
    assert!(claude.profile_path.ends_with("asp_explorer_claude.md"));
    assert_eq!(codex.roles, vec!["explore", "explorer", "subagent"]);
    assert_eq!(
        codex.allowed_rule_intents,
        vec!["reasoning-search", "structured-projection"]
    );
    assert_eq!(codex.agent_kind, "Subagent");
    assert_eq!(codex.display_role, "Evidence Explorer");
    assert_eq!(codex.description, "for code and evidence search");
    assert_eq!(claude.description, codex.description);
    assert_eq!(codex.sandbox_mode.as_deref(), Some("read-only"));
    assert!(
        codex
            .effective_permissions
            .denies(super::AgentPermissionAction::Edit)
    );
    assert!(
        claude
            .effective_permissions
            .denies(super::AgentPermissionAction::Edit)
    );
    assert_eq!(
        claude.definition_schema_id,
        super::ANTHROPIC_PLUGIN_AGENT_FRONTMATTER_SCHEMA_ID
    );
    let testing =
        compile_agent_route(&loaded, "asp_testing", "codex").expect("Codex testing route");
    assert_eq!(testing.platform_host_agent_name.as_str(), "asp_testing");
    assert_eq!(testing.focus_mode, super::AgentFocusMode::Leaf);
    assert_eq!(testing.agent_kind, "Subagent");
    assert_eq!(testing.display_role, "Test Runner");
    assert_eq!(
        testing.allowed_rule_intents,
        vec![
            "test-build-command",
            "review-command",
            "live-corpus-qualification",
            "git-history-inspection"
        ]
    );
    assert_eq!(testing.description, "for build and test jobs");
    assert!(testing.profile_path.ends_with("asp_testing_codex.toml"));
    assert_eq!(testing.sandbox_mode.as_deref(), Some("read-only"));
    let claude_testing =
        compile_agent_route(&loaded, "asp_testing", "claude").expect("Claude testing route");
    assert_eq!(
        claude_testing.platform_host_agent_name.as_str(),
        "asp-testing"
    );
    assert!(
        claude_testing
            .effective_permissions
            .denies(super::AgentPermissionAction::Edit)
    );
}

#[test]
fn managed_agent_schemas_have_stable_identity_and_internal_version() {
    for (schema, expected_id) in [
        (
            super::CODEX_AGENT_DEFINITION_SCHEMA,
            super::CODEX_AGENT_DEFINITION_SCHEMA_ID,
        ),
        (
            super::ANTHROPIC_AGENT_FRONTMATTER_SCHEMA,
            super::ANTHROPIC_AGENT_FRONTMATTER_SCHEMA_ID,
        ),
        (
            super::ANTHROPIC_PLUGIN_AGENT_FRONTMATTER_SCHEMA,
            super::ANTHROPIC_PLUGIN_AGENT_FRONTMATTER_SCHEMA_ID,
        ),
        (
            super::AGENT_ROUTE_REGISTRY_SCHEMA,
            super::AGENT_ROUTE_REGISTRY_DOCUMENT_SCHEMA_ID,
        ),
    ] {
        assert!(schema.contains(&format!("\"$id\": \"{expected_id}\"")));
        assert!(schema.contains("\"x-asp-schema-version\": 1"));
        assert!(
            !expected_id.contains(":v1")
                && !expected_id.contains(":v2")
                && !expected_id.contains(":v3")
        );
    }
}

#[test]
fn anthropic_permission_modes_match_the_official_frontmatter_contract() {
    let schema: serde_json::Value = serde_json::from_str(super::ANTHROPIC_AGENT_FRONTMATTER_SCHEMA)
        .expect("parse Anthropic agent schema");
    assert_eq!(
        schema["properties"]["permissionMode"]["enum"],
        serde_json::json!([
            "default",
            "acceptEdits",
            "auto",
            "dontAsk",
            "bypassPermissions",
            "plan"
        ])
    );
}

#[test]
fn unknown_platform_matcher_fields_fail_closed() {
    let temp = tempfile::tempdir().expect("tempdir");
    let path = temp.path().join("config.toml");
    std::fs::write(
        &path,
        r#"schema_id = "agent.semantic-protocols.agent-route-registry"
schema_version = 1

[platforms.codex]
matcher = "*_codex.toml"
manager_projection = "legacy-shadow"

[agents.bad]
session_lifetime = "resident"
roles = ["subagent"]
allowed_rule_intents = ["reasoning-search"]
"#,
    )
    .expect("write invalid route registry");
    let error = load_agent_route_registry(&path)
        .expect_err("unknown platform matcher field must fail closed");
    assert!(error.contains("unknown field `manager_projection`"));
}

#[test]
fn unknown_platform_fails_closed() {
    let loaded = load_agent_route_registry(&canonical_registry_path()).expect("canonical registry");
    let error = compile_agent_route(&loaded, "asp_testing", "unknown-host")
        .expect_err("unknown platform must fail closed");
    assert!(error.contains("does not define `asp_testing` for `unknown-host`"));
}

#[test]
fn platform_matcher_builds_distinct_native_name_registries() {
    let root = tempfile::tempdir().expect("platform registry tempdir");
    std::fs::write(
        root.path().join("config.toml"),
        r#"schema_id = "agent.semantic-protocols.agent-route-registry"
schema_version = 1

[platforms.codex]
matcher = "*_codex.toml"
[platforms.claude]
matcher = "*_claude.md"

[agents.custom]
session_lifetime = "resident"
roles = ["explore"]
allowed_rule_intents = ["reasoning-search"]
"#,
    )
    .expect("write matcher registry");
    std::fs::write(
        root.path().join("custom_codex.toml"),
        r#"name = "codex_probe"
description = "Codex evidence probe."
model = "gpt-5.6-luna"
model_reasoning_effort = "low"
sandbox_mode = "read-only"
developer_instructions = "Run Codex evidence queries."
"#,
    )
    .expect("write Codex matcher profile");
    std::fs::write(
        root.path().join("custom_claude.md"),
        r#"---
name: claude-probe
description: Claude evidence probe.
model: haiku
---
Run Claude evidence queries.
"#,
    )
    .expect("write Claude matcher profile");

    let loaded = load_agent_route_registry(&root.path().join("config.toml"))
        .expect("load platform registries");
    assert_eq!(
        compile_agent_route(&loaded, "custom", "codex")
            .expect("Codex route")
            .platform_host_agent_name
            .as_str(),
        "codex_probe"
    );
    assert_eq!(
        compile_agent_route(&loaded, "custom", "claude")
            .expect("Claude route")
            .platform_host_agent_name
            .as_str(),
        "claude-probe"
    );
    assert!(
        loaded
            .compile_route_for_platform_host_agent_name("codex", "claude-probe")
            .expect("platform exact lookup")
            .is_none()
    );
}

#[test]
fn codex_projection_rejects_unknown_fields() {
    let root = tempfile::tempdir().expect("Codex projection tempdir");
    std::fs::write(
        root.path().join("config.toml"),
        r#"schema_id = "agent.semantic-protocols.agent-route-registry"
schema_version = 1

[platforms.codex]
matcher = "*_codex.toml"

[agents.custom]
session_lifetime = "resident"
roles = ["explore"]
allowed_rule_intents = ["reasoning-search"]
"#,
    )
    .expect("write route registry");
    std::fs::write(
        root.path().join("custom_codex.toml"),
        r#"name = "codex_probe"
description = "Codex evidence probe."
model = "gpt-5.6-luna"
model_reasoning_effort = "low"
sandbox_mode = "read-only"
developer_instructions = "Run Codex evidence queries."
permission_mode = "compatibility-shadow"
"#,
    )
    .expect("write invalid Codex projection");

    let error = load_agent_route_registry(&root.path().join("config.toml"))
        .expect_err("unknown Codex fields must fail closed");
    assert!(error.contains("unknown `permission_mode` field"));
}

#[test]
fn claude_plugin_projection_rejects_ignored_permission_fields() {
    let root = tempfile::tempdir().expect("Claude projection tempdir");
    std::fs::write(
        root.path().join("config.toml"),
        r#"schema_id = "agent.semantic-protocols.agent-route-registry"
schema_version = 1

[platforms.claude]
matcher = "*_claude.md"

[agents.custom]
session_lifetime = "resident"
roles = ["explore"]
allowed_rule_intents = ["reasoning-search"]
"#,
    )
    .expect("write route registry");
    std::fs::write(
        root.path().join("custom_claude.md"),
        r#"---
name: claude-probe
description: Claude evidence probe.
model: haiku
permissionMode: plan
---
Run Claude evidence queries.
"#,
    )
    .expect("write invalid Claude plugin projection");

    let error = load_agent_route_registry(&root.path().join("config.toml"))
        .expect_err("ignored Claude plugin permission fields must fail closed");
    assert!(error.contains("unknown or ignored `permissionMode` frontmatter"));
}

#[test]
fn platform_scoped_load_does_not_hydrate_unrelated_host_artifacts() {
    let root = tempfile::tempdir().expect("platform isolation tempdir");
    std::fs::write(
        root.path().join("config.toml"),
        r#"schema_id = "agent.semantic-protocols.agent-route-registry"
schema_version = 1

[platforms.codex]
matcher = "*_codex.toml"

[platforms.claude]
matcher = "*_claude.md"

[agents.custom]
session_lifetime = "resident"
roles = ["explore"]
allowed_rule_intents = ["reasoning-search"]
"#,
    )
    .expect("write route registry");
    std::fs::write(
        root.path().join("custom_codex.toml"),
        r#"name = "codex_probe"
description = "Codex evidence probe."
model = "gpt-5.6-luna"
model_reasoning_effort = "low"
sandbox_mode = "read-only"
developer_instructions = "Run Codex evidence queries."
"#,
    )
    .expect("write Codex projection");
    std::fs::write(
        root.path().join("custom_claude.md"),
        r#"---
name: claude-probe
description: Claude evidence probe.
permissionMode: plan
---
Run Claude evidence queries.
"#,
    )
    .expect("write stale Claude projection");

    let loaded = load_agent_route_registry_for_platform(&root.path().join("config.toml"), "codex")
        .expect("Codex load must be isolated from Claude artifacts");
    assert!(
        loaded
            .compile_route_for_platform_host_agent_name("codex", "codex_probe")
            .expect("Codex route lookup")
            .is_some()
    );
    assert!(
        load_agent_route_registry(&root.path().join("config.toml"))
            .expect_err("full publication gate must still validate Claude")
            .contains("permissionMode")
    );
}

#[test]
fn platform_matcher_rejects_noncanonical_globs() {
    let error = super::parse_agent_route_registry(
        r#"schema_id = "agent.semantic-protocols.agent-route-registry"
schema_version = 1
[platforms.codex]
matcher = "*.toml"
[agents.probe]
session_lifetime = "resident"
roles = ["explore"]
allowed_rule_intents = ["reasoning-search"]
"#,
        "noncanonical matcher fixture",
    )
    .expect_err("broad platform matcher must fail closed");
    assert!(error.contains("must be exactly `*_codex.toml`"));
}
