use std::env;
use std::fs;
use std::path::Path;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

#[test]
fn agent_session_resume_reports_missing_message_target_until_native_target_is_registered() {
    let state_root = temp_state_root("asp-resume-cli");
    let codex_home = temp_state_root("asp-resume-cli-codex-home");
    let root_session_id = "019f3db5-0000-7000-8000-000000000001";
    let child_session_id = "019f3db5-0000-7000-8000-000000000002";
    let binary = env!("CARGO_BIN_EXE_asp");

    let register = Command::new(binary)
        .env("CODEX_HOME", &codex_home)
        .args([
            "agent",
            "session",
            "register",
            "--state-root",
            path_str(&state_root),
            "--name",
            "resume-test",
            "--child-session-id",
            child_session_id,
            "--root-session-id",
            root_session_id,
            "--roles",
            "subagent,search",
            "--model",
            "gpt-5.4-mini",
            "--active",
        ])
        .output()
        .expect("register test session");
    assert_success("register", &register);

    let resume = Command::new(binary)
        .env("CODEX_HOME", &codex_home)
        .args([
            "agent",
            "session",
            "resume",
            "--state-root",
            path_str(&state_root),
            "--name",
            "resume-test",
            "--root-session-id",
            root_session_id,
        ])
        .output()
        .expect("resume test session");
    assert_success("resume", &resume);

    let stdout = String::from_utf8_lossy(&resume.stdout);
    assert!(stdout.contains("[agent-session-resume]"), "{stdout}");
    assert!(stdout.contains("registryRoutable=false"), "{stdout}");
    assert!(stdout.contains("routable=false"), "{stdout}");
    assert!(
        stdout.contains("messageTargetStatus=\"missing\""),
        "{stdout}"
    );
    assert!(
        stdout.contains("messageTargetResultSource=\"registry-message-target-id-missing\""),
        "{stdout}"
    );
    assert!(stdout.contains("messageAgentTargetId=\"\""), "{stdout}");
    assert!(
        stdout
            .contains("nextAction=\"rebind-existing-child-target-with-native-same-child-resume\""),
        "{stdout}"
    );
    assert!(
        stdout.contains("rolloutHistoryStatus=\"not-needed\""),
        "{stdout}"
    );
    assert!(!stdout.contains("rolloutActivityStatus=\"agent-active\""));

    let _ = fs::remove_dir_all(state_root);
    let _ = fs::remove_dir_all(codex_home);
}

