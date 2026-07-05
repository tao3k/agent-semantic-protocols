use super::support::{
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
        stdout.contains("model mismatch: this same Codex child is running gpt-5.5"),
        "{stdout}"
    );
    assert!(
        stdout.contains("requiredAction=parent-send-message-same-child-with-required-model"),
        "{stdout}"
    );
    assert!(stdout.contains("requiresAgentMessageTargetId=true"), "{stdout}");

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

[[agents.residentAgents]]
enabled = true
name = "asp-explore"
role = "asp_explorer"
roles = ["subagent", "search"]
permissions = ["read-only"]
codexAgentName = "asp_explorer"
lifecycle = "asp-command"
sessionLifetime = "resident"
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
            "--roles",
            "subagent,search",
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

    let status_output = asp_command(&root)
        .env("HOME", &home)
        .env("CODEX_HOME", home.join(".codex"))
        .args([
            "agent",
            "session",
            "status",
            "--name",
            "asp-explore",
            "--root-session-id",
            "codex-root-thread",
            "--json",
        ])
        .output()
        .expect("status reads resident session lifetime");
    assert!(
        status_output.status.success(),
        "{}",
        String::from_utf8_lossy(&status_output.stderr)
    );
    let status_stdout = String::from_utf8(status_output.stdout).expect("status stdout");
    let status_json: serde_json::Value =
        serde_json::from_str(&status_stdout).expect("parse status json");
    assert_eq!(
        status_json["sessionLifetime"].as_str(),
        Some("resident"),
        "{status_stdout}"
    );
    assert_eq!(
        status_json["resident"].as_bool(),
        Some(true),
        "{status_stdout}"
    );
    assert_eq!(
        status_json["sessionLifetimeSource"].as_str(),
        Some("agent-config"),
        "{status_stdout}"
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
fn asp_agent_session_allows_codex_default_role_as_host_fallback() {
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
        .expect("allow codex default role fallback");
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).expect("default role fallback stdout");
    assert!(stdout.contains("\"status\": \"passed\""), "{stdout}");
    assert!(
        stdout.contains("agentRole default accepted as Codex host role fallback"),
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

#[test]
fn asp_agent_session_model_mismatch_is_warning_not_invalid() {
    let root = temp_project_root("agent-command-session-codex-model-mismatch-warning");
    let home = root.join("home");
    write_codex_asp_explorer_fixture_without_agent_path(
        &home,
        "codex-root-thread",
        "codex-child-thread",
        "gpt-5.4-mini",
        "gpt-5.5",
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
        .expect("register with codex model mismatch");
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).expect("model mismatch stdout");
    assert!(stdout.contains("\"status\": \"warning\""), "{stdout}");
    assert!(
        stdout.contains("requiredAction=parent-send-message-same-child-with-required-model"),
        "{stdout}"
    );
    assert!(
        stdout.contains("main/parent agent must send an agent message to the same managed child"),
        "{stdout}"
    );
    assert!(stdout.contains("requiresAgentMessageTargetId=true"), "{stdout}");
    assert!(
        stdout.contains("bootstrapBlocked=host-message-agent-target-unavailable"),
        "{stdout}"
    );
    assert!(
        stdout.contains("do not create or replace the child"),
        "{stdout}"
    );
    assert!(!stdout.contains("\"status\": \"failed\""), "{stdout}");

    let status_output = asp_command(&root)
        .env("HOME", &home)
        .args([
            "agent",
            "session",
            "status",
            "--name",
            "asp-explore",
            "--json",
        ])
        .output()
        .expect("status with codex model mismatch");
    assert!(
        status_output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&status_output.stderr)
    );
    let status_stdout = String::from_utf8(status_output.stdout).expect("model mismatch status");
    assert!(
        status_stdout.contains("\"validationStatus\": \"warning\""),
        "{status_stdout}"
    );
    assert!(
        status_stdout.contains("\"routable\": true"),
        "{status_stdout}"
    );
    assert!(
        status_stdout.contains("\"nextAction\": \"child-activity-running-wait\""),
        "{status_stdout}"
    );

    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn asp_agent_session_reads_codex_agent_file_session_lifetime() {
    let root = temp_project_root("agent-command-session-codex-agent-file-lifetime");
    let home = root.join("home");
    write_codex_asp_explorer_fixture_with_actual_profile(
        &home,
        "codex-root-thread",
        "codex-child-thread",
        "gpt-5.3-codex-spark",
        "gpt-5.3-codex-spark",
        "read-only",
        "read-only",
    );

    let agents_dir = home.join(".agent-semantic-protocols").join("agents");
    std::fs::create_dir_all(&agents_dir).expect("create ASP agents dir");
    std::fs::write(
        agents_dir.join("asp-explorer_codex.toml"),
        r#"name = "asp_explorer"
model = "gpt-5.3-codex-spark"
sandbox_mode = "read-only"
session_lifetime = "resident"
"#,
    )
    .expect("write codex agent file");

    let output = asp_command(&root)
        .env("HOME", &home)
        .env("CODEX_HOME", home.join(".codex"))
        .env("ASP_AGENTS_HOME", &agents_dir)
        .args([
            "agent",
            "session",
            "status",
            "--name",
            "asp-explore",
            "--json",
        ])
        .output()
        .expect("run asp agent session status");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success(), "{stdout}");
    assert!(
        stdout.contains("\"sessionLifetime\": \"resident\""),
        "{stdout}"
    );
    assert!(stdout.contains("\"resident\": true"), "{stdout}");
    assert!(
        stdout.contains("\"sessionLifetimeSource\": \"agent-file\""),
        "{stdout}"
    );

    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn asp_agent_session_register_keeps_drifted_existing_child_on_model_warning() {
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
            "--roles",
            "subagent,search",
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
            "--roles",
            "subagent,search",
            "--json",
        ])
        .output()
        .expect("recover drifted child session");
    assert!(
        replacement.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&replacement.stderr)
    );
    let stdout = String::from_utf8(replacement.stdout).expect("replacement stdout");
    assert!(stdout.contains("drifted-child-thread"), "{stdout}");
    assert!(!stdout.contains("replacement-child-thread"), "{stdout}");
    assert!(
        stdout.contains("requiredAction=parent-send-message-same-child-with-required-model"),
        "{stdout}"
    );
    assert!(stdout.contains("requiresAgentMessageTargetId=true"), "{stdout}");

    let _ = std::fs::remove_dir_all(root);
}
