use std::io::Write as _;
use std::os::unix::fs::PermissionsExt as _;
use std::path::Path;
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

fn hook_command() -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_asp-hook"));
    // The project test runner may itself use the inherited process-level
    // escape to reach Cargo. Ordinary Host-contract fixtures must start from
    // a normal environment; the dedicated escape fixture opts back in.
    command.env_remove("ASP_NO_AGENT");
    command
}

fn plugin_launcher() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../asp-codex-plugin/bin/asp-hook-exec")
}

#[test]
fn inherited_no_agent_returns_valid_json_for_every_host_event() {
    for event in EVENTS {
        let output = hook_command()
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
fn temporary_subagent_events_are_observational_without_registration_authority() {
    let temp = tempfile::tempdir().expect("isolated State Home");
    for event in ["subagent-start", "subagent-stop"] {
        let started = Instant::now();
        let output = hook_command()
            .arg(event)
            .args(["--client", "codex"])
            .env("ASP_STATE_HOME", temp.path().join("missing"))
            .stdin(Stdio::null())
            .output()
            .expect("run observational SubAgent event");
        assert!(started.elapsed() < Duration::from_secs(1), "event={event}");
        assert_eq!(output.status.code(), Some(0), "event={event}");
        assert_eq!(
            serde_json::from_slice::<serde_json::Value>(&output.stdout)
                .expect("observational Host JSON"),
            serde_json::json!({}),
            "event={event}"
        );
        assert!(output.stderr.is_empty(), "event={event}");
    }
}

#[test]
fn configured_testing_agent_is_allowed_without_child_registration() {
    let temp = tempfile::tempdir().expect("isolated State Home");
    let runtime_bin = temp.path().join("runtime/bin");
    std::fs::create_dir_all(&runtime_bin).expect("Runtime bin directory");
    std::fs::copy(env!("CARGO_BIN_EXE_asp-hook"), runtime_bin.join("asp-hook"))
        .expect("materialize Hook evaluator");
    std::fs::set_permissions(
        runtime_bin.join("asp-hook"),
        std::fs::Permissions::from_mode(0o500),
    )
    .expect("Hook evaluator mode");

    let main_payload = serde_json::json!({
        "session_id": "root-test",
        "tool_name": "Bash",
        "tool_input": {"command": "cargo test -p fixture"}
    });
    let run = |payload: &serde_json::Value| {
        let mut command = Command::new(plugin_launcher());
        command
            .args(["pre-tool", "--client", "codex", "--host-match", "Bash"])
            .env("ASP_STATE_HOME", temp.path())
            .env_remove("ASP_NO_AGENT")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        let mut child = command.spawn().expect("launch real plugin Hook");
        child
            .stdin
            .as_mut()
            .expect("Hook stdin")
            .write_all(payload.to_string().as_bytes())
            .expect("write Host payload");
        drop(child.stdin.take());
        child.wait_with_output().expect("Hook terminal")
    };

    let unregistered = run(&main_payload);
    assert_eq!(unregistered.status.code(), Some(0));
    let unregistered: serde_json::Value =
        serde_json::from_slice(&unregistered.stdout).expect("deny Host JSON");
    assert_eq!(
        unregistered["hookSpecificOutput"]["permissionDecision"],
        "deny"
    );
    assert!(
        unregistered["hookSpecificOutput"]["additionalContext"]
            .as_str()
            .is_some_and(|context| context.contains("collaboration.spawn_agent")),
        "{unregistered}"
    );
    assert!(
        unregistered["hookSpecificOutput"]["additionalContext"]
            .as_str()
            .is_some_and(|context| context.contains(
                "asp session register-child --parent-thread-id root-test --agent-name asp_testing"
            )),
        "{unregistered}"
    );
    let route_key = blake3::hash(b"root-test").to_hex();
    let route_path = temp
        .path()
        .join("hooks/session-routes")
        .join(format!("{route_key}.json"));
    let route: serde_json::Value = serde_json::from_slice(
        &std::fs::read(&route_path).expect("standalone Hook publishes collaboration route"),
    )
    .expect("decode collaboration route receipt");
    assert_eq!(
        route["schemaId"],
        "agent.semantic-protocols.hook-session-route"
    );
    assert_eq!(route["schemaVersion"], 1);
    assert_eq!(route["rootSessionId"], "root-test");
    assert_eq!(route["targetAgent"], "asp_testing");
    assert_eq!(route["configRuleId"], "testing-role-dispatch");

    let agent_payload = serde_json::json!({
        "session_id": "agent-session-test",
        "agent_role": "asp_testing",
        "tool_name": "Bash",
        "tool_input": {"command": "cargo test -p fixture"}
    });

    let started = Instant::now();
    let registered = run(&agent_payload);
    assert!(
        started.elapsed() < Duration::from_secs(1),
        "registered first PreTool exceeded Host deadline: {:?}",
        started.elapsed()
    );
    assert_eq!(registered.status.code(), Some(0));
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&registered.stdout).expect("allow Host JSON"),
        serde_json::json!({})
    );
    assert!(registered.stderr.is_empty());
}