#[test]
fn agent_session_resume_rejects_manually_registered_message_target_as_live() {
    let state_root = temp_state_root("asp-resume-cli-target");
    let codex_home = temp_state_root("asp-resume-cli-target-codex-home");
    let root_session_id = "019f3db5-0000-7000-8000-000000000011";
    let child_session_id = "019f3db5-0000-7000-8000-000000000012";
    let message_target_id = "native-message-agent-target-012";
    let binary = env!("CARGO_BIN_EXE_asp");

    let register = Command::new(binary)
        .env("CODEX_HOME", &codex_home)
        .args([
            "agent",
            "session",
            "register",
            "--state-root",
            path_str(&state_root),
            "--name",
            "resume-target-test",
            "--child-session-id",
            child_session_id,
            "--message-target-id",
            message_target_id,
            "--root-session-id",
            root_session_id,
            "--roles",
            "subagent,search",
            "--model",
            "gpt-5.4-mini",
            "--active",
        ])
        .output()
        .expect("register test session with message target");
    assert_success("register", &register);

    let resume = Command::new(binary)
        .env("CODEX_HOME", &codex_home)
        .args([
            "agent",
            "session",
            "resume",
            "--state-root",
            path_str(&state_root),
            "--name",
            "resume-target-test",
            "--root-session-id",
            root_session_id,
        ])
        .output()
        .expect("resume test session with message target");
    assert_success("resume", &resume);

    let stdout = String::from_utf8_lossy(&resume.stdout);
    assert!(stdout.contains("[agent-session-resume]"), "{stdout}");
    assert!(stdout.contains("registryRoutable=false"), "{stdout}");
    assert!(stdout.contains("routable=false"), "{stdout}");
    assert!(
        stdout.contains("messageTargetStatus=\"missing\""),
        "{stdout}"
    );
    assert!(
        stdout.contains("messageTargetResultSource=\"registry-message-target-id-missing\""),
        "{stdout}"
    );
    assert!(stdout.contains("messageAgentTargetId=\"\""), "{stdout}");
    assert!(
        stdout
            .contains("nextAction=\"rebind-existing-child-target-with-native-same-child-resume\""),
        "{stdout}"
    );

    let resume_json = Command::new(binary)
        .env("CODEX_HOME", &codex_home)
        .args([
            "agent",
            "session",
            "resume",
            "--state-root",
            path_str(&state_root),
            "--name",
            "resume-target-test",
            "--root-session-id",
            root_session_id,
            "--json",
        ])
        .output()
        .expect("resume --json stays on ASP control plane");
    assert_success("resume --json", &resume_json);

    let stdout = String::from_utf8_lossy(&resume_json.stdout);
    assert!(stdout.contains("[agent-session-resume]"), "{stdout}");
    assert!(stdout.contains("messageAgentTargetId=\"\""), "{stdout}");

    let status = Command::new(binary)
        .env("CODEX_HOME", &codex_home)
        .args([
            "agent",
            "session",
            "status",
            "--state-root",
            path_str(&state_root),
            "--name",
            "resume-target-test",
            "--root-session-id",
            root_session_id,
        ])
        .output()
        .expect("status test session with message target");
    assert_success("status", &status);

    let stdout = String::from_utf8_lossy(&status.stdout);
    assert!(stdout.contains("[agent-session-status]"), "{stdout}");
    assert!(stdout.contains("routable=false"), "{stdout}");
    assert!(
        stdout.contains("messageTargetStatus=\"unbound\""),
        "{stdout}"
    );
    assert!(
        stdout.contains(
            "messageTargetResultSource=\"persisted-message-target-without-live-attestation\""
        ),
        "{stdout}"
    );
    assert!(stdout.contains("messageAgentTargetId=\"\""), "{stdout}");

    let _ = fs::remove_dir_all(state_root);
    let _ = fs::remove_dir_all(codex_home);
}

#[test]
fn agent_session_resume_missing_session_checks_rollout_history_before_create() {
    let state_root = temp_state_root("asp-resume-rollout-preflight");
    let root_session_id = "019f3db5-0000-7000-8000-000000000021";
    let binary = env!("CARGO_BIN_EXE_asp");

    let resume = Command::new(binary)
        .args([
            "agent",
            "session",
            "resume",
            "--state-root",
            path_str(&state_root),
            "--name",
            "resume-test",
            "--root-session-id",
            root_session_id,
            "--roles",
            "subagent,search",
        ])
        .output()
        .expect("resume missing test session");
    assert_success("resume", &resume);

    let stdout = String::from_utf8_lossy(&resume.stdout);
    assert!(stdout.contains("[agent-session-resume]"), "{stdout}");
    assert!(stdout.contains("registryStatus=\"missing\""), "{stdout}");
    assert!(stdout.contains("registryRoutable=false"), "{stdout}");
    assert!(
        stdout.contains("rolloutHistoryStatus=\"checked-no-reusable-rollout\""),
        "{stdout}"
    );
    assert!(
        stdout
            .contains("rolloutHistoryAction=\"audit-host-agent-tree-after-rollout-history-miss\""),
        "{stdout}"
    );
    assert!(
        stdout.contains("nextAction=\"audit-host-agent-tree-after-rollout-history-miss\""),
        "{stdout}"
    );
    assert!(
        stdout.contains("messageTargetStatus=\"missing\""),
        "{stdout}"
    );

    let _ = fs::remove_dir_all(state_root);
}

