use crate::provider_command::support::{asp_command, temp_project_root};

#[test]
fn asp_agent_session_registers_named_root_and_subagent_sessions() {
    let root = temp_project_root("agent-command-session-registry");
    let state_root = root
        .join(".cache")
        .join("agent-semantic-protocol")
        .join("agent");

    let main_register = asp_command(&root)
        .env("CODEX_THREAD_ID", "codex-root-thread")
        .env_remove("CLAUDE_CODE_SESSION_ID")
        .env_remove("CLAUDE_CODE_REMOTE_SESSION_ID")
        .env_remove("AGENT_SESSION_ID")
        .env_remove("SESSION_ID")
        .args([
            "agent",
            "session",
            "register",
            "--state-root",
            state_root.to_str().unwrap(),
            "--name",
            "asp-main-explore",
            "--role",
            "main",
        ])
        .output()
        .expect("register main session");
    assert!(
        main_register.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&main_register.stderr)
    );
    let main_stdout = String::from_utf8(main_register.stdout).expect("main session stdout");
    assert!(
        main_stdout.contains(
            "[agent-session-register] owner=rust rootSession=\"codex-root-thread\" session=\"codex-root-thread\" name=\"asp-main-explore\" role=\"main\" status=\"active\""
        ),
        "{main_stdout}"
    );

    let child_register = asp_command(&root)
        .args([
            "agent",
            "session",
            "register",
            "--state-root",
            state_root.to_str().unwrap(),
            "--name",
            "asp-explore-code",
            "--child-session-id",
            "codex-child-code-thread",
            "--root-session-id",
            "codex-root-thread",
            "--parent-session-id",
            "codex-root-thread",
            "--role",
            "code",
            "--model",
            "cheap-code-model",
        ])
        .output()
        .expect("register child session");
    assert!(
        child_register.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&child_register.stderr)
    );

    let list = asp_command(&root)
        .env("CODEX_THREAD_ID", "codex-root-thread")
        .env_remove("CLAUDE_CODE_SESSION_ID")
        .env_remove("CLAUDE_CODE_REMOTE_SESSION_ID")
        .args([
            "agent",
            "session",
            "list",
            "--state-root",
            state_root.to_str().unwrap(),
        ])
        .output()
        .expect("list sessions");
    assert!(
        list.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&list.stderr)
    );
    let list_stdout = String::from_utf8(list.stdout).expect("session list stdout");
    assert!(
        list_stdout.contains(
            "[agent-session-list] owner=rust rootSession=\"codex-root-thread\" sessions=2"
        ),
        "{list_stdout}"
    );
    assert!(
        list_stdout.contains(
            "|session name=\"asp-explore-code\" session=\"codex-child-code-thread\" rootSession=\"codex-root-thread\" parentSession=\"codex-root-thread\" role=\"code\" model=\"cheap-code-model\" status=\"active\""
        ),
        "{list_stdout}"
    );

    let show = asp_command(&root)
        .env("CODEX_THREAD_ID", "codex-root-thread")
        .args([
            "agent",
            "session",
            "show",
            "--state-root",
            state_root.to_str().unwrap(),
            "--name",
            "asp-explore-code",
        ])
        .output()
        .expect("show child session");
    assert!(
        show.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&show.stderr)
    );
    let show_stdout = String::from_utf8(show.stdout).expect("session show stdout");
    assert!(
        show_stdout.contains(
            "|session name=\"asp-explore-code\" session=\"codex-child-code-thread\" rootSession=\"codex-root-thread\" parentSession=\"codex-root-thread\" role=\"code\" model=\"cheap-code-model\" status=\"active\""
        ),
        "{show_stdout}"
    );

    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn asp_agent_session_register_guide_explains_child_session_flow() {
    let root = temp_project_root("agent-command-session-register-guide");

    let guide = asp_command(&root)
        .args(["agent", "session", "register", "--guide"])
        .output()
        .expect("run session register guide");
    assert!(
        guide.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&guide.stderr)
    );
    let stdout = String::from_utf8(guide.stdout).expect("guide stdout");
    assert!(
        stdout.contains("asp agent session register guide"),
        "{stdout}"
    );
    assert!(
        stdout.contains(
            "asp agent session register --name asp-explore --child-session-id <child-session-id> --role asp-explore"
        ),
        "{stdout}"
    );
    assert!(stdout.contains("CODEX_THREAD_ID"), "{stdout}");
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
            "legacy-child-id",
            "--role",
            "asp-explore",
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
