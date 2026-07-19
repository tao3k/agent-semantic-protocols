use agent_semantic_config::codex_config_plugin_enabled;

#[test]
fn codex_plugin_enabled_is_owned_by_config_parser() {
    let root = tempfile::tempdir().expect("tempdir");
    let config = root.path().join("config.toml");
    std::fs::write(
        &config,
        "[plugins.\"asp-codex-plugin@asp-project\"]\nenabled = true\n",
    )
    .expect("write config");

    assert!(
        codex_config_plugin_enabled(&config, "asp-codex-plugin@asp-project").expect("parse config")
    );
    assert!(!codex_config_plugin_enabled(&config, "different@plugin").expect("parse config"));
    assert!(
        !codex_config_plugin_enabled(&root.path().join("missing.toml"), "anything")
            .expect("missing config is disabled")
    );
}
