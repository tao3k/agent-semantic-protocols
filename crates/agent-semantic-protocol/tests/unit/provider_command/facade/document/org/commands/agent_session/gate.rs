use super::support::write_codex_asp_explorer_fixture;
use crate::provider_command::support::{asp_command, temp_project_root};

#[test]
fn asp_query_gate_invalid_child_enters_cleanup_bootstrap() {
    let root = temp_project_root("agent-command-session-invalid-child-gate");
    let home = root.join("home");
    std::fs::create_dir_all(&home).expect("create temp home");
    let root_session_id = "invalid-gate-root-thread";
    let child_session_id = "invalid-gate-child-thread";
    write_codex_asp_explorer_fixture(
        &home,
        root_session_id,
        child_session_id,
        "gpt-5.3-codex-spark",
        "read-only",
    );

    let register = asp_command(&root)
        .env("HOME", &home)
        .env("CODEX_HOME", home.join(".codex"))
        .env_remove("ASP_STATE_HOME")
        .env("CODEX_THREAD_ID", root_session_id)
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
            "--status",
            "invalid",
        ])
        .output()
        .expect("register invalid resident child");
    assert!(
        register.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&register.stderr)
    );

    let denied = asp_command(&root)
        .env("HOME", &home)
        .env("CODEX_HOME", home.join(".codex"))
        .env_remove("ASP_STATE_HOME")
        .env("CODEX_THREAD_ID", root_session_id)
        .args([
            "rust",
            "search",
            "owner",
            "build.rs",
            "items",
            "--workspace",
            ".",
            "--view",
            "seeds",
        ])
        .output()
        .expect("run gated ASP search");
    assert!(!denied.status.success());
    let stderr = String::from_utf8(denied.stderr).expect("denied stderr");
    assert!(
        stderr.contains("start the configured ASP managed subagent"),
        "{stderr}"
    );
    assert!(
        !stderr.contains(&format!(
            "Route this command to resident asp-explore child session `{child_session_id}`"
        )),
        "{stderr}"
    );
    assert!(!stderr.contains("reuse"), "{stderr}");

    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn asp_agent_session_rejects_removed_session_id_flag() {
    let root = temp_project_root("agent-command-session-removed-session-id");

    let output = asp_command(&root)
        .args([
            "agent",
            "session",
            "register",
            "--name",
            "asp-explore",
            "--session-id",
            "retired-child-id",
            "--roles",
            "subagent,search",
        ])
        .output()
        .expect("run removed session id command");
    assert!(!output.status.success());
    let stderr = String::from_utf8(output.stderr).expect("removed flag stderr");
    assert!(
        stderr.contains("unknown session flag `--session-id`"),
        "{stderr}"
    );
    assert!(!stderr.contains("deprecated"), "{stderr}");
    assert!(!stderr.contains("register --guide"), "{stderr}");

    let _ = std::fs::remove_dir_all(root);
}
