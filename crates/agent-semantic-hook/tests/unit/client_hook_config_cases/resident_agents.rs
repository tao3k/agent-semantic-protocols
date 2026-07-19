use super::common::{fs, load_client_config, temp_root};

#[test]
fn missing_resident_agent_route_uses_builtin_search_profile_without_load_failure() {
    let root = temp_root("missing-resident-agent-route");
    let config_path = root.join("config.toml");
    fs::write(
        &config_path,
        r#"
[agents]
residentAgents = []
"#,
    )
    .expect("write hook config without resident route");

    let config = load_client_config(&config_path).expect("load with built-in resident fallback");

    assert_eq!(config.resident_asp_explore_child_name(), "asp-explore");
    assert_eq!(
        config.resident_asp_explore_codex_agent_name(),
        "asp_explorer"
    );
    let _ = fs::remove_dir_all(root);
}
