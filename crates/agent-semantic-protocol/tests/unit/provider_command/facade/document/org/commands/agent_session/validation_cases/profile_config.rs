use crate::provider_command::facade::document::org::commands::agent_session::support::{
    write_codex_asp_explorer_fixture_with_actual_agent_path,
    write_codex_asp_explorer_fixture_with_actual_profile,
    write_codex_asp_explorer_fixture_with_default_agent_role,
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
            "--roles",
            "subagent,search",
            "--json",
        ])
        .output()
        .expect("warn on mismatched codex model profile");
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).expect("model mismatch stdout");
    assert!(
        stdout.contains("child-session runtime model drift observed: host observed gpt-5.5"),
        "{stdout}"
    );
    assert!(
        stdout.contains(
            "requiredAction=main-agent-followup-existing-child-with-natural-language-runtime-switch"
        ),
        "{stdout}"
    );
    assert!(
        stdout.contains("runtimeEvidence=fresh-host-observation"),
        "{stdout}"
    );

    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn asp_agent_session_rejects_placeholder_native_message_target() {
    let root = temp_project_root("agent-command-session-placeholder-target");
    let home = root.join("home");
    let agents_dir = home.join(".codex").join("agents");
    std::fs::create_dir_all(&agents_dir).expect("create codex agents dir");
    std::fs::write(
        agents_dir.join("asp-explorer.toml"),
        "name = \"asp_explorer\"\nmodel = \"gpt-5.3-codex-spark\"\nsandbox_mode = \"read-only\"\nsession_lifetime = \"resident\"\n",
    )
    .expect("write asp explorer config");

    let output = asp_command(&root)
        .env("HOME", &home)
        .env("CODEX_HOME", home.join(".codex"))
        .env("CODEX_THREAD_ID", "codex-root-thread")
        .args([
            "agent",
            "session",
            "register",
            "--name",
            "asp-explore",
            "--child-session-id",
            "019f0000-0000-0000-0000-000000000001",
            "--message-target-id",
            "asp_host_dummy",
            "--root-session-id",
            "codex-root-thread",
            "--roles",
            "subagent,search",
            "--model",
            "gpt-5.3-codex-spark",
            "--json",
        ])
        .output()
        .expect("reject placeholder native target");
    assert!(
        !output.status.success(),
        "stdout: {} stderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let combined = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        combined
            .contains("Codex rollout metadata not found for child session `019f0000-0000-0000-0000-000000000001`"),
        "{combined}"
    );
    assert!(
        combined.contains("blockedState=validation-failed-or-non-routable-child"),
        "{combined}"
    );

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
            "--roles",
            "subagent,search",
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
            "--roles",
            "subagent,search",
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
fn asp_agent_session_rejects_codex_default_role_for_typed_resident() {
    let root = temp_project_root("agent-command-session-codex-default-role-fallback");
    let home = root.join("home");
    write_codex_asp_explorer_fixture_with_default_agent_role(
        &home,
        "codex-root-thread",
        "codex-child-thread",
        "gpt-5.4-mini",
        "gpt-5.4-mini",
        "read-only",
        "danger-full-access",
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
            "--roles",
            "subagent",
            "--model",
            "gpt-5.4-mini",
            "--json",
        ])
        .output()
        .expect("reject codex default role fallback");
    assert!(
        !output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stderr = String::from_utf8(output.stderr).expect("default role rejection stderr");
    assert!(
        stderr.contains("agentRole expected asp_explorer got default"),
        "{stderr}"
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
            "--roles",
            "subagent,search",
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
