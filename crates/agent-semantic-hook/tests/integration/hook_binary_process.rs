use std::io::Write as _;
use std::os::unix::fs::PermissionsExt as _;
use std::path::Path;
use std::process::Command;
use std::process::Stdio;
use std::time::Duration;
use std::time::Instant;

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

// These are one-shot Host-process acceptance tests, not a throughput test.
// Keep their sub-process deadline independent of unrelated sibling fixtures;
// the dedicated Reader Probe tests exercise actual concurrent pressure.
static HOOK_PROCESS_TEST_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

fn hook_process_test_guard() -> std::sync::MutexGuard<'static, ()> {
    HOOK_PROCESS_TEST_LOCK
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

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
    let _test_guard = hook_process_test_guard();
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
fn only_the_exact_inherited_no_agent_value_bypasses_policy_bootstrap() {
    let _test_guard = hook_process_test_guard();
    for value in ["", "0", "true", "2"] {
        let mut child = hook_command()
            .args(["pre-tool", "--client", "codex", "--host-match", "Bash"])
            .env("ASP_NO_AGENT", value)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .expect("spawn noncanonical no-agent Hook binary");
        serde_json::to_writer(
            child.stdin.as_mut().expect("Hook stdin"),
            &serde_json::json!({
                "tool_name": "Bash",
                "tool_input": {"command": "rg HookDecision fixture.rs"}
            }),
        )
        .expect("write Host payload");
        drop(child.stdin.take());
        let output = child
            .wait_with_output()
            .expect("wait for noncanonical no-agent Hook binary");
        let terminal: serde_json::Value =
            serde_json::from_slice(&output.stdout).expect("valid fail-closed Host JSON");
        assert_eq!(output.status.code(), Some(0), "value={value:?}");
        assert_eq!(
            terminal["hookSpecificOutput"]["permissionDecision"], "deny",
            "value={value:?} must not be a recovery selector"
        );
    }
}

#[test]
fn temporary_subagent_start_is_observational_and_stop_requires_identity() {
    let _test_guard = hook_process_test_guard();
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
        let terminal = serde_json::from_slice::<serde_json::Value>(&output.stdout)
            .expect("SubAgent Host JSON");
        if event == "subagent-start" {
            assert_eq!(terminal, serde_json::json!({}), "event={event}");
        } else {
            assert_eq!(terminal["decision"], "block", "event={event}");
            assert!(
                terminal["reason"]
                    .as_str()
                    .is_some_and(|reason| reason.starts_with("Example\n")),
                "event={event}"
            );
        }
        assert!(output.stderr.is_empty(), "event={event}");
    }
}

#[test]
fn asp_explorer_stop_requires_executable_source_free_evidence() {
    let _test_guard = hook_process_test_guard();
    let run = |last_assistant_message: &str| {
        let mut child = hook_command()
            .arg("subagent-stop")
            .args(["--client", "codex"])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .expect("spawn SubagentStop Hook");
        serde_json::to_writer(
            child.stdin.as_mut().expect("Hook stdin"),
            &serde_json::json!({
                "hook_event_name": "SubagentStop",
                "agent_id": "explorer-child",
                "agent_type": "asp_explorer",
                "last_assistant_message": last_assistant_message,
            }),
        )
        .expect("write SubagentStop payload");
        drop(child.stdin.take());
        child.wait_with_output().expect("SubagentStop terminal")
    };

    let valid = run(
        "[asp-search-subagent]\nstate=candidates\nQueryGrammar: asp query --selector <exact-selector> --projection <callable-skeleton|source>\nE1 | owner=crates/runtime | item=struct/Endpoint | selector=rust://crates/runtime#item/struct/Endpoint | matchedBy=rg:0|syntax:0 | relation=publishes",
    );
    assert_eq!(valid.status.code(), Some(0));
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&valid.stdout).expect("valid Host JSON"),
        serde_json::json!({})
    );

    let source_dump = run(
        "[asp-search-subagent]\nstate=candidates\nQueryGrammar: asp query --selector <exact-selector> --projection <callable-skeleton|source>\nE1 | owner=crates/runtime | item=struct/Endpoint | selector=rust://crates/runtime#item/struct/Endpoint | matchedBy=rg:0|syntax:0 | relation=publishes\n```rust\nfn endpoint() {}\n```",
    );
    assert_eq!(source_dump.status.code(), Some(0));
    let terminal: serde_json::Value =
        serde_json::from_slice(&source_dump.stdout).expect("blocking Host JSON");
    assert_eq!(terminal["decision"], "block");
    assert!(
        terminal["reason"].as_str().is_some_and(
            |reason| reason.starts_with("Example\n") && reason.contains("\n\nGrammar\n")
        )
    );
}