#[test]
fn process_bound_no_agent_allows_permission_request_without_inheriting_hook_environment() {
    let mut child = hook_command()
        .args(["permission-request", "--client", "codex"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn permission-request Hook binary");
    child
        .stdin
        .as_mut()
        .expect("Hook stdin")
        .write_all(
            br#"{"tool_name":"Bash","tool_input":{"command":"ASP_NO_AGENT=1 ./target/debug/asp install binary"}}"#,
        )
        .expect("write PermissionRequest payload");
    drop(child.stdin.take());
    let output = child.wait_with_output().expect("wait for permission Hook");
    assert_eq!(output.status.code(), Some(0));
    let terminal: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("valid PermissionRequest JSON");
    assert_eq!(
        terminal["hookSpecificOutput"]["hookEventName"],
        "PermissionRequest"
    );
    assert_eq!(
        terminal["hookSpecificOutput"]["decision"]["behavior"],
        "allow"
    );
    assert!(output.stderr.is_empty());
}

#[test]
fn permission_request_is_pass_through_after_native_pre_tool_policy() {
    for payload in [
        r#"{"tool_name":"Bash","tool_input":{"command":"cargo test"}}"#,
        r#"{"tool_name":"mcp__codex_app__create_thread","tool_input":{}}"#,
        r#"{"tool_name":"mcp__codex_app__send_message_to_thread","tool_input":{}}"#,
        r#"{"tool_name":"send_message_to_thread","tool_input":{}}"#,
    ] {
        let mut child = hook_command()
            .args(["permission-request", "--client", "codex"])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("spawn permission Hook binary");
        child
            .stdin
            .as_mut()
            .expect("Hook stdin")
            .write_all(payload.as_bytes())
            .expect("write PermissionRequest payload");
        drop(child.stdin.take());
        let output = child.wait_with_output().expect("wait for permission Hook");
        assert_eq!(output.status.code(), Some(0), "payload={payload}");
        let terminal: serde_json::Value =
            serde_json::from_slice(&output.stdout).expect("valid PermissionRequest JSON");
        assert_eq!(
            terminal["hookSpecificOutput"]["decision"]["behavior"], "allow",
            "payload={payload}"
        );
        assert!(output.stderr.is_empty(), "payload={payload}");
    }
}

#[test]
fn binary_identity_is_owned_by_the_hook_package() {
    let output = hook_command()
        .arg("--version")
        .output()
        .expect("run Hook binary version");
    assert_eq!(output.status.code(), Some(0));
    assert_eq!(output.stdout, b"asp-hook schema=1\n");
}

#[test]
fn binary_projects_its_embedded_policy_content_identity() {
    let output = hook_command()
        .arg("--identity")
        .env_remove("ASP_NO_AGENT")
        .output()
        .expect("run Hook binary identity");
    assert_eq!(output.status.code(), Some(0));
    let identity: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("typed Hook binary identity JSON");
    assert_eq!(
        identity["schemaId"],
        "agent.semantic-protocols.hook-runtime-identity"
    );
    assert_eq!(identity["schemaVersion"], 1);
    assert!(
        identity["policyContentDigest"]
            .as_str()
            .is_some_and(|digest| digest.starts_with("blake3-256:"))
    );
    assert!(output.stderr.is_empty());
}

#[test]
fn repeated_hook_subcommand_is_not_a_host_event_namespace() {
    let output = hook_command()
        .args([
            "hook",
            "pre-tool",
            "--client",
            "codex",
            "--host-match",
            "Bash",
        ])
        .stdin(Stdio::null())
        .output()
        .expect("run obsolete repeated Hook namespace");
    assert_eq!(output.status.code(), Some(0));
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&output.stdout)
            .expect("unknown standalone Hook command still has one valid Host JSON terminal"),
        serde_json::json!({})
    );
    assert!(output.stderr.is_empty());
}

#[test]
fn malformed_host_payload_returns_valid_fail_closed_json_instead_of_code_101() {
    let mut child = hook_command()
        .args([
            "pre-tool",
            "--client",
            "codex",
            "--policy-bundle",
            "/definitely/missing/compiled-hook-policy-bundle.json",
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
    let generation = agent_semantic_hook::aot_compiler::compile_aot_hook_policy_bundle(
        &config,
        "blake3-256:process-bound-no-agent",
    )
    .expect("compile canonical HookPolicyBundle");
    let temp = tempfile::tempdir().expect("temporary HookPolicyBundle");
    let policy_bundle_path = temp.path().join("compiled-hook-policy-bundle.json");
    std::fs::write(&policy_bundle_path, generation).expect("write HookPolicyBundle");
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
        "export ASP_NO_AGENT=1; arbitrary-command --unknown-option fixture.rs",
        "export ASP_NO_AGENT=1; exec arbitrary-command --unknown-option fixture.rs",
    ] {
        let started = Instant::now();
        let mut child = hook_command()
            .args([
                "pre-tool",
                "--client",
                "codex",
                "--policy-bundle",
                policy_bundle_path
                    .to_str()
                    .expect("UTF-8 policy bundle path"),
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
fn command_local_no_agent_escape_precedes_missing_policy_bundle() {
    let temp = tempfile::tempdir().expect("temporary missing policy bundle");
    let missing = temp.path().join("missing-policy-bundle.json");
    for command in [
        "ASP_NO_AGENT=1 arbitrary-command fixture.rs",
        "/usr/bin/env ASP_NO_AGENT=1 arbitrary-command fixture.rs",
        "export ASP_NO_AGENT=1; arbitrary-command fixture.rs",
        "export ASP_NO_AGENT=1; exec arbitrary-command fixture.rs",
    ] {
        let mut child = hook_command()
            .args([
                "pre-tool",
                "--client",
                "codex",
                "--policy-bundle",
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

    let mut child = hook_command()
        .args([
            "pre-tool",
            "--client",
            "codex",
            "--policy-bundle",
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
            "tool_input": {"command": "printf 'ASP_NO_AGENT=1'; arbitrary-command fixture.rs"}
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
            .is_some_and(|message| message.contains("Hook policy bundle")),
        "terminal={terminal}"
    );
}
