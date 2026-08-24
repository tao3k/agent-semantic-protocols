fn refresh_fixture_matcher(
    workspace: &std::path::Path,
    home: &std::path::Path,
    state_home: &std::path::Path,
) -> String {
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_asp"))
        .current_dir(workspace)
        .args(["hook", "refresh", "--client", "codex"])
        .env_clear()
        .env("HOME", home)
        .env("ASP_STATE_HOME", state_home)
        .output()
        .expect("run path-free Hook matcher refresh");
    let stdout = String::from_utf8(output.stdout).expect("Hook refresh stdout UTF-8");
    let stderr = String::from_utf8(output.stderr).expect("Hook refresh stderr UTF-8");
    assert!(output.status.success(), "stdout={stdout} stderr={stderr}");
    assert!(
        stdout.contains("binarySchemaVersion=1"),
        "refresh omitted Binary v1 receipt: {stdout}"
    );
    stdout
}

#[test]
fn generic_runtime_hook_policy_plane_is_absent() {
    let hook = include_str!("../../src/command/hook.rs");
    let bootstrap = include_str!("../../src/hook_bootstrap.rs");
    let runtime = include_str!("../../src/server/runtime_server.rs");
    let daemon = include_str!("../../src/server/runtime_server_daemon.rs");
    let ipc = include_str!("../../../agent-semantic-client-db/src/workspace_db_ipc/protocol.rs");
    let ipc_server =
        include_str!("../../../agent-semantic-client-db/src/workspace_db_ipc_server.rs");

    assert!(hook.contains("evaluate_hook_event_locally"));
    assert!(bootstrap.contains("codex_tool_event_requires_policy_evaluation"));
    assert!(bootstrap.contains("bootstrap-no-agent-bypass"));
    for source in [hook, bootstrap, runtime, daemon, ipc, ipc_server] {
        assert!(!source.contains("EvaluateHook"));
        assert!(!source.contains("HookEvaluationBuilder"));
        assert!(!source.contains("runtime_server_hook_evaluation_client"));
        assert!(!source.contains("ResidentHookSnapshotAuthority"));
    }
}

#[test]
fn hook_event_plane_has_zero_runtime_server_dependencies() {
    let manifest = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    assert!(
        !manifest
            .join("src/command/hook_runtime_generation_admission.rs")
            .exists(),
        "Hook generation admission must be owned by Runtime freshness, not Hook"
    );

    for source in [
        include_str!("../../src/command/hook.rs"),
        include_str!("../../src/hook_bootstrap.rs"),
        include_str!("../../src/command/hook_runtime.rs"),
        include_str!("../../src/command/hook_runtime_config_recovery.rs"),
    ] {
        for forbidden in [
            "RuntimeServerClientExecutor",
            "runtime_server_workspace_session",
            "ensure_runtime_generation",
            "submit_runtime_generation_mutation",
            "server start",
        ] {
            assert!(
                !source.contains(forbidden),
                "Hook event plane reintroduced Runtime Server dependency: {forbidden}"
            );
        }
    }

    let runtime = include_str!("../../src/command/hook_runtime.rs");
    for forbidden in [
        "enforce_resident_child_deny_contract",
        "has_recorded_subagent_context",
        "payload_live_target_resident_identity_proof",
        "payload_live_target_resident_identity_status",
        "AgentSessionRegistry",
        "load_activation",
        "record_active_context",
    ] {
        assert!(
            !runtime.contains(forbidden),
            "synchronous Hook evaluation reintroduced Agent Session or rollout I/O: {forbidden}"
        );
    }
    let manifest = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    for legacy in [
        "src/command/hook_runtime_agent_session.rs",
        "src/command/hook_runtime_agent_session_execution.rs",
        "src/command/hook_runtime_agent_session_rollout_topology.rs",
        "src/command/hook_runtime_resident_permissions.rs",
    ] {
        assert!(
            !manifest.join(legacy).exists(),
            "Hook Agent Session legacy owner was reintroduced: {legacy}"
        );
    }

    let snapshot = include_str!("../../src/command/hook_runtime_config_recovery.rs");
    assert!(snapshot.contains("MmapOptions"));
    assert!(snapshot.contains("durable_snapshot_config"));
    assert!(snapshot.contains("from_durable_snapshot_config"));
    assert!(!snapshot.contains("OnceLock"));
    assert!(!snapshot.contains("managed_hook_config::materialize"));
    assert!(!snapshot.contains("load_asp_session_policy"));

    let event_state = include_str!("../../../agent-semantic-hook/src/event_state.rs");
    assert!(event_state.contains("try_lock_exclusive"));
    assert!(event_state.contains("HOOK_EVENT_STATE_LOCK_TIMEOUT"));
    assert!(event_state.contains("Duration::from_millis(100)"));
    assert!(!event_state.contains("OnceLock<Mutex"));
}