#[test]
fn configured_testing_agent_is_allowed_without_child_registration() {
    let _test_guard = hook_process_test_guard();
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
    let _test_guard = hook_process_test_guard();
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
    let _test_guard = hook_process_test_guard();
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
    let _test_guard = hook_process_test_guard();
    let output = hook_command()
        .arg("--version")
        .output()
        .expect("run Hook binary version");
    assert_eq!(output.status.code(), Some(0));
    assert_eq!(output.stdout, b"asp-hook schema=1\n");
}

#[test]
fn binary_projects_its_embedded_policy_content_identity() {
    let _test_guard = hook_process_test_guard();
    let started = std::time::Instant::now();
    let output = hook_command()
        .arg("--identity")
        .env_remove("ASP_NO_AGENT")
        .output()
        .expect("run Hook binary identity");
    let elapsed = started.elapsed();
    assert_eq!(output.status.code(), Some(0));
    let identity: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("typed Hook binary identity JSON");
    assert_eq!(
        identity["schemaId"],
        "agent.semantic-protocols.hook-runtime-identity"
    );
    assert_eq!(identity["schemaVersion"], 1);
    assert!(
        identity["handlerElapsedNanos"]
            .as_u64()
            .is_some_and(|elapsed| elapsed < 1_000_000),
        "identity handler must remain below 1ms: {identity}"
    );
    assert_eq!(
        identity["policyContentDigest"],
        agent_semantic_hook::aot_compiler::embedded_hook_policy_content_digest()
            .expect("project embedded Hook policy content identity")
    );
    assert!(output.stderr.is_empty());
    assert!(
        elapsed < std::time::Duration::from_secs(1),
        "identity probe must remain compiler-free and bounded: {elapsed:?}"
    );
}

#[test]
fn default_hook_process_applies_state_home_config_overlay_without_policy_bundle_flag() {
    let _test_guard = hook_process_test_guard();
    let state_home = tempfile::tempdir().expect("create State Home fixture");
    let config_path = state_home.path().join("hooks/config.toml");
    std::fs::create_dir_all(
        config_path
            .parent()
            .expect("State Home Hook config has parent"),
    )
    .expect("create State Home Hook config root");
    let source = agent_semantic_config::default_hook_client_config_template().replace(
        "Agent-facing search JSON is denied; use the compact ASP search route.",
        "State Home overlay policy is active.",
    );
    std::fs::write(&config_path, source).expect("write State Home Hook config");

    let mut child = hook_command()
        .args(["pre-tool", "--client", "codex", "--host-match", "Bash"])
        .env("ASP_STATE_HOME", state_home.path())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn default Hook binary");
    serde_json::to_writer(
        child.stdin.as_mut().expect("Hook stdin"),
        &serde_json::json!({
            "tool_name": "Bash",
            "tool_input": {"command": "asp rust search --json HookDecision"}
        }),
    )
    .expect("write Host payload");
    drop(child.stdin.take());
    let output = child.wait_with_output().expect("wait for Hook binary");
    let terminal: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("valid Hook Host JSON");
    assert_eq!(output.status.code(), Some(0));
    assert_eq!(terminal["hookSpecificOutput"]["permissionDecision"], "deny");
    assert!(
        terminal["hookSpecificOutput"]["permissionDecisionReason"]
            .as_str()
            .is_some_and(|message| message.contains("State Home overlay policy is active.")),
        "default Hook path ignored State Home config overlay: {terminal}"
    );
    assert!(output.stderr.is_empty());
}

