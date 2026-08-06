use crate::provider_command::facade::document::org::commands::agent_session::support::{
    install_rust_marker_provider, write_codex_asp_explorer_fixture_with_actual_profile,
    write_codex_asp_explorer_fixture_without_agent_path,
};
use crate::provider_command::support::{asp_command, state_home, temp_project_root};

#[test]
fn asp_agent_session_treats_configured_fallback_model_as_non_ready_drift() {
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
name = "asp_explorer"
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
            "asp_explorer",
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
    assert!(stdout.contains("\"status\": \"warning\""), "{stdout}");
    assert!(
        !stdout.contains("model switched to configured fallback"),
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
            "asp_explorer",
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
    assert_eq!(
        status_json["validationStatus"].as_str(),
        Some("warning"),
        "{status_stdout}"
    );
    assert_eq!(
        status_json["routable"].as_bool(),
        Some(false),
        "{status_stdout}"
    );

    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn asp_agent_session_reads_dynamic_agent_table_session_lifetime() {
    let root = temp_project_root("agent-command-session-dynamic-agent-table-lifetime");
    install_rust_marker_provider(&state_home(&root));
    let home = root.join("home");
    write_codex_asp_explorer_fixture_with_actual_profile(
        &home,
        "codex-root-thread",
        "codex-child-thread",
        "gpt-5.4-mini",
        "gpt-5.4-mini",
        "read-only",
        "read-only",
    );
    let agents_dir = home.join(".agent-semantic-protocols").join("agents");
    std::fs::create_dir_all(&agents_dir).expect("create ASP agents dir");
    std::fs::write(
        agents_dir.join("config.toml"),
        r#"schema_id = "agent.semantic-protocols.agent-route-registry"
schema_version = 1

[platforms.codex]
matcher = "*_codex.toml"

[agents.asp_explorer]
session_lifetime = "resident"
roles = ["subagent", "search"]

"#,
    )
    .expect("write ASP dynamic agents config");

    std::fs::write(
        agents_dir.join("asp_explorer_codex.toml"),
        r#"name = "asp_explorer"
description = "ASP search/query evidence explorer."
model = "gpt-5.4-mini"
model_reasoning_effort = "low"
sandbox_mode = "read-only"
developer_instructions = "Run parser-owned ASP evidence queries."
"#,
    )
    .expect("write dynamic agent Codex profile");

    let sync = asp_command(&root)
        .env("HOME", &home)
        .env("CODEX_HOME", home.join(".codex"))
        .env("ASP_AGENTS_HOME", &agents_dir)
        .env("CODEX_THREAD_ID", "codex-root-thread")
        .arg("sync")
        .output()
        .expect("sync dynamic-agent lifecycle fixture");
    assert!(
        sync.status.success(),
        "{}",
        String::from_utf8_lossy(&sync.stderr)
    );

    let output = asp_command(&root)
        .env("HOME", &home)
        .env("CODEX_HOME", home.join(".codex"))
        .env("ASP_AGENTS_HOME", &agents_dir)
        .args([
            "agent",
            "session",
            "status",
            "--name",
            "asp_explorer",
            "--root-session-id",
            "codex-root-thread",
            "--json",
        ])
        .output()
        .expect("status reads dynamic agent table lifetime");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).expect("status stdout");
    let status_json: serde_json::Value = serde_json::from_str(&stdout).expect("parse status json");
    assert_eq!(
        status_json["sessionLifetime"].as_str(),
        Some("resident"),
        "{stdout}"
    );
    assert_eq!(status_json["resident"].as_bool(), Some(true), "{stdout}");
    assert_eq!(
        status_json["sessionLifetimeSource"].as_str(),
        Some("agent-config"),
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
            "asp_explorer",
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
        stdout.contains(
            "requiredAction=main-agent-followup-existing-child-with-natural-language-runtime-switch"
        ),
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
            "asp_explorer",
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
        status_stdout.contains("\"routable\": false"),
        "{status_stdout}"
    );

    let _ = std::fs::remove_dir_all(root);
}
