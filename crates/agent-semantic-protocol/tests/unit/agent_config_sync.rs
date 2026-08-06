use super::synchronize_agent_config_to_roots;

#[test]
fn runtime_reconcile_projection_replaces_stale_state_catalog() {
    let root = tempfile::tempdir().expect("agent config sync fixture");
    let state_home = root.path().join("state");
    let codex_home = root.path().join("codex");
    let state_agents = state_home.join("agents");
    std::fs::create_dir_all(&state_agents).expect("state agents fixture");
    std::fs::write(
        state_agents.join("config.toml"),
        "schema_id = \"stale\"\nschema_version = 1\n",
    )
    .expect("write stale state catalog");
    std::fs::write(
        state_agents.join("asp-explorer_claude.md"),
        "legacy profile",
    )
    .expect("write stale matched profile");
    std::fs::write(state_agents.join("user-notes.txt"), "preserve me")
        .expect("write unrelated state file");
    std::fs::create_dir_all(codex_home.join("agents")).expect("Codex agents fixture");
    let project_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");

    let receipt =
        synchronize_agent_config_to_roots(&project_root, &state_home, &codex_home).unwrap();

    assert_eq!(
        std::fs::read(state_agents.join("config.toml")).unwrap(),
        std::fs::read(project_root.join("agents/config.toml")).unwrap()
    );
    assert!(state_agents.join("asp_explorer_codex.toml").is_file());
    assert!(!state_agents.join("asp-explorer_claude.md").exists());
    assert!(state_agents.join("user-notes.txt").is_file());
    assert!(codex_home.join("agents/asp_explorer.toml").is_file());
    assert_eq!(receipt["schemaVersion"], "1");
    assert_eq!(
        receipt["removedStaleProfiles"],
        serde_json::json!(["asp-explorer_claude.md"])
    );
}
