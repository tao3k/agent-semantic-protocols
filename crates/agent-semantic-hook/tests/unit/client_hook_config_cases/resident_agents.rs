use super::common::{fs, load_client_config, temp_root};

#[test]
fn missing_resident_agent_route_is_rejected_without_a_compatibility_fallback() {
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

    let error = load_client_config(&config_path)
        .expect_err("missing configured residents must fail closed");
    assert!(
        error.contains("unavailable resident `asp-explore`"),
        "{error}"
    );
    let _ = fs::remove_dir_all(root);
}
