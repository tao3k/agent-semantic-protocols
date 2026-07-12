use super::support::{
    write_codex_asp_explorer_fixture, write_codex_asp_explorer_fixture_with_actual_sandbox,
};
use crate::provider_command::support::{asp_command, temp_project_root};

#[test]
fn asp_agent_session_archived_child_does_not_block_missing_resident_bootstrap() {
    let root = temp_project_root("agent-command-session-archived-child-bootstrap");
    write_message_target_owner_fixture(&root);
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
        .expect("register child session");
    assert!(
        register.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&register.stderr)
    );

    let close = asp_command(&root)
        .env("HOME", &home)
        .env("CODEX_HOME", home.join(".codex"))
        .env("ASP_STATE_HOME", &state_home)
        .args([
            "agent",
            "session",
            "close",
            "--child-session-id",
            child_session_id,
            "--root-session-id",
            root_session_id,
            "--json",
        ])
        .output()
        .expect("close child session");
    assert!(
        close.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&close.stderr)
    );

    let denied = asp_command(&root)
        .env("HOME", &home)
        .env("CODEX_HOME", home.join(".codex"))
        .env("ASP_STATE_HOME", &state_home)
        .env("CODEX_THREAD_ID", root_session_id)
        .args([
            "rust",
            "search",
            "owner",
            "crates/agent-semantic-protocol/src/command/agent_session_registry_message_target.rs",
            "items",
            "--query",
            "message_target_snapshot",
            "--workspace",
            ".",
            "--view",
            "seeds",
        ])
        .output()
        .expect("main session denied search");
    assert!(denied.status.success(), "search should be allowed");
    let output = format!(
        "{}{}",
        String::from_utf8_lossy(&denied.stdout),
        String::from_utf8_lossy(&denied.stderr)
    );
    assert!(output.contains("[search-owner]"), "{output}");
    assert!(!output.contains("reuse"), "{output}");

    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn asp_agent_session_invalid_child_does_not_block_missing_resident_bootstrap() {
    let root = temp_project_root("agent-command-session-invalid-child-bootstrap");
    write_message_target_owner_fixture(&root);
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
            "--status",
            "invalid",
        ])
        .output()
        .expect("register invalid child session");
    assert!(
        register.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&register.stderr)
    );

    let denied = asp_command(&root)
        .env("HOME", &home)
        .env("CODEX_HOME", home.join(".codex"))
        .env("ASP_STATE_HOME", &state_home)
        .env("CODEX_THREAD_ID", root_session_id)
        .args([
            "rust",
            "search",
            "owner",
            "crates/agent-semantic-protocol/src/command/agent_session_registry_message_target.rs",
            "items",
            "--query",
            "message_target_snapshot",
            "--workspace",
            ".",
            "--view",
            "seeds",
        ])
        .output()
        .expect("main session denied search");
    assert!(denied.status.success(), "search should be allowed");
    let output = format!(
        "{}{}",
        String::from_utf8_lossy(&denied.stdout),
        String::from_utf8_lossy(&denied.stderr)
    );
    assert!(output.contains("[search-owner]"), "{output}");
    assert!(
        !output.contains("childSessionId={child_session_id}"),
        "{output}"
    );
    assert!(!output.contains("reuse"), "{output}");

    let _ = std::fs::remove_dir_all(root);
}

fn write_message_target_owner_fixture(root: &std::path::Path) {
    let path = root.join(
        "crates/agent-semantic-protocol/src/command/agent_session_registry_message_target.rs",
    );
    std::fs::create_dir_all(path.parent().expect("owner fixture parent"))
        .expect("create owner fixture parent");
    std::fs::write(path, "pub fn message_target_snapshot() {}\n").expect("write owner fixture");
}

