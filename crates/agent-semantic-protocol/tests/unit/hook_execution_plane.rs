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
        include_str!("../../src/command/hook_runtime_agent_session_dispatch.rs"),
        include_str!("../../src/command/hook_runtime_source_access_materialize.rs"),
    ] {
        for forbidden in [
            "RuntimeServerClientExecutor",
            "runtime_server_workspace_session",
            "ensure_runtime_generation",
            "submit_runtime_generation_mutation",
            "server reconcile",
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
    let dispatch = include_str!("../../src/command/hook_runtime_agent_session_dispatch.rs");
    for forbidden in [
        "configured_resident_target(",
        "requiredForkTurns",
        "materialize_host_proven_resident_execution",
        "materialize_resident_dispatch_wrapper",
    ] {
        assert!(
            !dispatch.contains(forbidden),
            "Hook reimplemented Codex scheduling through {forbidden}"
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
    assert!(
        !std::path::Path::new(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../asp-codex-plugin/hooks/hooks.json"
        ))
        .exists()
    );

    let bootstrap = include_str!("../../src/hook_bootstrap.rs");
    assert!(bootstrap.contains("bootstrap-local-action-passthrough"));
    assert!(bootstrap.contains("local-policy-evaluator"));
    assert!(!bootstrap.contains("evaluate_hook_event_via_runtime"));
}

#[test]
fn hook_recovery_references_the_org_agent_window_without_rust_owned_choice_panes() {
    let execution = include_str!("../../src/command/hook_runtime_agent_session_dispatch.rs");
    let event_state = include_str!("../../../agent-semantic-hook/src/event_state.rs");
    let event_replay = include_str!("../../../agent-semantic-hook/src/event_replay.rs");
    let config = include_str!("../../../agent-semantic-config/templates/hooks/config.toml");

    for source in [execution, event_state, event_replay, config] {
        assert!(source.contains("asp session"));
        assert!(!source.contains("asp session @"));
        for legacy in [
            "choice-pane",
            "choice pane",
            "bootstrap-pane",
            "bootstrap pane",
            "asp agent session bootstrap",
        ] {
            assert!(
                !source.contains(legacy),
                "Hook execution recovery must not implement a Rust-owned ChoicePlane: {legacy}"
            );
        }
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
    assert!(
        context.contains("compiled-and-published"),
        "cold Hook invocation must publish its immutable matcher snapshot: {context}"
    );
    assert!(
        context.contains("\"agentWindowCommand\":\"asp session --agents choice-plane\""),
        "{context}"
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