#[test]
fn codex_wildcard_is_transport_coverage_not_runtime_routing() {
    let rendered = agent_semantic_hook::codex_global_hook_block_with_binary(Some(
        std::path::Path::new("/state/runtime/bin/asp"),
    ));
    let hooks: toml::Value = toml::from_str(&rendered).expect("parse global inline Codex hooks");
    for event in ["PreToolUse", "PermissionRequest"] {
        assert_eq!(hooks["hooks"][event][0]["matcher"].as_str(), Some("*"));
    }
    let plugin: serde_json::Value = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../asp-codex-plugin/hooks/hooks.json"
    )))
    .expect("parse fixed Codex plugin Hook transport");
    for event in ["PreToolUse", "PermissionRequest"] {
        assert_eq!(plugin["hooks"][event][0]["matcher"], "*");
        assert_eq!(
            plugin["hooks"][event][0]["hooks"][0]["command"],
            format!(
                "\"$PLUGIN_ROOT/bin/asp-hook\" {} --client codex",
                if event == "PreToolUse" {
                    "pre-tool"
                } else {
                    "permission-request"
                }
            )
        );
    }

    let bootstrap = include_str!("../../src/hook_bootstrap.rs");
    assert!(bootstrap.contains("bootstrap-local-action-passthrough"));
    assert!(bootstrap.contains("local-policy-evaluator"));
    assert!(!bootstrap.contains("evaluate_hook_event_via_runtime"));
}

#[test]
fn explicit_no_agent_environment_bypasses_host_hook_before_payload_evaluation() {
    use std::process::{Command, Stdio};

    let binary = env!("CARGO_BIN_EXE_asp");
    let warm = Command::new(binary)
        .arg("--version")
        .env_clear()
        .output()
        .expect("warm Hook binary image");
    assert!(warm.status.success());

    for event in [
        "pre-tool",
        "permission-request",
        "post-tool",
        "stop",
        "notification",
        "user-prompt",
        "session-start",
        "subagent-start",
        "subagent-stop",
    ] {
        let mut child = Command::new(binary)
            .args(["hook", event, "--client", "codex"])
            .env_clear()
            .env("ASP_NO_AGENT", "1")
            .env("ASP_HOOK_BOOTSTRAP_TRACE", "1")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("spawn no-agent Hook bypass");
        let mut held_open_stdin = Some(child.stdin.take().expect("piped Hook stdin"));
        let started = std::time::Instant::now();
        let status = loop {
            if let Some(status) = child.try_wait().expect("poll no-agent Hook bypass") {
                break status;
            }
            if started.elapsed() >= std::time::Duration::from_millis(750) {
                child.kill().expect("kill blocked no-agent Hook bypass");
                drop(held_open_stdin.take());
                let output = child
                    .wait_with_output()
                    .expect("collect blocked no-agent Hook bypass output");
                let stdout = String::from_utf8_lossy(&output.stdout);
                let stderr = String::from_utf8_lossy(&output.stderr);
                panic!(
                    "ASP_NO_AGENT Hook bypass waited for stdin or another synchronous dependency: event={event} stdout={stdout} stderr={stderr}"
                );
            }
            std::thread::sleep(std::time::Duration::from_millis(1));
        };
        drop(held_open_stdin.take());
        let output = child
            .wait_with_output()
            .expect("collect no-agent Hook bypass output");
        let stdout = String::from_utf8(output.stdout).expect("Hook bypass stdout UTF-8");
        let stderr = String::from_utf8(output.stderr).expect("Hook bypass stderr UTF-8");

        assert!(
            status.success(),
            "event={event} stdout={stdout} stderr={stderr}"
        );
        assert_eq!(stdout.trim(), "{}", "event={event}");
        assert!(
            stderr.contains("route=process-entry-no-agent-bypass"),
            "event={event} stderr={stderr}"
        );
        assert!(
            !stderr.contains("local-policy-evaluator"),
            "event={event} stderr={stderr}"
        );
        assert!(
            !stderr.contains("runtime-server"),
            "event={event} stderr={stderr}"
        );
    }
}

