use crate::provider_command::support;

#[test]
fn agent_platform_allows_agent_session_json_output() {
    let root = support::temp_project_root("agent-platform-allows-agent-session-json-output");

    let output = support::asp_command(&root)
        .env("CODEX_THREAD_ID", "test-agent-platform")
        .args(["agent", "session", "list", "--json"])
        .output()
        .expect("run asp agent session json command inside agent platform");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).expect("stdout");
    assert!(stdout.contains("\"sessions\""), "{stdout}");
    let stderr = String::from_utf8(output.stderr).expect("stderr");
    assert!(!stderr.contains("--json output is disabled"), "{stderr}");

    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn agent_platform_denies_non_session_json_output_with_token_warning() {
    let root = support::temp_project_root("agent-platform-denies-non-session-json-output");

    let output = support::asp_command(&root)
        .env("CODEX_THREAD_ID", "test-agent-platform")
        .args(["healthcheck", "--json", "."])
        .output()
        .expect("run non-session asp json command inside agent platform");

    assert!(
        !output.status.success(),
        "agent platform non-session --json must not succeed"
    );
    assert!(
        output.stdout.is_empty(),
        "denied json command must not emit stdout"
    );
    let stderr = String::from_utf8(output.stderr).expect("stderr");
    assert!(
        stderr.contains("warning: --json output is disabled"),
        "{stderr}"
    );
    assert!(
        stderr.contains("debug or programmatic use only"),
        "{stderr}"
    );
    assert!(stderr.contains("not normal agent workflow"), "{stderr}");
    assert!(stderr.contains("wastes tokens"), "{stderr}");
    assert!(stderr.contains("ASP_NO_AGENT_PLATFORM=1"), "{stderr}");

    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn claude_session_env_allows_agent_session_json_output() {
    let root = support::temp_project_root("claude-session-env-allows-agent-session-json-output");

    let output = support::asp_command(&root)
        .env("CLAUDE_SESSION_ID", "test-agent-platform")
        .args(["agent", "session", "list", "--json"])
        .output()
        .expect("run asp agent session json command inside claude agent platform");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).expect("stdout");
    assert!(stdout.contains("\"sessions\""), "{stdout}");
    let stderr = String::from_utf8(output.stderr).expect("stderr");
    assert!(!stderr.contains("--json output is disabled"), "{stderr}");

    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn explicit_non_agent_platform_env_allows_json_output() {
    let root = support::temp_project_root("agent-platform-json-output-explicit-non-agent");

    let output = support::asp_command(&root)
        .env("CODEX_THREAD_ID", "test-agent-platform")
        .env("ASP_NO_AGENT_PLATFORM", "1")
        .args(["agent", "session", "list", "--json"])
        .output()
        .expect("run asp json command with explicit non-agent env");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).expect("stdout");
    assert!(stdout.contains("\"rootSessionId\""), "{stdout}");
    let stderr = String::from_utf8(output.stderr).expect("stderr");
    assert!(!stderr.contains("--json output is disabled"), "{stderr}");

    let _ = std::fs::remove_dir_all(root);
}
