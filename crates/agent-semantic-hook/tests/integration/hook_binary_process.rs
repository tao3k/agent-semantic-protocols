use std::io::Write as _;
use std::os::unix::fs::PermissionsExt as _;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

const EVENTS: &[&str] = &[
    "pre-tool",
    "permission-request",
    "post-tool",
    "stop",
    "notification",
    "user-prompt",
    "session-start",
    "subagent-start",
    "subagent-stop",
];

#[test]
fn inherited_no_agent_returns_valid_json_for_every_host_event() {
    for event in EVENTS {
        let output = Command::new(env!("CARGO_BIN_EXE_asp-hook"))
            .arg(event)
            .args(["--client", "codex"])
            .env("ASP_NO_AGENT", "1")
            .stdin(Stdio::null())
            .output()
            .expect("run Hook binary");
        assert_eq!(output.status.code(), Some(0), "event={event}");
        assert_eq!(
            serde_json::from_slice::<serde_json::Value>(&output.stdout)
                .unwrap_or_else(|error| panic!("event={event} invalid JSON: {error}")),
            serde_json::json!({}),
            "event={event}"
        );
        assert!(output.stderr.is_empty(), "event={event}");
    }
}

#[test]
fn binary_identity_is_owned_by_the_hook_package() {
    let output = Command::new(env!("CARGO_BIN_EXE_asp-hook"))
        .arg("--version")
        .output()
        .expect("run Hook binary version");
    assert_eq!(output.status.code(), Some(0));
    assert_eq!(output.stdout, b"asp-hook schema=1\n");
}

