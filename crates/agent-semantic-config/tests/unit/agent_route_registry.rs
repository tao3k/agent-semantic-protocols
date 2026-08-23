use super::{AgentSessionLifetime, compile_agent_route, load_agent_route_registry};

fn canonical_registry_path() -> std::path::PathBuf {
    std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../agents/config.toml")
}

#[test]
fn canonical_registry_compiles_host_routes() {
    let loaded = load_agent_route_registry(&canonical_registry_path()).expect("canonical registry");
    let (explorer_key, _) = loaded
        .registry
        .unique_route_for_role("explore")
        .expect("unique explore route");
    assert_eq!(explorer_key, "asp_explorer");
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
    assert_eq!(claude.platform_host_agent_name.as_str(), "asp_explorer");
    assert_eq!(codex.model.as_deref(), Some("gpt-5.6-luna"));
    assert_eq!(claude.model.as_deref(), Some("haiku"));
    assert!(codex.profile_path.ends_with("asp_explorer_codex.toml"));
    assert!(claude.profile_path.ends_with("asp_explorer_claude.md"));
    assert_eq!(codex.roles, vec!["explore", "subagent"]);
    assert_eq!(codex.agent_kind, "Subagent");
    assert_eq!(codex.display_role, "Evidence Explorer");
    assert_eq!(codex.description, "for code and evidence search");
    assert_eq!(claude.description, codex.description);
    assert_eq!(codex.sandbox_mode.as_deref(), Some("read-only"));
    let testing =
        compile_agent_route(&loaded, "asp_testing", "codex").expect("Codex testing route");
    assert_eq!(testing.platform_host_agent_name.as_str(), "asp_testing");
    assert_eq!(testing.focus_mode, super::AgentFocusMode::Leaf);
    assert_eq!(testing.agent_kind, "Subagent");
    assert_eq!(testing.display_role, "Test Runner");
    assert_eq!(testing.description, "for build and test jobs");
    assert!(testing.profile_path.ends_with("asp_testing_codex.toml"));
    assert_eq!(testing.sandbox_mode.as_deref(), Some("read-only"));
}

#[test]
fn ambiguous_role_owner_fails_closed() {
    let registry = super::parse_agent_route_registry(
        r#"schema_id = "agent.semantic-protocols.agent-route-registry"
schema_version = 1

[platforms.codex]
matcher = "*_codex.toml"

[agents.first]
session_lifetime = "resident"
roles = ["explore"]

[agents.second]
session_lifetime = "resident"
roles = ["explore"]
"#,
        "ambiguous registry fixture",
    )
    .expect("parse registry");

    let error = registry
        .unique_route_for_role("explore")
        .expect_err("ambiguous role owner must fail closed");
    assert!(error.contains("multiple routes"));
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
"#,
    )
    .expect("write matcher registry");
    std::fs::write(
        root.path().join("custom_codex.toml"),
        r#"name = "codex_probe"
description = "Codex evidence probe."
model = "gpt-5.6-luna"
sandbox_mode = "read-only"
developer_instructions = "Run Codex evidence queries."
"#,
    )
    .expect("write Codex matcher profile");
    std::fs::write(
        root.path().join("custom_claude.md"),
        r#"---
name: claude_probe
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
        "claude_probe"
    );
    assert!(
        loaded
            .compile_route_for_platform_host_agent_name("codex", "claude_probe")
            .expect("platform exact lookup")
            .is_none()
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
"#,
        "noncanonical matcher fixture",
    )
    .expect_err("broad platform matcher must fail closed");
    assert!(error.contains("must be exactly `*_codex.toml`"));
}
