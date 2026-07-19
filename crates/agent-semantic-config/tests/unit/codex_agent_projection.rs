use std::fs;

use agent_semantic_config::codex_agent_projection::{
    update_asp_codex_agent_sources_and_symlink_projections, write_codex_dynamic_model,
};

#[test]
#[cfg(unix)]
fn codex_agent_projection_is_symlink_and_does_not_truncate_source() {
    let temp = tempfile::tempdir().expect("tempdir");
    let asp_agents = temp.path().join("asp-agents");
    let codex_agents = temp.path().join("codex-agents");
    fs::create_dir_all(&asp_agents).expect("create asp agents");
    let source = asp_agents.join("asp-explorer_codex.toml");
    fs::write(
        &source,
        "name = \"asp_explorer\"\nmodel = \"gpt-5.4-mini\"\nsandbox_mode = \"read-only\"\n",
    )
    .expect("write source");
    fs::create_dir_all(&codex_agents).expect("create codex agents");
    std::os::unix::fs::symlink(&source, codex_agents.join("asp-explorer.toml"))
        .expect("seed symlink");

    let mut updated = Vec::new();
    update_asp_codex_agent_sources_and_symlink_projections(
        &asp_agents,
        &codex_agents,
        "gpt-5.5",
        &mut updated,
    )
    .expect("update projection");

    let source_text = fs::read_to_string(&source).expect("read source");
    assert!(source_text.contains("model = \"gpt-5.5\""));
    assert!(source_text.contains("sandbox_mode = \"read-only\""));
    let projection = codex_agents.join("asp-explorer.toml");
    assert_eq!(fs::read_link(&projection).expect("read link"), source);
}

#[test]
#[cfg(unix)]
fn codex_agent_projection_adds_read_only_sandbox_when_missing() {
    let temp = tempfile::tempdir().expect("tempdir");
    let asp_agents = temp.path().join("asp-agents");
    let codex_agents = temp.path().join("codex-agents");
    fs::create_dir_all(&asp_agents).expect("create asp agents");
    let source = asp_agents.join("asp-explorer_codex.toml");
    fs::write(
        &source,
        "name = \"asp_explorer\"\nmodel = \"gpt-5.4-mini\"\n",
    )
    .expect("write source");

    let mut updated = Vec::new();
    update_asp_codex_agent_sources_and_symlink_projections(
        &asp_agents,
        &codex_agents,
        "gpt-5.4-mini",
        &mut updated,
    )
    .expect("update projection");

    let source_text = fs::read_to_string(&source).expect("read source");
    assert!(source_text.contains("sandbox_mode = \"read-only\""));
    assert_eq!(
        fs::read_link(codex_agents.join("asp-explorer.toml")).expect("read link"),
        source
    );
}

#[test]
#[cfg(unix)]
fn codex_agent_projection_removes_asp_only_session_lifetime() {
    let temp = tempfile::tempdir().expect("tempdir");
    let asp_agents = temp.path().join("asp-agents");
    let codex_agents = temp.path().join("codex-agents");
    fs::create_dir_all(&asp_agents).expect("create asp agents");
    let source = asp_agents.join("asp-explorer_codex.toml");
    fs::write(
        &source,
        "name = \"asp_explorer\"\nmodel = \"gpt-5.4-mini\"\nmodel_reasoning_effort = \"low\"\nsession_lifetime = \"resident\"\n",
    )
    .expect("write source");

    let mut updated = Vec::new();
    update_asp_codex_agent_sources_and_symlink_projections(
        &asp_agents,
        &codex_agents,
        "gpt-5.4-mini",
        &mut updated,
    )
    .expect("update projection");

    let source_text = fs::read_to_string(&source).expect("read source");
    assert!(!source_text.contains("session_lifetime"));
    assert!(source_text.contains("sandbox_mode = \"read-only\""));
    assert!(
        source_text.contains("model_reasoning_effort = \"low\""),
        "dynamic model projection must preserve the managed child reasoning profile"
    );
    assert_eq!(
        fs::read_link(codex_agents.join("asp-explorer.toml")).expect("read link"),
        source
    );
}

#[test]
fn codex_dynamic_model_writes_managed_agent_session_names() {
    let temp = tempfile::tempdir().expect("tempdir");
    let config_path = temp.path().join("agents/config.toml");

    write_codex_dynamic_model(&config_path, "gpt-5.4-mini").expect("write dynamic model");

    let text = fs::read_to_string(&config_path).expect("read config");
    let value: toml::Value = toml::from_str(&text).expect("parse config");
    assert_eq!(
        value["platform"]["codex"]["models"]["primary"].as_str(),
        Some("gpt-5.4-mini")
    );
    assert_eq!(
        value["agents"]["asp_explorer"]["session_name"].as_str(),
        Some("asp-explore")
    );
    assert_eq!(
        value["agents"]["asp_explorer"]["session_lifetime"].as_str(),
        Some("resident")
    );
    assert_eq!(
        value["agents"]["asp_testing"]["session_name"].as_str(),
        Some("asp-testing")
    );
}
