use super::support::write_codex_asp_explorer_fixture;
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
            "--roles",
            "subagent",
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
            "[agent-session-register] owner=rust rootSession=\"codex-root-thread\" session=\"codex-root-thread\" name=\"asp-main-explore\" role=\"subagent\" status=\"active\""
        ),
        "{main_stdout}"
    );

    let child_register = asp_command(&root)
        .env("CODEX_THREAD_ID", "codex-root-thread")
        .env_remove("CLAUDE_CODE_SESSION_ID")
        .env_remove("CLAUDE_CODE_REMOTE_SESSION_ID")
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
            "--roles",
            "subagent,search",
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
            "|session name=\"asp-explore-code\" session=\"codex-child-code-thread\" rootSession=\"codex-root-thread\" parentSession=\"codex-root-thread\" role=\"search,subagent\" model=\"cheap-code-model\" status=\"active\""
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
            "|session name=\"asp-explore-code\" session=\"codex-child-code-thread\" rootSession=\"codex-root-thread\" parentSession=\"codex-root-thread\" role=\"search,subagent\" model=\"cheap-code-model\" status=\"active\""
        ),
        "{show_stdout}"
    );

    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn asp_agent_session_defaults_to_global_state_root() {
    let root = temp_project_root("agent-command-session-global-root-a");
    let other_root = temp_project_root("agent-command-session-global-root-b");
    let _ = std::fs::remove_dir_all(&root);
    let _ = std::fs::remove_dir_all(&other_root);
    std::fs::create_dir_all(&root).expect("create clean root");
    std::fs::create_dir_all(&other_root).expect("create clean other root");
    let home = root.join("home");
    std::fs::create_dir_all(&home).expect("create temp home");
    let root_session_id = "global-root-thread";
    let child_session_id = "global-child-thread";
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
        .env_remove("CLAUDE_CODE_SESSION_ID")
        .env_remove("CLAUDE_CODE_REMOTE_SESSION_ID")
        .env_remove("AGENT_SESSION_ID")
        .env_remove("SESSION_ID")
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
        .expect("register global child session");
    assert!(
        register.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&register.stderr)
    );

    let list = asp_command(&other_root)
        .env("HOME", &home)
        .env("CODEX_HOME", home.join(".codex"))
        .env_remove("ASP_STATE_HOME")
        .env("CODEX_THREAD_ID", root_session_id)
        .args(["agent", "session", "list"])
        .output()
        .expect("list project-scoped global registry from another cwd");
    assert!(
        list.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&list.stderr)
    );
    let stdout = String::from_utf8(list.stdout).expect("global list stdout");
    assert!(
        stdout.contains(&format!(
            "[agent-session-list] owner=rust rootSession=\"{root_session_id}\" sessions=1"
        )),
        "{stdout}"
    );
    assert!(
        stdout.contains(&format!("session=\"{child_session_id}\"")),
        "{stdout}"
    );

    let _ = std::fs::remove_dir_all(root);
    let _ = std::fs::remove_dir_all(other_root);
}