#[test]
fn agent_session_resume_archived_same_root_reuses_existing_child_before_rollout_lookup() {
    let state_root = temp_state_root("asp-resume-archived-same-root");
    let codex_home = temp_state_root("asp-resume-archived-same-root-codex-home");
    let agents_dir = state_root.join("agents");
    fs::create_dir_all(&agents_dir).expect("create agents dir");
    fs::write(
        agents_dir.join("asp-explorer_codex.toml"),
        "name = \"asp_explorer\"\nmodel = \"gpt-5.4-mini\"\nsandbox_mode = \"read-only\"\n",
    )
    .expect("write asp explorer config");
    let root_session_id = "019f2c45-ba38-7000-8000-000000000031";
    let child_session_id = "019f2c45-ba38-7000-8000-000000000032";
    let message_target_id = "native-message-agent-target-032";
    write_codex_rollout_fixture(
        &codex_home,
        child_session_id,
        root_session_id,
        "gpt-5.4-mini",
    );
    let binary = env!("CARGO_BIN_EXE_asp");

    let register = Command::new(binary)
        .env("ASP_STATE_HOME", &state_root)
        .env("CODEX_HOME", &codex_home)
        .args([
            "agent",
            "session",
            "register",
            "--state-root",
            path_str(&state_root),
            "--name",
            "asp-explore",
            "--child-session-id",
            child_session_id,
            "--message-target-id",
            message_target_id,
            "--root-session-id",
            root_session_id,
            "--roles",
            "subagent,search",
            "--model",
            "gpt-5.4-mini",
            "--active",
        ])
        .output()
        .expect("register archived same-root resident");
    assert_success("register", &register);

    let close = Command::new(binary)
        .env("ASP_STATE_HOME", &state_root)
        .env("CODEX_HOME", &codex_home)
        .args([
            "agent",
            "session",
            "close",
            "--state-root",
            path_str(&state_root),
            "--name",
            "asp-explore",
            "--root-session-id",
            root_session_id,
        ])
        .output()
        .expect("archive same-root resident");
    assert_success("close", &close);
    fs::remove_dir_all(codex_home.join("sessions")).expect("remove rollout before resume");

    let resume = Command::new(binary)
        .env("ASP_STATE_HOME", &state_root)
        .env("CODEX_HOME", &codex_home)
        .args([
            "agent",
            "session",
            "resume",
            "--state-root",
            path_str(&state_root),
            "--name",
            "asp-explore",
            "--root-session-id",
            root_session_id,
        ])
        .output()
        .expect("resume archived same-root resident");
    assert_success("resume", &resume);

    let stdout = String::from_utf8_lossy(&resume.stdout);
    assert!(stdout.contains("registryStatus=\"archived\""), "{stdout}");
    assert!(stdout.contains("registryRoutable=false"), "{stdout}");
    assert!(
        stdout.contains("session=\"019f2c45-ba38-7000-8000-000000000032\""),
        "{stdout}"
    );
    assert!(
        stdout.contains("rolloutHistoryStatus=\"not-needed\""),
        "{stdout}"
    );
    assert!(stdout.contains("rolloutHistoryAction=\"none\""), "{stdout}");
    assert!(
        stdout.contains("nextAction=\"resume-archived-same-root-child-with-native-host\""),
        "{stdout}"
    );
    assert!(
        stdout.contains(
            "modelAlignmentAction=\"parent-resume-existing-archived-child-with-native-host\""
        ),
        "{stdout}"
    );
    assert!(
        stdout.contains("Session-start owns reactivation after the host resume"),
        "{stdout}"
    );
    assert!(
        !stdout.contains("create-resident-child-after-rollout-history-miss"),
        "{stdout}"
    );

    let _ = fs::remove_dir_all(state_root);
    let _ = fs::remove_dir_all(codex_home);
}

