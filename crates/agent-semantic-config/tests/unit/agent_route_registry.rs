use super::{
    AgentSessionLifetime, compile_agent_route, load_agent_route_registry,
    render_hook_agent_routes,
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
fn canonical_registry_compiles_host_routes() {
    let loaded = load_agent_route_registry(&canonical_registry_path()).expect("canonical registry");
    let codex = compile_agent_route(&loaded, "asp_explorer", "codex").expect("Codex route");
    let claude = compile_agent_route(&loaded, "asp_explorer", "claude").expect("Claude route");
    assert_eq!(codex.route_key.as_str(), "asp_explorer");
    assert_eq!(claude.route_key.as_str(), "asp_explorer");
    assert_eq!(codex.session_name.as_str(), "asp_explorer");
    assert_eq!(claude.session_name.as_str(), "asp_explorer");
    assert_eq!(codex.session_lifetime, AgentSessionLifetime::Resident);
    assert_eq!(codex.platform.as_str(), "codex");
    assert_eq!(claude.platform.as_str(), "claude");
    assert_eq!(codex.platform_host_agent_name.as_str(), "asp_explorer");
    assert_eq!(claude.platform_host_agent_name.as_str(), "asp_explorer");
    assert_eq!(codex.model.as_deref(), Some("gpt-5.6-luna"));
    assert_eq!(claude.model.as_deref(), Some("haiku"));
    assert_eq!(codex.projection, "asp-explorer_codex.toml");
    assert_eq!(claude.projection, "asp-explorer_claude.md");
    assert_eq!(codex.roles, vec!["explore", "subagent"]);
}

#[test]
fn unknown_platform_projection_fields_fail_closed() {
    let temp = tempfile::tempdir().expect("tempdir");
    let path = temp.path().join("config.toml");
    std::fs::write(
        &path,
        r#"schema_id = "agent.semantic-protocols.agent-route-registry"
schema_version = 1

[agents.bad]
session_name = "bad"
session_lifetime = "resident"
roles = ["subagent"]

[agents.bad.platforms.codex]
projection = "bad_codex.toml"
manager_projection = "legacy-shadow"
"#,
    )
    .expect("write invalid route registry");
    let error = load_agent_route_registry(&path)
        .expect_err("unknown platform projection field must fail closed");
    assert!(error.contains("unknown field `manager_projection`"));
}

#[test]
fn unknown_platform_fails_closed() {
    let loaded = load_agent_route_registry(&canonical_registry_path()).expect("canonical registry");
    let error = compile_agent_route(&loaded, "asp_testing", "unknown-host")
        .expect_err("unknown platform must fail closed");
    assert!(error.contains("does not define platform `unknown-host`"));
}
