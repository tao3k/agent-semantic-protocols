use std::env;
use std::fs;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

#[test]
fn agent_session_resume_reports_missing_message_target_until_native_target_is_registered() {
    let state_root = temp_state_root("asp-resume-cli");
    let codex_home = temp_state_root("asp-resume-cli-codex-home");
    let root_session_id = "019f3db5-0000-7000-8000-000000000001";
    let child_session_id = "019f3db5-0000-7000-8000-000000000002";
    let binary = env!("CARGO_BIN_EXE_asp");

    let register = run_with_supervisor_retry(|| {
        Command::new(binary)
            .env("CODEX_HOME", &codex_home)
            .args([
                "agent",
                "session",
                "register",
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
            .expect("register test session")
    });
    assert_success("register", &register);

    let resume = Command::new(binary)
        .env("CODEX_HOME", &codex_home)
        .args([
            "agent",
            "session",
            "resume",
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

    let _runtime_server = start_runtime_server_fixture(binary);

    let register = Command::new(binary)
        .env("ASP_RUNTIME_SERVER_LAUNCHCTL_PATH", "/usr/bin/true")
        .env("ASP_RUNTIME_SERVER_SYSTEMCTL_PATH", "/usr/bin/true")
        .env("CODEX_HOME", &codex_home)
        .args([
            "agent",
            "session",
            "register",
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
            "messageTargetResultSource=\"persisted-message-target-without-fresh-host-attestation\""
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
fn agent_session_resume_archived_same_root_is_rejected_as_historical() {
    let test_started = std::time::Instant::now();
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
    let binary = asp_test_binary();
    let _runtime_server = start_runtime_server_fixture(&binary);
    eprintln!(
        "archived-resume runtime-ready: {:?}",
        test_started.elapsed()
    );

    let register = run_with_supervisor_retry(|| {
        Command::new(&binary)
            .env("ASP_RUNTIME_SERVER_LAUNCHCTL_PATH", "/usr/bin/true")
            .env("ASP_RUNTIME_SERVER_SYSTEMCTL_PATH", "/usr/bin/true")
            .env("CODEX_HOME", &codex_home)
            .args([
                "agent",
                "session",
                "register",
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
            .expect("register archived same-root resident")
    });
    eprintln!(
        "archived-resume register-returned: {:?}",
        test_started.elapsed()
    );
    assert_success("register", &register);

    let status = Command::new(&binary)
        .env("ASP_RUNTIME_SERVER_LAUNCHCTL_PATH", "/usr/bin/true")
        .env("ASP_RUNTIME_SERVER_SYSTEMCTL_PATH", "/usr/bin/true")
        .env("CODEX_HOME", &codex_home)
        .args([
            "agent",
            "session",
            "status",
            "--name",
            "asp-explore",
            "--root-session-id",
            root_session_id,
        ])
        .output()
        .expect("status archived same-root resident before close");
    eprintln!(
        "archived-resume status-returned: {:?}",
        test_started.elapsed()
    );
    assert_success("status-before-close", &status);

    let close = run_with_supervisor_retry(|| {
        let mut command = Command::new(&binary);
        command
            .env("ASP_RUNTIME_SERVER_LAUNCHCTL_PATH", "/usr/bin/true")
            .env("ASP_RUNTIME_SERVER_SYSTEMCTL_PATH", "/usr/bin/true")
            .env("CODEX_HOME", &codex_home)
            .args([
                "agent",
                "session",
                "close",
                "--name",
                "asp-explore",
                "--root-session-id",
                root_session_id,
            ]);
        output_with_deadline(
            &mut command,
            "archive same-root resident",
            std::time::Duration::from_millis(1_000),
        )
    });
    eprintln!(
        "archived-resume close-returned: {:?}",
        test_started.elapsed()
    );
    assert_success("close", &close);
    fs::remove_dir_all(codex_home.join("sessions")).expect("remove rollout before resume");

    let resume = run_with_supervisor_retry(|| {
        Command::new(&binary)
            .env("ASP_RUNTIME_SERVER_LAUNCHCTL_PATH", "/usr/bin/true")
            .env("ASP_RUNTIME_SERVER_SYSTEMCTL_PATH", "/usr/bin/true")
            .env("CODEX_HOME", &codex_home)
            .args([
                "agent",
                "session",
                "resume",
                "--name",
                "asp-explore",
                "--root-session-id",
                root_session_id,
            ])
            .output()
            .expect("resume archived same-root resident")
    });
    eprintln!(
        "archived-resume resume-returned: {:?}",
        test_started.elapsed()
    );
    assert!(!resume.status.success(), "resume unexpectedly succeeded");
    let stderr = String::from_utf8_lossy(&resume.stderr);
    assert!(
        stderr.contains("archived agent session is historical and cannot be resumed"),
        "{stderr}"
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

fn output_with_deadline(
    command: &mut Command,
    label: &str,
    deadline: std::time::Duration,
) -> std::process::Output {
    command
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());
    let mut child = command
        .spawn()
        .unwrap_or_else(|error| panic!("failed to start {label}: {error}"));
    let started = std::time::Instant::now();
    loop {
        if child
            .try_wait()
            .unwrap_or_else(|error| panic!("failed to observe {label}: {error}"))
            .is_some()
        {
            return child
                .wait_with_output()
                .unwrap_or_else(|error| panic!("failed to collect {label}: {error}"));
        }
        if started.elapsed() >= deadline {
            let _ = child.kill();
            let output = child
                .wait_with_output()
                .unwrap_or_else(|error| panic!("failed to collect timed-out {label}: {error}"));
            panic!(
                "{label} exceeded {deadline:?}; global expiry refresh must remain outside the close hot path\nstdout:\n{}\nstderr:\n{}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            );
        }
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
}

fn asp_test_binary() -> String {
    let binary =
        env::var("ASP_TEST_ASP_BINARY").unwrap_or_else(|_| env!("CARGO_BIN_EXE_asp").to_owned());
    let path = std::path::Path::new(&binary);
    assert!(
        path.is_absolute(),
        "ASP_TEST_ASP_BINARY must be an absolute path: {}",
        path.display()
    );
    let metadata = fs::metadata(path).unwrap_or_else(|error| {
        panic!(
            "ASP_TEST_ASP_BINARY must name an existing executable {}: {error}",
            path.display()
        )
    });
    assert!(
        metadata.is_file(),
        "ASP_TEST_ASP_BINARY must name a file: {}",
        path.display()
    );
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        assert!(
            metadata.permissions().mode() & 0o111 != 0,
            "ASP_TEST_ASP_BINARY must be executable: {}",
            path.display()
        );
    }
    binary
}

struct RuntimeServerGuard {
    child: Option<std::process::Child>,
}

impl Drop for RuntimeServerGuard {
    fn drop(&mut self) {
        if let Some(child) = self.child.as_mut() {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

fn start_runtime_server_fixture(binary: &str) -> RuntimeServerGuard {
    if runtime_server_fixture_is_healthy(binary) {
        return RuntimeServerGuard { child: None };
    }
    let mut guard = RuntimeServerGuard {
        child: Some(
            Command::new(binary)
                .env("ASP_RUNTIME_SERVER_LAUNCHCTL_PATH", "/usr/bin/true")
                .env("ASP_RUNTIME_SERVER_SYSTEMCTL_PATH", "/usr/bin/true")
                .args(["server", "daemon"])
                .spawn()
                .expect("start isolated Runtime Server fixture"),
        ),
    };
    for _ in 0..200 {
        if let Some(status) = guard
            .child
            .as_mut()
            .expect("fixture owns the Runtime Server child")
            .try_wait()
            .expect("observe isolated Runtime Server fixture")
        {
            panic!("isolated Runtime Server fixture exited early: {status}");
        }
        if runtime_server_fixture_is_healthy(binary) {
            return guard;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    panic!("isolated Runtime Server fixture did not become healthy within 2 seconds");
}

fn runtime_server_fixture_is_healthy(binary: &str) -> bool {
    let status = Command::new(binary)
        .env("ASP_RUNTIME_SERVER_LAUNCHCTL_PATH", "/usr/bin/true")
        .env("ASP_RUNTIME_SERVER_SYSTEMCTL_PATH", "/usr/bin/true")
        .args(["server", "status"])
        .output()
        .expect("probe isolated Runtime Server fixture");
    status.status.success()
        && serde_json::from_slice::<serde_json::Value>(&status.stdout)
            .ok()
            .is_some_and(|receipt| {
                receipt.get("schemaId").and_then(serde_json::Value::as_str)
                    == Some("agent.semantic-protocols.runtime-server-control-receipt.v1")
                    && receipt
                        .get("schemaVersion")
                        .and_then(serde_json::Value::as_str)
                        == Some("1")
                    && receipt.get("state").and_then(serde_json::Value::as_str) == Some("healthy")
            })
}

fn run_with_supervisor_retry(
    mut run: impl FnMut() -> std::process::Output,
) -> std::process::Output {
    let first = run();
    if first.status.success() {
        return first;
    }
    let should_retry = serde_json::from_slice::<serde_json::Value>(&first.stderr)
        .ok()
        .is_some_and(|failure| {
            failure.get("schemaId").and_then(serde_json::Value::as_str)
                == Some("agent.semantic-protocols.runtime-server-supervisor-wall-failure")
                && failure
                    .get("retryAfterMs")
                    .and_then(serde_json::Value::as_u64)
                    == Some(250)
        });
    if !should_retry {
        return first;
    }
    std::thread::sleep(std::time::Duration::from_millis(250));
    run()
}