#[test]
fn agent_session_resume_reports_required_model_alignment_for_asp_explore() {
    let state_root = temp_state_root("asp-resume-model-alignment");
    let codex_home = temp_state_root("asp-resume-model-codex-home");
    let agents_dir = state_root.join("agents");
    fs::create_dir_all(&agents_dir).expect("create agents dir");
    fs::write(
        agents_dir.join("asp-explorer_codex.toml"),
        "name = \"asp_explorer\"\nmodel = \"gpt-5.4-mini\"\nsandbox_mode = \"read-only\"\n",
    )
    .expect("write asp explorer config");

    let root_session_id = "019f1f1a-5389-7223-a150-77dcb5ea8dd4";
    let child_session_id = "019f2dc6-3ed6-73b3-809d-62c4a3802ffb";
    write_codex_rollout_fixture(
        &codex_home,
        child_session_id,
        root_session_id,
        "gpt-5.4-mini",
    );
    let binary = env!("CARGO_BIN_EXE_asp");

    let register = Command::new(binary)
        .env("ASP_STATE_HOME", &state_root)
        .env("CODEX_HOME", &codex_home)
        .args([
            "agent",
            "session",
            "register",
            "--state-root",
            path_str(&state_root),
            "--name",
            "asp-explore",
            "--child-session-id",
            child_session_id,
            "--root-session-id",
            root_session_id,
            "--roles",
            "subagent,search",
            "--active",
        ])
        .output()
        .expect("register asp explore test session");
    assert_success("register", &register);

    let resume = Command::new(binary)
        .env("ASP_STATE_HOME", &state_root)
        .env("CODEX_HOME", &codex_home)
        .args([
            "agent",
            "session",
            "resume",
            "--state-root",
            path_str(&state_root),
            "--name",
            "asp-explore",
            "--root-session-id",
            root_session_id,
        ])
        .output()
        .expect("resume asp explore test session");
    assert_success("resume", &resume);

    let stdout = String::from_utf8_lossy(&resume.stdout);
    assert!(stdout.contains("[agent-session-resume]"), "{stdout}");
    assert!(stdout.contains("registryRoutable=false"), "{stdout}");
    assert!(
        stdout.contains("requiredModel=\"gpt-5.4-mini\""),
        "{stdout}"
    );
    assert!(
        stdout.contains(
            "modelAlignmentAction=\"rebind-existing-child-target-with-native-same-child-resume\""
        ),
        "{stdout}"
    );
    assert!(
        stdout.contains("The persisted session is not a live native message target"),
        "{stdout}"
    );

    let status = Command::new(binary)
        .env("ASP_STATE_HOME", &state_root)
        .env("CODEX_HOME", &codex_home)
        .args([
            "agent",
            "session",
            "status",
            "--state-root",
            path_str(&state_root),
            "--name",
            "asp-explore",
            "--root-session-id",
            root_session_id,
        ])
        .output()
        .expect("status asp explore test session");
    assert_success("status", &status);

    let stdout = String::from_utf8_lossy(&status.stdout);
    assert!(stdout.contains("[agent-session-status]"), "{stdout}");
    assert!(
        stdout.contains("messageTargetStatus=\"unbound\""),
        "{stdout}"
    );
    assert!(
        stdout.contains("messageTargetResultSource=\"live-message-target-binding-missing\""),
        "{stdout}"
    );
    assert!(stdout.contains("modelAlignmentAction=\"none\""), "{stdout}");
    assert!(
        stdout.contains(
            "nextAction=\"reenter-bootstrap-for-host-tree-target-rebind-or-typed-replacement\""
        ),
        "{stdout}"
    );

    let _ = fs::remove_dir_all(state_root);
    let _ = fs::remove_dir_all(codex_home);
}

fn temp_state_root(prefix: &str) -> std::path::PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock should be after unix epoch")
        .as_nanos();
    env::temp_dir().join(format!("{prefix}-{}-{nanos}", std::process::id()))
}

fn write_codex_rollout_fixture(
    codex_home: &std::path::Path,
    session_id: &str,
    root_session_id: &str,
    model: &str,
) {
    let rollout_dir = codex_home
        .join("sessions")
        .join("2026")
        .join("07")
        .join("04");
    fs::create_dir_all(&rollout_dir).expect("create codex rollout dir");
    let rollout_path = rollout_dir.join(format!("rollout-2026-07-04T08-36-35-{session_id}.jsonl"));
    fs::write(
        rollout_path,
        format!(
            r#"{{"type":"session_meta","payload":{{"id":"{session_id}","session_id":"{root_session_id}","parent_thread_id":"{root_session_id}","thread_source":"subagent","agent_role":"asp_explorer","source":{{"subagent":{{"thread_spawn":{{"parent_thread_id":"{root_session_id}","agent_role":"asp_explorer","agent_nickname":"ASP Explore","depth":1}}}}}},"model_provider":"openai","cwd":"/tmp/project"}}}}
{{"type":"turn_context","payload":{{"model":"{model}","sandbox_policy":{{"type":"danger-full-access"}},"approval_policy":"never","permission_profile":{{"type":"disabled"}}}}}}
"#
        ),
    )
    .expect("write codex rollout fixture");
}

fn path_str(path: &Path) -> &str {
    path.to_str()
        .expect("temporary test path should be valid UTF-8")
}

fn assert_success(label: &str, output: &std::process::Output) {
    if output.status.success() {
        return;
    }
    panic!(
        "{label} failed\nstatus: {:?}\nstdout:\n{}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}