#[test]
fn asp_agent_session_register_infers_root_from_codex_child_rollout() {
    let root = temp_project_root("agent-command-session-register-infers-rollout-root");
    let home = root.join("home");
    std::fs::create_dir_all(&home).expect("create temp home");
    let root_session_id = "infer-root-thread";
    let child_session_id = "infer-child-thread";
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
        .env("CODEX_THREAD_ID", child_session_id)
        .args([
            "agent",
            "session",
            "register",
            "--name",
            "asp-explore",
            "--child-session-id",
            child_session_id,
            "--roles",
            "subagent,search",
        ])
        .output()
        .expect("register child session with inferred root");
    assert!(
        register.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&register.stderr)
    );
    let stdout = String::from_utf8(register.stdout).expect("register stdout");
    assert!(
        stdout.contains(&format!(
            "rootSession=\"{root_session_id}\" session=\"{child_session_id}\""
        )),
        "{stdout}"
    );

    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn asp_agent_session_reuse_in_unregistered_child_does_not_use_codex_rollout_root() {
    let root = temp_project_root("agent-command-session-reuse-infers-rollout-root");
    let home = root.join("home");
    std::fs::create_dir_all(&home).expect("create temp home");
    let root_session_id = "reuse-root-thread";
    let registered_child_session_id = "registered-reuse-child-thread";
    let unregistered_child_session_id = "unregistered-reuse-child-thread";
    write_codex_asp_explorer_fixture(
        &home,
        root_session_id,
        registered_child_session_id,
        "gpt-5.3-codex-spark",
        "read-only",
    );

    let register = asp_command(&root)
        .env("HOME", &home)
        .env("CODEX_HOME", home.join(".codex"))
        .env_remove("ASP_STATE_HOME")
        .env("CODEX_THREAD_ID", root_session_id)
        .env("ASP_NO_AGENT_PLATFORM", "1")
        .args([
            "agent",
            "session",
            "register",
            "--name",
            "asp-explore",
            "--child-session-id",
            registered_child_session_id,
            "--roles",
            "subagent,search",
        ])
        .output()
        .expect("register resident child session");
    assert!(
        register.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&register.stderr)
    );
    write_codex_asp_explorer_fixture(
        &home,
        root_session_id,
        unregistered_child_session_id,
        "gpt-5.3-codex-spark",
        "read-only",
    );

    let reuse = asp_command(&root)
        .env("HOME", &home)
        .env("CODEX_HOME", home.join(".codex"))
        .env_remove("ASP_STATE_HOME")
        .env("CODEX_THREAD_ID", unregistered_child_session_id)
        .args(["agent", "session", "resume", "--name", "asp-explore"])
        .output()
        .expect("do not reuse resident child from unregistered subagent rollout");
    assert!(
        reuse.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&reuse.stderr)
    );
    let stdout = String::from_utf8(reuse.stdout).expect("reuse stdout");
    assert!(stdout.contains("[agent-session-resume]"), "{stdout}");
    assert!(
        stdout.contains(&format!("session=\"{registered_child_session_id}\"")),
        "{stdout}"
    );
    assert!(
        stdout.contains("messageTargetStatus=\"missing\""),
        "{stdout}"
    );
    assert!(
        stdout.contains(&format!("rootSession=\"{root_session_id}\"")),
        "{stdout}"
    );

    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn asp_agent_session_reuse_adopts_existing_codex_asp_child_from_root_rollout() {
    let root = temp_project_root("agent-command-session-reuse-adopts-root-rollout-child");
    let home = root.join("home");
    std::fs::create_dir_all(&home).expect("create temp home");
    let root_session_id = "reuse-adopt-root-thread";
    let child_session_id = "reuse-adopt-child-thread";
    write_codex_asp_explorer_fixture(
        &home,
        root_session_id,
        child_session_id,
        "gpt-5.3-codex-spark",
        "read-only",
    );

    let reuse = asp_command(&root)
        .env("HOME", &home)
        .env("CODEX_HOME", home.join(".codex"))
        .env_remove("ASP_STATE_HOME")
        .env("CODEX_THREAD_ID", root_session_id)
        .env("ASP_NO_AGENT_PLATFORM", "1")
        .args([
            "agent",
            "session",
            "resume",
            "--name",
            "asp-explore",
            "--json",
        ])
        .output()
        .expect("reuse existing rollout child");
    assert!(
        reuse.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&reuse.stderr)
    );
    let stdout = String::from_utf8(reuse.stdout).expect("reuse stdout");
    assert!(
        stdout.contains(&format!("session=\"{child_session_id}\"")),
        "{stdout}"
    );
    assert!(stdout.contains("name=\"asp-explore\""), "{stdout}");
    assert!(
        stdout.contains("rolloutHistoryStatus=\"adopted-reusable-rollout\""),
        "{stdout}"
    );
    assert!(
        stdout.contains("messageTargetStatus=\"missing\""),
        "{stdout}"
    );

    let second_reuse = asp_command(&root)
        .env("HOME", &home)
        .env("CODEX_HOME", home.join(".codex"))
        .env_remove("ASP_STATE_HOME")
        .env("CODEX_THREAD_ID", root_session_id)
        .env("ASP_NO_AGENT_PLATFORM", "1")
        .args([
            "agent",
            "session",
            "resume",
            "--name",
            "asp-explore",
            "--json",
        ])
        .output()
        .expect("reuse adopted registry child");
    assert!(
        second_reuse.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&second_reuse.stderr)
    );
    let second_stdout = String::from_utf8(second_reuse.stdout).expect("second reuse stdout");
    assert!(
        second_stdout.contains(&format!("session=\"{child_session_id}\"")),
        "{second_stdout}"
    );
    assert!(
        second_stdout.contains("messageTargetStatus=\"missing\""),
        "{second_stdout}"
    );

    let _ = std::fs::remove_dir_all(root);
}