#[test]
fn default_hook_process_denies_rtk_style_script_reader_from_dynamic_probe() {
    #[cfg(target_os = "macos")]
    {
        let _test_guard = hook_process_test_guard();
        let state_home = tempfile::tempdir().expect("create Reader State Home fixture");
        let project_root = tempfile::tempdir().expect("create Reader project fixture");
        let bin = project_root.path().join("bin");
        std::fs::create_dir_all(project_root.path().join("docs")).expect("create docs root");
        std::fs::create_dir_all(&bin).expect("create fake RTK bin");
        let rtk = bin.join("rtk");
        std::fs::write(
            &rtk,
            "#!/bin/sh\nset -eu\n[ \"$1\" = read ] || exit 64\nshift\nwhile [ \"$#\" -gt 1 ]; do shift; done\ncase \"$1\" in\n  *readable-sentinel*) exit 0 ;;\n  *denied-sentinel*) exit 1 ;;\n  *) exit 65 ;;\nesac\n",
        )
        .expect("write interpreter-backed Reader fixture");
        std::fs::set_permissions(&rtk, std::fs::Permissions::from_mode(0o500))
            .expect("make interpreter-backed Reader fixture executable");

        let probe_payload = serde_json::json!({
            "tool_name": "Bash",
            "tool_input": {"command": "rtk read --max-lines 1 docs/hook-contract.md"}
        })
        .to_string();
        let policy = agent_semantic_hook::aot_compiler::compile_embedded_hook_policy_bundle()
            .expect("compile embedded Hook policy");
        let probe = agent_semantic_hook::aot_evaluator::reader_probe_request(
            std::str::from_utf8(&policy).expect("Hook policy UTF-8"),
            &probe_payload,
            "Bash",
        )
        .expect("admit unknown reader to dynamic probe")
        .expect("RTK-style reader must not be lost before the dynamic probe");
        assert_eq!(
            probe.command_tokens,
            ["rtk", "read", "--max-lines", "1", "docs/hook-contract.md"]
        );
        assert_eq!(probe.subject, "docs/hook-contract.md");

        let mut child = hook_command()
            .args(["pre-tool", "--client", "codex", "--host-match", "Bash"])
            .current_dir(project_root.path())
            .env("ASP_STATE_HOME", state_home.path())
            .env("PATH", format!("{}:/usr/bin:/bin", bin.display()))
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("spawn dynamic Reader Hook binary");
        serde_json::to_writer(
            child.stdin.as_mut().expect("Hook stdin"),
            &serde_json::json!({
                "tool_name": "Bash",
                "session_id": "rtk-dynamic-reader-hook-test",
                "tool_use_id": "rtk-dynamic-reader-hook-tool-use",
                "cwd": project_root.path(),
                "tool_input": {
                    "command": "rtk read --max-lines 1 docs/hook-contract.md"
                }
            }),
        )
        .expect("write dynamic Reader Host payload");
        drop(child.stdin.take());
        let output = child
            .wait_with_output()
            .expect("wait for dynamic Reader Hook binary");
        let terminal: serde_json::Value =
            serde_json::from_slice(&output.stdout).expect("valid dynamic Reader Host JSON");

        let paths = agent_semantic_runtime::project_state_paths_with_state_home(
            project_root.path(),
            state_home.path(),
        )
        .expect("resolve Hook event state");
        let events = std::fs::read_to_string(paths.hook_state_dir.join("events.jsonl"))
            .expect("dynamic Reader probe event");
        let event = events
            .lines()
            .filter_map(|line| serde_json::from_str::<serde_json::Value>(line).ok())
            .find(|event| event["fields"]["toolUseId"] == "rtk-dynamic-reader-hook-tool-use")
            .expect("linked dynamic Reader probe receipt");
        let access = event["fields"]["readerProbe"]["access"]
            .as_str()
            .expect("Reader access");
        assert!(matches!(access, "read" | "unknown"), "event={event}");
        if access == "unknown" {
            assert_eq!(
                event["fields"]["policyDecision"]["evidence"],
                "reader-probe-indeterminate-fail-closed",
                "event={event}"
            );
            assert!(
                matches!(
                    event["fields"]["readerProbe"]["terminal"].as_str(),
                    Some("probe-timeout" | "probe-deferred")
                ),
                "event={event}"
            );
        }
        assert_eq!(
            event["fields"]["readerProbe"]["backend"],
            "permission-differential"
        );
        assert_eq!(event["fields"]["readerProbe"]["probeProcessLaunched"], true);
        assert_eq!(
            event["fields"]["policyDecision"]["configRuleId"],
            "route-markdown-document-read-to-asp-explorer",
            "event={event}"
        );
        assert_eq!(event["decision"], "deny", "event={event}");
        assert_eq!(output.status.code(), Some(0));
        assert_eq!(
            terminal["hookSpecificOutput"]["permissionDecision"], "deny",
            "dynamic script reader unexpectedly reached Host allow: {terminal}"
        );
        assert!(
            terminal["hookSpecificOutput"]["permissionDecisionReason"]
                .as_str()
                .is_some_and(|message| message.contains("Direct Markdown reads are denied")),
            "{terminal}"
        );
        assert!(output.stderr.is_empty());
    }
}