#[test]
fn malformed_host_payload_returns_valid_fail_closed_json_instead_of_code_101() {
    let mut child = Command::new(env!("CARGO_BIN_EXE_asp-hook"))
        .args([
            "pre-tool",
            "--client",
            "codex",
            "--generation",
            "/definitely/missing/compiled-hook-generation.json",
            "--host-match",
            "Bash",
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn Hook binary");
    child
        .stdin
        .as_mut()
        .expect("Hook stdin")
        .write_all(b"{not-json")
        .expect("write malformed payload");
    drop(child.stdin.take());
    let output = child.wait_with_output().expect("wait for Hook binary");
    let terminal: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("valid fail-closed Host JSON");
    assert_eq!(output.status.code(), Some(0));
    assert_eq!(terminal["hookSpecificOutput"]["permissionDecision"], "deny");
    assert!(output.stderr.is_empty());
}

#[test]
fn process_bound_no_agent_forms_bypass_the_installed_policy_engine() {
    let config = agent_semantic_config::default_hook_client_config_file()
        .expect("load canonical Hook Config V1");
    let generation = agent_semantic_hook::aot_compiler::compile_aot_hook_generation(
        &config,
        "blake3-256:process-bound-no-agent",
    )
    .expect("compile canonical HookGeneration");
    let temp = tempfile::tempdir().expect("temporary HookGeneration");
    let generation_path = temp.path().join("compiled-hook-generation.json");
    std::fs::write(&generation_path, generation).expect("write HookGeneration");
    std::fs::write(temp.path().join("fixture.rs"), "fn fixture() {}\n")
        .expect("write registered source operand");
    let probe_marker = temp.path().join("reader-probe-must-not-run");
    let probe_command = temp.path().join("arbitrary-command");
    std::fs::write(
        &probe_command,
        format!("#!/bin/sh\n: > '{}'\n", probe_marker.display()),
    )
    .expect("write probe sentinel command");
    let mut permissions = std::fs::metadata(&probe_command)
        .expect("probe command metadata")
        .permissions();
    permissions.set_mode(0o755);
    std::fs::set_permissions(&probe_command, permissions).expect("probe command executable");

    for command in [
        "ASP_NO_AGENT=1 arbitrary-command --unknown-option fixture.rs",
        "/usr/bin/env ASP_NO_AGENT=1 arbitrary-command --unknown-option fixture.rs",
        "export ASP_NO_AGENT=1; exec arbitrary-command --unknown-option fixture.rs",
    ] {
        let started = Instant::now();
        let mut child = Command::new(env!("CARGO_BIN_EXE_asp-hook"))
            .args([
                "pre-tool",
                "--client",
                "codex",
                "--generation",
                generation_path.to_str().expect("UTF-8 generation path"),
                "--host-match",
                "Bash",
            ])
            .current_dir(temp.path())
            .env("PATH", temp.path())
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("spawn Hook binary");
        serde_json::to_writer(
            child.stdin.as_mut().expect("Hook stdin"),
            &serde_json::json!({
                "tool_name": "Bash",
                "tool_input": {"command": command}
            }),
        )
        .expect("write Host payload");
        child.stdin.take().expect("close Hook stdin").flush().ok();
        let output = child.wait_with_output().expect("wait for Hook binary");
        assert!(
            started.elapsed() < Duration::from_secs(1),
            "first Host Hook terminal exceeded 1s: command={command} elapsed={:?}",
            started.elapsed()
        );
        assert_eq!(output.status.code(), Some(0), "command={command}");
        assert_eq!(
            serde_json::from_slice::<serde_json::Value>(&output.stdout)
                .unwrap_or_else(|error| panic!("command={command} invalid JSON: {error}")),
            serde_json::json!({}),
            "command={command}"
        );
        assert!(output.stderr.is_empty(), "command={command}");
        assert!(
            !probe_marker.exists(),
            "terminal no-agent allow must precede Reader Probe: command={command}"
        );
    }
}

#[test]
fn command_local_no_agent_escape_precedes_missing_generation() {
    let temp = tempfile::tempdir().expect("temporary missing generation");
    let missing = temp.path().join("missing-generation.json");
    for command in [
        "ASP_NO_AGENT=1 arbitrary-command fixture.rs",
        "/usr/bin/env ASP_NO_AGENT=1 arbitrary-command fixture.rs",
        "export ASP_NO_AGENT=1; exec arbitrary-command fixture.rs",
    ] {
        let mut child = Command::new(env!("CARGO_BIN_EXE_asp-hook"))
            .args([
                "pre-tool",
                "--client",
                "codex",
                "--generation",
                missing.to_str().expect("UTF-8 missing path"),
                "--host-match",
                "Bash",
            ])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("spawn Hook binary");
        serde_json::to_writer(
            child.stdin.as_mut().expect("Hook stdin"),
            &serde_json::json!({
                "tool_name": "Bash",
                "tool_input": {"command": command}
            }),
        )
        .expect("write Host payload");
        drop(child.stdin.take());
        let output = child.wait_with_output().expect("wait for Hook binary");
        assert_eq!(output.status.code(), Some(0), "command={command}");
        assert_eq!(
            serde_json::from_slice::<serde_json::Value>(&output.stdout)
                .unwrap_or_else(|error| panic!("command={command} invalid JSON: {error}")),
            serde_json::json!({}),
            "command={command}"
        );
    }

    let mut child = Command::new(env!("CARGO_BIN_EXE_asp-hook"))
        .args([
            "pre-tool",
            "--client",
            "codex",
            "--generation",
            missing.to_str().expect("UTF-8 missing path"),
            "--host-match",
            "Bash",
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("spawn negative Hook binary");
    serde_json::to_writer(
        child.stdin.as_mut().expect("Hook stdin"),
        &serde_json::json!({
            "tool_name": "Bash",
            "tool_input": {"command": "export ASP_NO_AGENT=1; arbitrary-command fixture.rs"}
        }),
    )
    .expect("write negative Host payload");
    drop(child.stdin.take());
    let output = child
        .wait_with_output()
        .expect("wait for negative Hook binary");
    let terminal: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("valid fail-closed Host JSON");
    assert_eq!(output.status.code(), Some(0));
    assert_eq!(terminal["hookSpecificOutput"]["permissionDecision"], "deny");
    assert!(
        terminal["hookSpecificOutput"]["permissionDecisionReason"]
            .as_str()
            .is_some_and(|message| message.contains("HookGeneration")),
        "terminal={terminal}"
    );
}