#[test]
fn legacy_rust_choice_plane_owners_are_absent() {
    let manifest = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    for relative in [
        "src/command/agent_session_registry_bootstrap.rs",
        "src/command/agent_session_registry_bootstrap_render.rs",
        "src/command/agent_session_registry_args.rs",
        "../agent-semantic-client-db/src/agent_session_registry/interactive_loop.rs",
        "../agent-semantic-client-db/src/agent_session_registry/interactive_loop_types.rs",
        "../agent-semantic-loop/src/choice.rs",
    ] {
        assert!(
            !manifest.join(relative).exists(),
            "legacy Rust ChoicePlane owner was reintroduced: {relative}"
        );
    }

    let decision_schema =
        include_str!("../../../../schemas/semantic-agent-hook-decision.v1.schema.json");
    assert!(!decision_schema.contains("interactiveCommand"));
}

#[test]
fn unrelated_action_binary_path_does_not_require_runtime_server() {
    use std::io::Write;
    use std::process::{Command, Stdio};

    let binary = env!("CARGO_BIN_EXE_asp");
    let mut child = Command::new(binary)
        .args(["hook", "pre-tool", "--client", "codex"])
        .env_clear()
        .env("ASP_HOOK_BOOTSTRAP_TRACE", "1")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn Hook binary");
    let mut stdin = child.stdin.take().expect("Hook stdin");
    stdin
        .write_all(br#"{"tool_name":"update_plan","tool_input":{"plan":[]}}"#)
        .expect("write Hook payload");
    drop(stdin);
    let output = child.wait_with_output().expect("wait for Hook binary");
    let stdout = String::from_utf8(output.stdout).expect("Hook stdout is UTF-8");
    let stderr = String::from_utf8(output.stderr).expect("Hook stderr is UTF-8");

    assert!(output.status.success(), "stderr: {stderr}");
    assert_eq!(stdout.trim(), "{}");
    assert!(
        stderr.contains("route=bootstrap-local-action-passthrough"),
        "stderr: {stderr}"
    );
    assert!(!stderr.contains("runtime-server"), "stderr: {stderr}");
    let execution_micros = stderr
        .lines()
        .find_map(|line| {
            line.strip_prefix("[asp-hook] route=bootstrap-local-action-emitted elapsedMicros=")
        })
        .expect("Hook execution latency trace")
        .parse::<u128>()
        .expect("Hook execution latency is an integer");
    assert!(
        execution_micros < 100_000,
        "unrelated Hook execution exceeded 100ms after process entry: binary={binary} executionMicros={execution_micros} stderr={stderr}"
    );
}

#[test]
fn structured_rust_read_binary_path_is_local_bounded_and_runtime_free() {
    use std::io::Write;
    use std::process::{Command, Stdio};
    use std::time::{SystemTime, UNIX_EPOCH};

    let workspace = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(std::path::Path::parent)
        .expect("workspace root");
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock after epoch")
        .as_nanos();
    let root = std::env::temp_dir().join(format!(
        "asp-hook-local-read-{}-{nonce}",
        std::process::id()
    ));
    let state_home = root.join("state");
    std::fs::create_dir_all(state_home.join("hooks")).expect("create isolated Hook state");
    std::fs::create_dir_all(state_home.join("agents")).expect("create isolated Agent registry");
    std::fs::write(
        state_home.join("hooks/config.toml"),
        agent_semantic_hook::default_client_config_template(),
    )
    .expect("write isolated managed Hook config");
    for (name, source) in [
        (
            "config.toml",
            include_str!("../../../../agents/config.toml"),
        ),
        (
            "asp_explorer_codex.toml",
            include_str!("../../../../agents/asp_explorer_codex.toml"),
        ),
        (
            "asp_explorer_claude.md",
            include_str!("../../../../agents/asp_explorer_claude.md"),
        ),
        (
            "asp_testing_codex.toml",
            include_str!("../../../../agents/asp_testing_codex.toml"),
        ),
        (
            "asp_testing_claude.md",
            include_str!("../../../../agents/asp_testing_claude.md"),
        ),
    ] {
        std::fs::write(state_home.join("agents").join(name), source)
            .expect("write isolated Agent registry projection");
    }

    let refresh = refresh_fixture_matcher(workspace, &root, &state_home);
    assert!(refresh.contains("generation="), "{refresh}");

    let binary = env!("CARGO_BIN_EXE_asp");
    let mut child = Command::new(binary)
        .current_dir(workspace)
        .args(["hook", "pre-tool", "--client", "codex"])
        .env_clear()
        .env("HOME", &root)
        .env("ASP_STATE_HOME", &state_home)
        .env("ASP_HOOK_BOOTSTRAP_TRACE", "1")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn isolated Hook binary");
    let payload = serde_json::json!({
        "session_id": "local-structured-read-regression",
        "cwd": workspace,
        "hook_event_name": "PreToolUse",
        "tool_name": "Read",
        "tool_input": {
            "file_path": "crates/agent-semantic-hook/src/protocol.rs"
        }
    });
    serde_json::to_writer(child.stdin.as_mut().expect("Hook stdin"), &payload)
        .expect("write structured Read payload");
    child
        .stdin
        .as_mut()
        .expect("Hook stdin")
        .flush()
        .expect("flush Hook payload");
    drop(child.stdin.take());
    let output = child.wait_with_output().expect("wait for Hook binary");
    let stdout = String::from_utf8(output.stdout).expect("Hook stdout is UTF-8");
    let stderr = String::from_utf8(output.stderr).expect("Hook stderr is UTF-8");

    assert!(output.status.success(), "stdout={stdout} stderr={stderr}");
    let response: serde_json::Value = serde_json::from_str(&stdout).expect("parse Hook response");
    assert_eq!(
        response["hookSpecificOutput"]["permissionDecision"], "deny",
        "stdout={stdout} stderr={stderr}"
    );
    let context = response["hookSpecificOutput"]["additionalContext"]
        .as_str()
        .expect("typed Hook decision context");
    assert!(context.contains("direct-source-read"), "{context}");
    assert!(context.contains("mmap-hit"), "{context}");
    assert!(
        context.contains("asp rust search owner crates/agent-semantic-hook/src/protocol.rs items --workspace . --view seeds"),
        "source deny must carry the exact parser-owned recovery command: {context}"
    );
    assert!(!context.contains("asp session @"), "{context}");
    assert!(!stderr.contains("runtime-server"), "stderr={stderr}");
    let execution_micros = stderr
        .lines()
        .find_map(|line| {
            line.strip_prefix(
                "[asp-hook] route=local-policy-evaluator stage=complete elapsedMicros=",
            )
        })
        .expect("Hook local execution completion trace")
        .parse::<u128>()
        .expect("Hook execution latency is an integer");
    assert!(
        execution_micros < 100_000,
        "structured Read Hook execution exceeded 100ms: executionMicros={execution_micros} stderr={stderr}"
    );

    let mut warm = Command::new(binary)
        .current_dir(workspace)
        .args(["hook", "pre-tool", "--client", "codex"])
        .env_clear()
        .env("HOME", &root)
        .env("ASP_STATE_HOME", &state_home)
        .env("ASP_HOOK_BOOTSTRAP_TRACE", "1")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn warm Hook binary");
    serde_json::to_writer(warm.stdin.as_mut().expect("warm Hook stdin"), &payload)
        .expect("write warm Hook payload");
    warm.stdin
        .as_mut()
        .expect("warm Hook stdin")
        .flush()
        .expect("flush warm Hook payload");
    drop(warm.stdin.take());
    let warm = warm.wait_with_output().expect("wait for warm Hook binary");
    let warm_stdout = String::from_utf8(warm.stdout).expect("warm Hook stdout is UTF-8");
    let warm_stderr = String::from_utf8(warm.stderr).expect("warm Hook stderr is UTF-8");
    assert!(
        warm.status.success(),
        "stdout={warm_stdout} stderr={warm_stderr}"
    );
    let warm_response: serde_json::Value =
        serde_json::from_str(&warm_stdout).expect("parse warm Hook response");
    let warm_context = warm_response["hookSpecificOutput"]["additionalContext"]
        .as_str()
        .expect("warm typed Hook decision context");
    assert!(
        warm_context.contains("mmap-hit"),
        "warm Hook invocation must load the process-independent snapshot: {warm_context}"
    );
    let warm_execution_micros = warm_stderr
        .lines()
        .find_map(|line| {
            line.strip_prefix(
                "[asp-hook] route=local-policy-evaluator stage=complete elapsedMicros=",
            )
        })
        .expect("warm Hook local execution completion trace")
        .parse::<u128>()
        .expect("warm Hook execution latency is an integer");
    assert!(
        warm_execution_micros < 100_000,
        "warm structured Read Hook execution exceeded 100ms: executionMicros={warm_execution_micros} stderr={warm_stderr}"
    );

    fn find_compiled_matcher(path: &std::path::Path) -> Option<std::path::PathBuf> {
        let entries = std::fs::read_dir(path).ok()?;
        for entry in entries.flatten() {
            let candidate = entry.path();
            if candidate.is_dir() {
                if let Some(found) = find_compiled_matcher(&candidate) {
                    return Some(found);
                }
            } else if candidate.file_name().and_then(|name| name.to_str())
                == Some("active-matcher.v1.bin")
            {
                return Some(candidate);
            }
        }
        None
    }
    let snapshot = find_compiled_matcher(&state_home).expect("compiled matcher snapshot");
    let matcher_dir = snapshot.parent().expect("active matcher parent");
    let published_files = std::fs::read_dir(matcher_dir)
        .expect("read active matcher directory")
        .filter_map(Result::ok)
        .filter(|entry| entry.file_type().is_ok_and(|kind| kind.is_file()))
        .collect::<Vec<_>>();
    assert!(
        published_files.iter().all(|entry| {
            entry
                .file_name()
                .to_str()
                .is_some_and(|name| name.starts_with("active-") && name.ends_with(".v1.bin"))
        }),
        "Hook hot path may own atomic policy shards but no pointer/generation chain: {published_files:?}"
    );
    for entry in &published_files {
        std::fs::write(entry.path(), b"corrupt matcher snapshot")
            .expect("corrupt fixture matcher shard");
    }
    refresh_fixture_matcher(workspace, &root, &state_home);
    let mut recovery = Command::new(binary)
        .current_dir(workspace)
        .args(["hook", "pre-tool", "--client", "codex"])
        .env_clear()
        .env("HOME", &root)
        .env("ASP_STATE_HOME", &state_home)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn recovery Hook binary");
    serde_json::to_writer(
        recovery.stdin.as_mut().expect("recovery Hook stdin"),
        &payload,
    )
    .expect("write recovery Hook payload");
    drop(recovery.stdin.take());
    let recovery = recovery
        .wait_with_output()
        .expect("wait for recovery Hook binary");
    assert!(recovery.status.success());
    let recovery_stdout = String::from_utf8(recovery.stdout).expect("recovery stdout UTF-8");
    let recovery_response: serde_json::Value =
        serde_json::from_str(&recovery_stdout).expect("parse recovery Hook response");
    let recovery_context = recovery_response["hookSpecificOutput"]["additionalContext"]
        .as_str()
        .expect("recovery Hook context");
    assert!(recovery_context.contains("mmap-hit"), "{recovery_context}");
    let repaired = std::fs::read(&snapshot).expect("read repaired matcher snapshot");
    assert_eq!(&repaired[..8], b"ASPHK1PC");
    assert!(
        repaired.len() > 120,
        "repaired Binary v1 bundle omitted its section index"
    );
    std::fs::remove_dir_all(root).expect("cleanup isolated Hook state");
}

#[test]
fn runtime_control_endpoint_is_published_before_optional_telemetry_starts() {
    let daemon = include_str!("../../src/server/runtime_server_daemon.rs");
    let endpoint = daemon
        .find("RuntimeServer::bind_and_publish_with_artifact_catalog")
        .expect("Runtime Server endpoint publication");
    let telemetry = daemon
        .find("RuntimeServerOpenTelemetry::start")
        .expect("optional Runtime Server OpenTelemetry startup");

    assert!(
        endpoint < telemetry,
        "optional telemetry must not gate Runtime Server control-plane publication"
    );
}

#[test]
fn control_plane_refresh_repairs_managed_config_before_hook_evaluation() {
    use sha2::{Digest, Sha256};
    use std::io::Write;
    use std::process::{Command, Stdio};
    use std::time::{SystemTime, UNIX_EPOCH};

    let workspace = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(std::path::Path::parent)
        .expect("workspace root");
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock after epoch")
        .as_nanos();
    let root = std::env::temp_dir().join(format!(
        "asp-hook-managed-config-auto-sync-{}-{nonce}",
        std::process::id()
    ));
    let state_home = root.join("state");
    let config_path = state_home.join("hooks/config.toml");
    std::fs::create_dir_all(config_path.parent().expect("config parent"))
        .expect("create isolated Hook config root");

    let expected_fingerprint = agent_semantic_config::hook_client_contract_fingerprint();
    let stale = agent_semantic_hook::default_client_config_template()
        .replace(&expected_fingerprint, "hook-client-v1-intentionally-stale");
    assert_ne!(
        stale,
        agent_semantic_hook::default_client_config_template(),
        "fixture must contain the current contract fingerprint"
    );
    std::fs::write(&config_path, &stale).expect("write stale managed Hook config");
    std::fs::write(
        state_home.join("hooks/config.toml.managed.sha256"),
        format!("{:x}", Sha256::digest(stale.as_bytes())),
    )
    .expect("write matching managed ownership sidecar");

    refresh_fixture_matcher(workspace, &root, &state_home);

    let payload = serde_json::json!({
        "session_id": "managed-config-auto-sync",
        "cwd": workspace,
        "hook_event_name": "PreToolUse",
        "tool_name": "Read",
        "tool_input": {
            "file_path": "crates/agent-semantic-hook/src/protocol.rs"
        }
    });
    let mut child = Command::new(env!("CARGO_BIN_EXE_asp"))
        .current_dir(workspace)
        .args(["hook", "pre-tool", "--client", "codex"])
        .env_clear()
        .env("HOME", &root)
        .env("ASP_STATE_HOME", &state_home)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn Hook with stale managed config");
    serde_json::to_writer(child.stdin.as_mut().expect("Hook stdin"), &payload)
        .expect("write Hook payload");
    child
        .stdin
        .as_mut()
        .expect("Hook stdin")
        .flush()
        .expect("flush Hook payload");
    drop(child.stdin.take());
    let output = child
        .wait_with_output()
        .expect("wait for Hook mmap evaluation");
    let stdout = String::from_utf8(output.stdout).expect("Hook stdout UTF-8");
    let stderr = String::from_utf8(output.stderr).expect("Hook stderr UTF-8");
    assert!(output.status.success(), "stdout={stdout} stderr={stderr}");
    assert!(
        !stdout.contains("hook-local-policy-unavailable"),
        "published matcher must not emit a recovery failure: {stdout}"
    );
    let response: serde_json::Value =
        serde_json::from_str(&stdout).expect("parse Hook decision response");
    let context = response["hookSpecificOutput"]["additionalContext"]
        .as_str()
        .expect("typed Hook decision context");
    assert!(context.contains("mmap-hit"), "{context}");
    assert!(
        context.contains("Registered rust source reads are denied"),
        "{context}"
    );
    assert!(
        context.contains(
            "asp rust search owner crates/agent-semantic-hook/src/protocol.rs items --workspace . --view seeds"
        ),
        "{context}"
    );

    let refreshed = std::fs::read_to_string(&config_path).expect("read refreshed Hook config");
    assert!(refreshed.contains(&expected_fingerprint));
    std::fs::remove_dir_all(root).expect("cleanup isolated Hook state");
}