#[test]
fn default_hook_process_rejects_retired_policy_bundle_flag() {
    let _test_guard = hook_process_test_guard();
    let mut child = hook_command()
        .args([
            "pre-tool",
            "--client",
            "codex",
            "--policy-bundle",
            "/tmp/not-a-serving-policy.json",
            "--host-match",
            "Bash",
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn default Hook binary");
    serde_json::to_writer(
        child.stdin.as_mut().expect("Hook stdin"),
        &serde_json::json!({
            "tool_name": "Bash",
            "tool_input": {"command": "rg HookDecision fixture.rs"}
        }),
    )
    .expect("write Host payload");
    drop(child.stdin.take());
    let output = child.wait_with_output().expect("wait for Hook binary");
    let terminal: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("valid Hook Host JSON");
    assert_eq!(output.status.code(), Some(0));
    assert_eq!(terminal["hookSpecificOutput"]["permissionDecision"], "deny");
    assert!(
        terminal["hookSpecificOutput"]["permissionDecisionReason"]
            .as_str()
            .is_some_and(|message| message.contains("--policy-bundle is not a supported")),
        "retired policy flag must not be silently accepted: {terminal}"
    );
    assert!(output.stderr.is_empty());
}

#[test]
fn invalid_state_home_config_fails_closed_on_the_default_hook_path() {
    let _test_guard = hook_process_test_guard();
    let state_home = tempfile::tempdir().expect("create State Home fixture");
    let config_path = state_home.path().join("hooks/config.toml");
    std::fs::create_dir_all(
        config_path
            .parent()
            .expect("State Home Hook config has parent"),
    )
    .expect("create State Home Hook config root");
    std::fs::write(&config_path, "[[rules]\nid = [\n")
        .expect("write invalid State Home Hook config");

    let mut child = hook_command()
        .args(["pre-tool", "--client", "codex", "--host-match", "Bash"])
        .env("ASP_STATE_HOME", state_home.path())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn default Hook binary");
    serde_json::to_writer(
        child.stdin.as_mut().expect("Hook stdin"),
        &serde_json::json!({
            "tool_name": "Bash",
            "tool_input": {"command": "rg HookDecision fixture.rs"}
        }),
    )
    .expect("write Host payload");
    drop(child.stdin.take());
    let output = child.wait_with_output().expect("wait for Hook binary");
    let terminal: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("valid Hook Host JSON");
    assert_eq!(output.status.code(), Some(0));
    assert_eq!(terminal["hookSpecificOutput"]["permissionDecision"], "deny");
    assert!(
        terminal["hookSpecificOutput"]["permissionDecisionReason"]
            .as_str()
            .is_some_and(|message| message.contains("failed to parse")),
        "invalid State Home config must fail closed with the parse evidence: {terminal}"
    );
    assert!(output.stderr.is_empty());
}

#[test]
fn repeated_hook_subcommand_is_not_a_host_event_namespace() {
    let _test_guard = hook_process_test_guard();
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
    let _test_guard = hook_process_test_guard();
    let mut child = hook_command()
        .args(["pre-tool", "--client", "codex", "--host-match", "Bash"])
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
    let _test_guard = hook_process_test_guard();
    let temp = tempfile::tempdir().expect("temporary Hook fixture");
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
            .args(["pre-tool", "--client", "codex", "--host-match", "Bash"])
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
fn command_local_no_agent_escape_precedes_the_default_policy() {
    let _test_guard = hook_process_test_guard();
    for command in [
        "ASP_NO_AGENT=1 rg HookDecision fixture.rs",
        "/usr/bin/env ASP_NO_AGENT=1 rg HookDecision fixture.rs",
        "export ASP_NO_AGENT=1; rg HookDecision fixture.rs",
        "export ASP_NO_AGENT=1; exec rg HookDecision fixture.rs",
    ] {
        let mut child = hook_command()
            .args(["pre-tool", "--client", "codex", "--host-match", "Bash"])
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
        .args(["pre-tool", "--client", "codex", "--host-match", "Bash"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("spawn negative Hook binary");
    serde_json::to_writer(
        child.stdin.as_mut().expect("Hook stdin"),
        &serde_json::json!({
            "tool_name": "Bash",
            "tool_input": {"command": "printf 'ASP_NO_AGENT=1'; rg HookDecision fixture.rs"}
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
    let mut child = hook_command()
        .args(["pre-tool", "--client", "codex", "--host-match", "Bash"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("spawn lookalike-variable Hook binary");
    serde_json::to_writer(
        child.stdin.as_mut().expect("Hook stdin"),
        &serde_json::json!({
            "tool_name": "Bash",
            "tool_input": {"command": "ASP_NO_ASP=1 rg HookDecision fixture.rs"}
        }),
    )
    .expect("write lookalike-variable Host payload");
    drop(child.stdin.take());
    let output = child
        .wait_with_output()
        .expect("wait for lookalike-variable Hook binary");
    let terminal: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("valid fail-closed Host JSON");
    assert_eq!(output.status.code(), Some(0));
    assert_eq!(terminal["hookSpecificOutput"]["permissionDecision"], "deny");
}
