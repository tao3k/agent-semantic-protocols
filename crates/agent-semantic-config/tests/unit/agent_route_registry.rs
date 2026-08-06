use super::{
    AgentSessionLifetime, compile_agent_route, load_agent_route_registry, render_hook_agent_routes,
};

fn canonical_registry_path() -> std::path::PathBuf {
    std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../agents/config.toml")
}

#[test]
fn hook_agent_routes_are_derived_from_platform_projection_owners() {
    let agents_root = canonical_registry_path()
        .parent()
        .expect("agents root")
        .to_path_buf();
    let rendered = render_hook_agent_routes(&agents_root).expect("render hook agent routes");
    let projection: toml::Value = toml::from_str(&rendered).expect("parse hook projection");
    assert_eq!(
        projection["agents"]["placeholders"]["explore"].as_str(),
        Some("asp_explorer")
    );
    assert_eq!(
        projection["agents"]["placeholders"]["testing"].as_str(),
        Some("asp_testing")
    );
    let residents = projection["agents"]["residentAgents"]
        .as_array()
        .expect("resident agent projection");
    assert_eq!(residents.len(), 2);
    assert!(residents.iter().any(|resident| {
        resident["name"].as_str() == Some("asp_explorer")
            && resident["role"].as_str() == Some("asp_explorer")
            && resident["codexAgentName"].as_str() == Some("asp_explorer")
    }));
    assert!(residents.iter().any(|resident| {
        resident["name"].as_str() == Some("asp_testing")
            && resident["role"].as_str() == Some("asp_testing")
            && resident["codexAgentName"].as_str() == Some("asp_testing")
    }));
}

#[test]
fn hook_role_placeholder_follows_registry_identity_instead_of_a_rule_literal() {
    let root = tempfile::tempdir().expect("agent registry tempdir");
    std::fs::write(
        root.path().join("config.toml"),
        r#"schema_id = "agent.semantic-protocols.agent-route-registry"
schema_version = 1

[platforms.codex]
matcher = "*_codex.toml"
[platforms.claude]
matcher = "*_claude.md"

[agents.custom_probe]
session_lifetime = "resident"
roles = ["explore"]
"#,
    )
    .expect("write registry");
    std::fs::write(
        root.path().join("custom_probe_codex.toml"),
        r#"name = "custom_probe"
description = "Custom evidence probe."
nickname_candidates = ["Custom Probe"]
model = "gpt-5.6-luna"
model_reasoning_effort = "low"
sandbox_mode = "read-only"
developer_instructions = "Run configured evidence queries."
"#,
    )
    .expect("write Codex profile");
    std::fs::write(
        root.path().join("custom_probe_claude.md"),
        r#"---
name: custom_probe
description: Custom evidence probe.
tools: Bash
model: haiku
permissionMode: plan
maxTurns: 8
---
Run configured evidence queries.
"#,
    )
    .expect("write Claude profile");

    let rendered = render_hook_agent_routes(root.path()).expect("render custom Hook projection");
    let projection: toml::Value = toml::from_str(&rendered).expect("parse Hook projection");
    assert_eq!(
        projection["agents"]["placeholders"]["explore"].as_str(),
        Some("custom_probe")
    );
    assert!(!rendered.contains("asp_explorer"));
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
    assert_eq!(codex.platform.as_str(), "codex");
    assert_eq!(claude.platform.as_str(), "claude");
    assert_eq!(codex.platform_host_agent_name.as_str(), "asp_explorer");
    assert_eq!(claude.platform_host_agent_name.as_str(), "asp_explorer");
    assert_eq!(codex.model.as_deref(), Some("gpt-5.6-luna"));
    assert_eq!(claude.model.as_deref(), Some("haiku"));
    assert!(codex.profile_path.ends_with("asp_explorer_codex.toml"));
    assert!(claude.profile_path.ends_with("asp_explorer_claude.md"));
    assert_eq!(codex.roles, vec!["explore", "subagent"]);
    assert_eq!(codex.description, "ASP search/query evidence explorer.");
    assert_eq!(claude.description, codex.description);
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
