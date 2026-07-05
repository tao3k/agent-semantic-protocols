use super::support::{
    write_codex_asp_explorer_fixture, write_codex_asp_explorer_fixture_with_actual_sandbox,
};
use crate::provider_command::support::{asp_command, temp_project_root};

#[test]
fn asp_agent_session_wraps_codex_saved_session_commands() {
    let root = temp_project_root("agent-command-session-codex-wrapper");
    let home = root.join("home");
    write_codex_asp_explorer_fixture(
        &home,
        "codex-root-thread",
        "codex-child-thread",
        "gpt-5.3-codex-spark",
        "read-only",
    );
    let state_root = root
        .join(".cache")
        .join("agent-semantic-protocol")
        .join("agent");

    let child_register = asp_command(&root)
        .env("HOME", &home)
        .args([
            "agent",
            "session",
            "register",
            "--state-root",
            state_root.to_str().unwrap(),
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
        .expect("register child session for wrapper");
    assert!(
        child_register.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&child_register.stderr)
    );

    let resume = asp_command(&root)
        .env("HOME", &home)
        .env("CODEX_THREAD_ID", "codex-root-thread")
        .env("ASP_CODEX_BIN", "/bin/echo")
        .args([
            "agent",
            "session",
            "resume",
            "--state-root",
            state_root.to_str().unwrap(),
            "--name",
            "asp-explore",
        ])
        .output()
        .expect("wrap codex resume");
    assert!(
        resume.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&resume.stderr)
    );
    assert_eq!(
        String::from_utf8(resume.stdout).expect("resume stdout"),
        "resume codex-child-thread\n"
    );

    let delete = asp_command(&root)
        .env("HOME", &home)
        .env("ASP_CODEX_BIN", "/bin/echo")
        .args([
            "agent",
            "session",
            "delete",
            "--state-root",
            state_root.to_str().unwrap(),
            "--child-session-id",
            "codex-child-thread",
            "--force",
        ])
        .output()
        .expect("wrap codex delete");
    assert!(
        delete.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&delete.stderr)
    );
    assert_eq!(
        String::from_utf8(delete.stdout).expect("delete stdout"),
        "delete --force codex-child-thread\n"
    );

    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn asp_agent_session_status_from_temp_cwd_uses_root_project_scope() {
    let root = temp_project_root("agent-command-session-status-temp-cwd");
    let home = root.join("home");
    let root_session_id = "codex-root-thread";
    let child_session_id = "codex-child-thread";
    write_codex_asp_explorer_fixture(
        &home,
        root_session_id,
        child_session_id,
        "gpt-5.3-codex-spark",
        "read-only",
    );

    let state_home = root.join(".asp-home");
    let register = asp_command(&root)
        .env("HOME", &home)
        .env("CODEX_HOME", home.join(".codex"))
        .env("ASP_STATE_HOME", &state_home)
        .args([
            "agent",
            "session",
            "register",
            "--name",
            "asp-explore",
            "--child-session-id",
            child_session_id,
            "--root-session-id",
            root_session_id,
            "--roles",
            "subagent,search",
        ])
        .output()
        .expect("register child into global registry");
    assert!(
        register.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&register.stderr)
    );

    let temp_cwd = std::env::temp_dir().join("asp-agent-session-status-temp-cwd");
    std::fs::create_dir_all(&temp_cwd).expect("create temp cwd");

    let status = asp_command(&root)
        .current_dir(&temp_cwd)
        .env("HOME", &home)
        .env("CODEX_HOME", home.join(".codex"))
        .env("ASP_STATE_HOME", &state_home)
        .env("CODEX_THREAD_ID", child_session_id)
        .args([
            "agent",
            "session",
            "status",
            "--name",
            "asp-explore",
            "--json",
        ])
        .output()
        .expect("status from temp cwd");
    assert!(
        status.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&status.stderr)
    );
    let stdout = String::from_utf8(status.stdout).expect("status stdout");
    assert!(
        stdout.contains("\"registryStatus\": \"active\""),
        "{stdout}"
    );
    assert!(stdout.contains("\"routable\": true"), "{stdout}");
    assert!(
        stdout.contains(&format!("\"rootSessionId\": \"{root_session_id}\"")),
        "{stdout}"
    );

    let temp_state = agent_semantic_runtime::state_core::ResolvedState::resolve_with_state_home(
        &temp_cwd,
        &state_home,
    )
    .expect("resolve temp cwd state");
    assert!(
        !temp_state.paths.project_json.is_file(),
        "temp cwd must not materialize a state project"
    );

    let _ = std::fs::remove_dir_all(&temp_cwd);
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn asp_agent_session_allows_mismatched_codex_sandbox_profile() {
    let root = temp_project_root("agent-command-session-codex-profile-mismatch");
    let home = root.join("home");
    let root_session_id = "sandbox-profile-root-thread";
    let child_session_id = "sandbox-profile-child-thread";
    write_codex_asp_explorer_fixture_with_actual_sandbox(
        &home,
        root_session_id,
        child_session_id,
        "gpt-5.3-codex-spark",
        "read-only",
        "danger-full-access",
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
            child_session_id,
            "--root-session-id",
            root_session_id,
            "--roles",
            "subagent,search",
        ])
        .output()
        .expect("register mismatched codex sandbox profile");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
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
            root_session_id,
            "--json",
        ])
        .output()
        .expect("status reports sandbox mismatch");
    assert!(
        status_output.status.success(),
        "{}",
        String::from_utf8_lossy(&status_output.stderr)
    );
    let stdout = String::from_utf8(status_output.stdout).expect("status stdout");
    assert!(
        stdout.contains("\"validationStatus\": \"passed\""),
        "{stdout}"
    );
    assert!(
        stdout.contains("sandbox expected read-only got danger-full-access"),
        "{stdout}"
    );

    let _ = std::fs::remove_dir_all(root);
}
