use crate::provider_command::facade::document::org::commands::agent_session::support::{
    write_codex_asp_explorer_fixture_with_actual_profile,
    write_codex_asp_explorer_fixture_without_agent_path,
};
use crate::provider_command::support::{asp_command, temp_project_root};

#[test]
fn asp_agent_session_register_rejects_replacement_without_generation_cas() {
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
        .expect("reject replacement without generation CAS");
    assert!(
        !replacement.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&replacement.stderr)
    );
    let stderr = String::from_utf8(replacement.stderr).expect("replacement stderr");
    assert!(
        stderr.contains("replacement requires exact compare-and-swap"),
        "{stderr}"
    );

    let _ = std::fs::remove_dir_all(root);
}