#[test]
fn asp_agent_session_smoke_invalid_child_bootstrap_runs_one_step() {
    let root = temp_project_root("agent-command-session-smoke-invalid-child-bootstrap");
    let smoke = asp_command(&root)
        .args(["agent", "session", "smoke", "--json"])
        .output()
        .expect("run agent session smoke");
    let output = format!(
        "{}{}",
        String::from_utf8_lossy(&smoke.stdout),
        String::from_utf8_lossy(&smoke.stderr)
    );
    assert!(smoke.status.success(), "{output}");
    assert!(
        output.contains("\"invalidChildBootstrapOk\": true"),
        "{output}"
    );
    assert!(output.contains("\"success\": true"), "{output}");
    assert!(!output.contains("reuse"), "{output}");

    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn asp_agent_session_bootstrap_create_choice_uses_concrete_codex_native_action() {
    let root = temp_project_root("agent-command-session-bootstrap-create-choice");
    let home = root.join("home");
    let agents_dir = home.join(".codex").join("agents");
    std::fs::create_dir_all(&agents_dir).expect("create codex agents dir");
    std::fs::write(
        agents_dir.join("asp-explorer.toml"),
        "name = \"asp_explorer\"\nmodel = \"gpt-5.4-mini\"\nmodel_reasoning_effort = \"low\"\nsandbox_mode = \"read-only\"\nsession_lifetime = \"resident\"\n",
    )
    .expect("write asp explorer config");
    let state_home = root.join(".asp-home");
    let root_session_id = "codex-root-thread";

    let bootstrap = asp_command(&root)
        .env("HOME", &home)
        .env("CODEX_HOME", home.join(".codex"))
        .env("ASP_STATE_HOME", &state_home)
        .env("CODEX_THREAD_ID", root_session_id)
        .args([
            "agent",
            "session",
            "bootstrap",
            "--name",
            "asp-explore",
            "--root-session-id",
            root_session_id,
        ])
        .output()
        .expect("run agent session bootstrap");
    let output = format!(
        "{}{}",
        String::from_utf8_lossy(&bootstrap.stdout),
        String::from_utf8_lossy(&bootstrap.stderr)
    );
    assert!(bootstrap.status.success(), "{output}");
    assert!(output.contains("pane: asp.session.create.v1"), "{output}");
    assert!(
        output.contains("1: create-managed-resident-child"),
        "{output}"
    );
    assert!(
        output.contains("Use the detected platform-native managed-agent creation surface"),
        "{output}"
    );
    assert!(
        output.contains("SubagentStart event owns registration and validation"),
        "{output}"
    );
    assert!(
        output.contains("platform-native-create: platform=codex managedAgentKind=asp_explorer"),
        "{output}"
    );
    assert!(
        output.contains("let SubagentStart capture the native identity"),
        "{output}"
    );
    assert!(
        output.contains(
            "platform-native-create-blocker: if platform=codex cannot create managedAgentKind=asp_explorer or emits no SubagentStart event"
        ),
        "{output}"
    );
    assert!(
        output.contains("lifecycle identity is captured by host hooks"),
        "{output}"
    );
    assert!(!output.contains("codex-native-create:"), "{output}");
    assert!(
        output.contains("never pass child ids, message targets, model claims, or lifecycle status"),
        "{output}"
    );
    assert!(!output.contains("receipt:"), "{output}");
    assert!(!output.contains("receipt-root:"), "{output}");
    assert!(!output.contains("--model <configuredModel>"), "{output}");
    assert!(!output.contains("{platform}"), "{output}");
    assert!(!output.contains("{managedAgentKind}"), "{output}");
    assert!(!output.contains("{requiredTransport}"), "{output}");
    assert!(!output.contains("--json"), "{output}");

    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn asp_agent_session_bootstrap_rejects_model_authored_native_receipts() {
    let root = temp_project_root("agent-command-session-bootstrap-native-receipt");
    let home = root.join("home");
    let agents_dir = home.join(".codex").join("agents");
    std::fs::create_dir_all(&agents_dir).expect("create codex agents dir");
    std::fs::write(
        agents_dir.join("asp-explorer.toml"),
        "name = \"asp_explorer\"\nmodel = \"gpt-5.4-mini\"\nmodel_reasoning_effort = \"low\"\nsandbox_mode = \"read-only\"\nsession_lifetime = \"resident\"\n",
    )
    .expect("write asp explorer config");
    let state_home = root.join(".asp-home");
    let root_session_id = "codex-root-thread";
    let child_session_id = "codex-child-thread";

    let bootstrap = asp_command(&root)
        .env("HOME", &home)
        .env("CODEX_HOME", home.join(".codex"))
        .env("ASP_STATE_HOME", &state_home)
        .env("CODEX_THREAD_ID", root_session_id)
        .args([
            "agent",
            "session",
            "bootstrap",
            "--name",
            "asp-explore",
            "--root-session-id",
            root_session_id,
            "--child-session-id",
            child_session_id,
            "--message-target-id",
            child_session_id,
            "--roles",
            "subagent,search",
            "--model",
            "gpt-5.4-mini",
        ])
        .output()
        .expect("run agent session bootstrap with native receipt");
    let output = format!(
        "{}{}",
        String::from_utf8_lossy(&bootstrap.stdout),
        String::from_utf8_lossy(&bootstrap.stderr)
    );
    assert!(!bootstrap.status.success(), "{output}");
    assert!(
        output.contains(
            "does not accept child identity, message target, model, parent, or status receipts"
        ),
        "{output}"
    );
    assert!(
        output.contains("let SubagentStart/SubagentStop update the registry"),
        "{output}"
    );

    let _ = std::fs::remove_dir_all(root);
}

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
            "--message-target-id",
            "codex-agent-target",
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
    let resume_stdout = String::from_utf8(resume.stdout).expect("resume stdout");
    assert!(
        resume_stdout.contains("[agent-session-resume]")
            && resume_stdout.contains("session=\"codex-child-thread\"")
            && resume_stdout.contains("messageTargetStatus=\"ready\"")
            && resume_stdout.contains("nextAction=\"send-follow-up-to-registered-message-target\""),
        "unexpected resume stdout: {resume_stdout}"
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
            "--message-target-id",
            "codex-agent-target",
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
        .env("ASP_NO_AGENT_PLATFORM", "1")
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
