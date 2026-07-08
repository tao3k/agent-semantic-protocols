use crate::provider_command::support::{asp_command, temp_project_root};

#[test]
fn asp_agent_session_register_guide_explains_child_session_flow() {
    let root = temp_project_root("agent-command-session-register-guide");

    let guide = asp_command(&root)
        .env("CODEX_THREAD_ID", "codex-root-thread")
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
        stdout
            .contains("Register is a low-level step owned by the resident-child interactive loop"),
        "{stdout}"
    );
    assert!(stdout.contains("Detected host: codex"), "{stdout}");
    assert!(stdout.contains("Session env: CODEX_THREAD_ID"), "{stdout}");
    assert!(
        stdout.contains("Canonical loop entry:")
            && stdout.contains("asp agent session bootstrap --name asp-explore --json"),
        "{stdout}"
    );
    assert!(
        stdout.contains("Only run register when a loop choice asks for durable registration"),
        "{stdout}"
    );
    assert!(
        stdout.contains(
            "Codex action: start or resume the configured ASP managed subagent `asp_explorer`"
        ) && stdout.contains("do not ask the child to fork, create, or register another session"),
        "{stdout}"
    );
    assert!(
        stdout.contains("~/.agent-semantic-protocols/agents/asp-explorer_codex.toml"),
        "{stdout}"
    );
    assert!(
        stdout.contains("~/.codex/agents/asp-explorer.toml"),
        "{stdout}"
    );
    assert!(
        stdout.contains("Do not use register --guide, lifecycle audit, normal-thread reads"),
        "{stdout}"
    );

    let claude_guide = asp_command(&root)
        .env("CLAUDE_CODE_SESSION_ID", "claude-root-session")
        .args(["agent", "session", "register", "--guide"])
        .output()
        .expect("run session register guide for claude");
    assert!(
        claude_guide.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&claude_guide.stderr)
    );
    let claude_stdout = String::from_utf8(claude_guide.stdout).expect("claude guide stdout");
    assert!(
        claude_stdout.contains("Detected host: claude"),
        "{claude_stdout}"
    );
    assert!(
        claude_stdout.contains("Session env: CLAUDE_CODE_SESSION_ID"),
        "{claude_stdout}"
    );
    assert!(
        claude_stdout.contains("Claude action: start the configured subagent `asp-explorer`"),
        "{claude_stdout}"
    );
    assert!(
        claude_stdout.contains("~/.agent-semantic-protocols/agents/asp-explorer_claude.md"),
        "{claude_stdout}"
    );
    assert!(
        claude_stdout.contains("~/.claude/agents/asp-explorer.md"),
        "{claude_stdout}"
    );
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn asp_agent_session_fork_guide_refuses_bootstrap_semantics() {
    let root = temp_project_root("agent-command-session-fork-guide");

    let guide = asp_command(&root)
        .env("CODEX_THREAD_ID", "codex-root-thread")
        .args(["agent", "session", "fork", "--guide"])
        .output()
        .expect("run session fork guide");
    assert!(
        guide.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&guide.stderr)
    );
    let stdout = String::from_utf8(guide.stdout).expect("fork guide stdout");
    assert!(stdout.contains("asp agent session fork guide"), "{stdout}");
    assert!(stdout.contains("Action step flow"), "{stdout}");
    assert!(
        stdout.contains("This does not create a resident ASP child session"),
        "{stdout}"
    );
    assert!(stdout.contains("do not use fork as bootstrap"), "{stdout}");
    assert!(
        stdout.contains("asp agent session bootstrap --name asp-explore --json"),
        "{stdout}"
    );
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn asp_agent_session_status_guide_explains_start_resident_child_action() {
    let root = temp_project_root("agent-command-session-status-guide");

    let guide = asp_command(&root)
        .env("CODEX_THREAD_ID", "codex-root-thread")
        .args(["agent", "session", "status", "--guide"])
        .output()
        .expect("run session status guide");
    assert!(
        guide.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&guide.stderr)
    );
    let stdout = String::from_utf8(guide.stdout).expect("status guide stdout");
    assert!(
        stdout.contains("asp agent session status guide"),
        "{stdout}"
    );
    assert!(stdout.contains("Action step flow"), "{stdout}");
    assert!(
        stdout.contains("nextAction=start-resident-child-and-register"),
        "{stdout}"
    );
    assert!(
        stdout.contains(
            "Codex action: start or resume the configured ASP managed subagent `asp_explorer`"
        ) && stdout.contains("do not ask the child to fork, create, or register another session"),
        "{stdout}"
    );
    assert!(
        stdout.contains("asp agent session register --name asp-explore --child-session-id <child-session-id> --roles subagent,search"),
        "{stdout}"
    );
    assert!(stdout.contains("agentMessageTargetId"), "{stdout}");
    assert!(
        stdout.contains("bootstrapBlocked=host-message-agent-target-unavailable"),
        "{stdout}"
    );
    let _ = std::fs::remove_dir_all(root);
}
