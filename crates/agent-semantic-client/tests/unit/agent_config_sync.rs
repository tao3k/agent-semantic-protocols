// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

#[test]
fn embedded_registry_sync_replaces_stale_state_catalog_without_project_agents_directory() {
    let root = tempfile::tempdir().expect("agent config sync fixture");
    let state_home = root.path().join("state");
    let codex_home = root.path().join("codex");
    let state_agents = agent_semantic_artifacts::StateHomeLayout::new(&state_home)
        .control()
        .agent_registry_root();
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
    let downstream_workspace = root.path().join("downstream-without-agent-config");
    std::fs::create_dir_all(&downstream_workspace).expect("downstream workspace fixture");
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_asp"))
        .args(["config", "agents", "sync"])
        .current_dir(&downstream_workspace)
        .env("ASP_STATE_HOME", &state_home)
        .env("CODEX_HOME", &codex_home)
        .output()
        .expect("run embedded agent config sync");
    assert!(
        output.status.success(),
        "stdout={} stderr={}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let receipt: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("decode agent config sync receipt");
    let embedded_catalog = agent_semantic_config::embedded_agent_assets::embedded_agent_assets()
        .iter()
        .find(|asset| asset.file_name == "config.toml")
        .expect("embedded agent registry catalog");

    assert_eq!(
        std::fs::read(state_agents.join("config.toml")).unwrap(),
        embedded_catalog.contents
    );
    assert!(state_agents.join("asp_explorer_codex.toml").is_file());
    assert!(!state_agents.join("asp-explorer_claude.md").exists());
    assert!(state_agents.join("user-notes.txt").is_file());
    assert!(codex_home.join("agents/asp_explorer.toml").is_file());
    assert_eq!(receipt["schemaVersion"], "1");
    assert_eq!(receipt["catalogAuthority"], "embedded-asp-binary");
    assert!(receipt.get("projectCatalog").is_none());
    assert_eq!(
        receipt["removedStaleProfiles"],
        serde_json::json!(["asp-explorer_claude.md"])
    );
}
