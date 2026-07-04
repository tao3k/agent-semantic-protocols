use super::support::{
    write_codex_asp_explorer_fixture_with_actual_agent_path,
    write_codex_asp_explorer_fixture_with_actual_profile,
    write_codex_asp_explorer_fixture_without_agent_path,
};
use crate::provider_command::support::{asp_command, temp_project_root};

#[test]
fn asp_agent_session_rejects_mismatched_codex_model_profile() {
    let root = temp_project_root("agent-command-session-codex-model-mismatch");
    let home = root.join("home");
    write_codex_asp_explorer_fixture_with_actual_profile(
        &home,
        "codex-root-thread",
        "codex-child-thread",
        "gpt-5.3-codex-spark",
        "gpt-5.5",
        "read-only",
        "read-only",
    );

    let output = asp_command(&root)
        .env("HOME", &home)
        .env("CODEX_HOME", home.join(".codex"))
        .args([
            "agent",
            "session",
            "register",
            "--name",
            "asp-explore",
            "--child-session-id",
            "codex-child-thread",
            "--root-session-id",
            "codex-root-thread",
            "--role",
            "asp-explore",
        ])
        .output()
        .expect("reject mismatched codex model profile");
    assert!(!output.status.success());
    let stderr = String::from_utf8(output.stderr).expect("model mismatch stderr");
    assert!(
        stderr.contains(
            "agent session validation failed: model expected gpt-5.3-codex-spark got gpt-5.5"
        ),
        "{stderr}"
    );

    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn asp_agent_session_accepts_codex_fallback_model_from_agents_config() {
    let root = temp_project_root("agent-command-session-codex-model-fallback");
    let home = root.join("home");
    write_codex_asp_explorer_fixture_with_actual_profile(
        &home,
        "codex-root-thread",
        "codex-child-thread",
        "gpt-5.3-codex-spark",
        "gpt-5.4-mini",
        "read-only",
        "read-only",
    );
    let agents_dir = home.join(".agent-semantic-protocols").join("agents");
    std::fs::create_dir_all(&agents_dir).expect("create ASP agents dir");
    std::fs::write(
        agents_dir.join("config.toml"),
        r#"[platform.codex.models]
primary = "gpt-5.3-codex-spark"
fallback = ["gpt-5.4-mini"]
capacityThreshold = 0.8
"#,
    )
    .expect("write ASP agents config");

    let output = asp_command(&root)
        .env("HOME", &home)
        .env("CODEX_HOME", home.join(".codex"))
        .args([
            "agent",
            "session",
            "register",
            "--name",
            "asp-explore",
            "--child-session-id",
            "codex-child-thread",
            "--root-session-id",
            "codex-root-thread",
            "--role",
            "asp-explore",
            "--json",
        ])
        .output()
        .expect("run asp agent session register");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(output.status.success(), "{stderr}");
    assert!(
        stdout.contains("model switched to configured fallback gpt-5.4-mini"),
        "{stdout}"
    );
    assert!(
        !stdout.contains("\\\"status\\\":\\\"warning\\\"")
            && !stdout.contains("\\\"status\\\":\\\"failed\\\""),
        "{stdout}"
    );

    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn asp_agent_session_switch_model_updates_codex_dynamic_config_and_agent_projection() {
    let root = temp_project_root("agent-command-session-codex-switch-model");
    let home = root.join("home");
    let asp_agents_dir = home.join(".agent-semantic-protocols").join("agents");
    std::fs::create_dir_all(&asp_agents_dir).expect("create ASP agents dir");
    std::fs::write(
        asp_agents_dir.join("config.toml"),
        r#"[platform.codex.models]
primary = "gpt-5.3-codex-spark"
fallback = ["gpt-5.4-mini"]
capacityThreshold = 0.8
"#,
    )
    .expect("write ASP agents config");
    std::fs::write(
        asp_agents_dir.join("asp-explorer_codex.toml"),
        "name = \"asp_explorer\"\nmodel = \"gpt-5.3-codex-spark\"\nsandbox_mode = \"read-only\"\n",
    )
    .expect("write ASP explorer config");
    std::fs::write(
        asp_agents_dir.join("asp-testing_codex.toml"),
        "name = \"asp_testing\"\nmodel = \"gpt-5.3-codex-spark\"\nsandbox_mode = \"read-only\"\n",
    )
    .expect("write ASP testing config");

    let codex_agents_dir = home.join(".codex").join("agents");
    std::fs::create_dir_all(&codex_agents_dir).expect("create Codex agents dir");
    std::fs::write(
        codex_agents_dir.join("asp-explorer.toml"),
        "name = \"asp_explorer\"\nmodel = \"gpt-5.3-codex-spark\"\nsandbox_mode = \"read-only\"\n",
    )
    .expect("write projected ASP explorer config");
    std::fs::write(
        codex_agents_dir.join("unrelated.toml"),
        "name = \"unrelated\"\nmodel = \"gpt-5.3-codex-spark\"\n",
    )
    .expect("write unrelated Codex config");

    let output = asp_command(&root)
        .env("HOME", &home)
        .env("CODEX_HOME", home.join(".codex"))
        .env("CODEX_THREAD_ID", "codex-root-thread")
        .args([
            "agent",
            "session",
            "switch-model",
            "--model",
            "gpt-5.4-mini",
            "--json",
        ])
        .output()
        .expect("run asp agent session switch-model");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(output.status.success(), "{stderr}");
    assert!(stdout.contains("\"status\":\"switched\""), "{stdout}");
    assert!(stdout.contains("\"platform\":\"codex\""), "{stdout}");

    let dynamic_config =
        std::fs::read_to_string(asp_agents_dir.join("config.toml")).expect("read dynamic config");
    assert!(dynamic_config.contains("primary = \"gpt-5.4-mini\""));
    assert!(dynamic_config.contains("fallback = [\"gpt-5.4-mini\"]"));
    let asp_explorer = std::fs::read_to_string(asp_agents_dir.join("asp-explorer_codex.toml"))
        .expect("read ASP explorer config");
    assert!(asp_explorer.contains("model = \"gpt-5.4-mini\""));
    let asp_testing = std::fs::read_to_string(asp_agents_dir.join("asp-testing_codex.toml"))
        .expect("read ASP testing config");
    assert!(asp_testing.contains("model = \"gpt-5.4-mini\""));
    let codex_explorer = std::fs::read_to_string(codex_agents_dir.join("asp-explorer.toml"))
        .expect("read projected ASP explorer config");
    assert!(codex_explorer.contains("model = \"gpt-5.4-mini\""));
    let codex_testing = std::fs::read_to_string(codex_agents_dir.join("asp-testing.toml"))
        .expect("read projected ASP testing config");
    assert!(codex_testing.contains("model = \"gpt-5.4-mini\""));
    let unrelated =
        std::fs::read_to_string(codex_agents_dir.join("unrelated.toml")).expect("read unrelated");
    assert!(unrelated.contains("model = \"gpt-5.3-codex-spark\""));

    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn asp_agent_session_rejects_mismatched_codex_agent_config_path() {
    let root = temp_project_root("agent-command-session-codex-agent-path-mismatch");
    let home = root.join("home");
    let wrong_agent_path = home.join(".codex").join("agents").join("wrong-agent.toml");
    write_codex_asp_explorer_fixture_with_actual_agent_path(
        &home,
        "codex-root-thread",
        "codex-child-thread",
        "gpt-5.3-codex-spark",
        "gpt-5.3-codex-spark",
        "read-only",
        "read-only",
        Some(&wrong_agent_path),
    );

    let output = asp_command(&root)
        .env("HOME", &home)
        .args([
            "agent",
            "session",
            "register",
            "--name",
            "asp-explore",
            "--child-session-id",
            "codex-child-thread",
            "--root-session-id",
            "codex-root-thread",
            "--role",
            "asp-explore",
        ])
        .output()
        .expect("reject mismatched codex agent config path");
    assert!(!output.status.success());
    let stderr = String::from_utf8(output.stderr).expect("agent path mismatch stderr");
    assert!(
        stderr.contains("agent session validation failed: agentPath expected"),
        "{stderr}"
    );
    assert!(stderr.contains("wrong-agent.toml"), "{stderr}");

    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn asp_agent_session_warns_but_allows_missing_codex_agent_config_path() {
    let root = temp_project_root("agent-command-session-codex-agent-path-missing");
    let home = root.join("home");
    write_codex_asp_explorer_fixture_without_agent_path(
        &home,
        "codex-root-thread",
        "codex-child-thread",
        "gpt-5.3-codex-spark",
        "gpt-5.3-codex-spark",
        "read-only",
        "read-only",
    );

    let output = asp_command(&root)
        .env("HOME", &home)
        .args([
            "agent",
            "session",
            "register",
            "--name",
            "asp-explore",
            "--child-session-id",
            "codex-child-thread",
            "--root-session-id",
            "codex-root-thread",
            "--role",
            "asp-explore",
            "--json",
        ])
        .output()
        .expect("allow missing codex agent config path as warning");
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).expect("missing agent path stdout");
    assert!(stdout.contains("\"status\": \"passed\""), "{stdout}");
    assert!(
        stdout.contains("agentPath missing in rollout; validating against expected config"),
        "{stdout}"
    );

    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn asp_agent_session_validation_prefers_canonical_codex_agent_config() {
    let root = temp_project_root("agent-command-session-codex-canonical-config");
    let home = root.join("home");
    write_codex_asp_explorer_fixture_without_agent_path(
        &home,
        "codex-root-thread",
        "codex-child-thread",
        "gpt-5.5",
        "gpt-5.3-codex-spark",
        "read-only",
        "read-only",
    );
    let canonical_config = home
        .join(".agent-semantic-protocols")
        .join("agents")
        .join("asp-explorer_codex.toml");
    std::fs::create_dir_all(canonical_config.parent().expect("canonical parent"))
        .expect("create canonical agents dir");
    std::fs::write(
        &canonical_config,
        "name = \"asp_explorer\"\nmodel = \"gpt-5.3-codex-spark\"\nsandbox_mode = \"read-only\"\n",
    )
    .expect("write canonical codex agent config");

    let output = asp_command(&root)
        .env("HOME", &home)
        .args([
            "agent",
            "session",
            "register",
            "--name",
            "asp-explore",
            "--child-session-id",
            "codex-child-thread",
            "--root-session-id",
            "codex-root-thread",
            "--role",
            "asp-explore",
            "--json",
        ])
        .output()
        .expect("register with canonical codex agent config");
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).expect("canonical config stdout");
    assert!(
        stdout.contains("asp-explorer_codex.toml"),
        "expected canonical config path, got {stdout}"
    );
    assert!(stdout.contains("\"status\": \"passed\""), "{stdout}");
    assert!(
        stdout.contains("agentPath missing in rollout; validating against expected config"),
        "{stdout}"
    );

    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn asp_agent_session_register_replaces_drifted_existing_child() {
    let root = temp_project_root("agent-command-session-replace-drifted-child");
    let home = root.join("home");
    write_codex_asp_explorer_fixture_with_actual_profile(
        &home,
        "codex-root-thread",
        "drifted-child-thread",
        "gpt-5.3-codex-spark",
        "gpt-5.3-codex-spark",
        "read-only",
        "read-only",
    );
    write_codex_asp_explorer_fixture_without_agent_path(
        &home,
        "codex-root-thread",
        "replacement-child-thread",
        "gpt-5.3-codex-spark",
        "gpt-5.3-codex-spark",
        "read-only",
        "read-only",
    );

    let drifted = asp_command(&root)
        .env("HOME", &home)
        .args([
            "agent",
            "session",
            "register",
            "--name",
            "asp-explore",
            "--child-session-id",
            "drifted-child-thread",
            "--root-session-id",
            "codex-root-thread",
            "--role",
            "asp-explore",
        ])
        .output()
        .expect("register initially valid child");
    assert!(
        drifted.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&drifted.stderr)
    );
    write_codex_asp_explorer_fixture_with_actual_profile(
        &home,
        "codex-root-thread",
        "drifted-child-thread",
        "gpt-5.3-codex-spark",
        "gpt-5.5",
        "read-only",
        "read-only",
    );

    let replacement = asp_command(&root)
        .env("HOME", &home)
        .args([
            "agent",
            "session",
            "register",
            "--name",
            "asp-explore",
            "--child-session-id",
            "replacement-child-thread",
            "--root-session-id",
            "codex-root-thread",
            "--role",
            "asp-explore",
            "--json",
        ])
        .output()
        .expect("replace drifted child session");
    assert!(
        replacement.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&replacement.stderr)
    );
    let stdout = String::from_utf8(replacement.stdout).expect("replacement stdout");
    assert!(stdout.contains("replacement-child-thread"), "{stdout}");
    assert!(!stdout.contains("drifted-child-thread"), "{stdout}");

    let _ = std::fs::remove_dir_all(root);
}
