fn typed_hook_decision_context(response: &serde_json::Value) -> serde_json::Value {
    let context = response["hookSpecificOutput"]["additionalContext"]
        .as_str()
        .expect("typed Hook decision context");
    serde_json::from_str(
        context
            .strip_prefix("[agent-hook-decision] ")
            .expect("typed Hook decision prefix"),
    )
    .expect("typed Hook decision JSON")
}

#[cfg(target_os = "macos")]
fn publish_fixture_generation(
    workspace: &std::path::Path,
    home: &std::path::Path,
    state_home: &std::path::Path,
) -> String {
    use agent_semantic_artifacts::hook_generation::{
        HookGenerationCandidate, commit_hook_generation, prepare_hook_generation,
    };

    let candidate = state_home.join("hooks/test-candidate");
    std::fs::create_dir_all(&candidate).expect("create HookGeneration test candidate");
    let config = agent_semantic_config::default_hook_client_config_template().into_bytes();
    let config_path = candidate.join("config.toml");
    std::fs::write(&config_path, &config).expect("write candidate config");
    let canonical = agent_semantic_config::default_hook_client_config_file()
        .expect("canonical HookGeneration config");
    let compiled = agent_semantic_hook::aot_compiler::compile_aot_hook_generation(
        &canonical,
        "candidate-unpublished",
    )
    .expect("compile HookGeneration fixture");
    let registry = std::fs::read(workspace.join("agents/config.toml"))
        .expect("read canonical Agent registry fixture");
    let prepared = prepare_hook_generation(
        state_home,
        HookGenerationCandidate {
            hook_binary: &workspace.join("target/debug/asp-hook"),
            config: &config,
            compiled_matcher: &compiled,
            registry: &registry,
        },
    )
    .expect("prepare HookGeneration fixture");
    let subject = "asp-hook-candidate-validation.rs";
    let payload = serde_json::json!({
        "session_id": "hook-generation-candidate-validation",
        "cwd": workspace,
        "hook_event_name": "PreToolUse",
        "tool_name": "Bash",
        "tool_input": {
            "command": reader_probe_command(home, subject)
        }
    });
    let command = payload["tool_input"]["command"]
        .as_str()
        .expect("candidate Reader command");
    let component = agent_semantic_hook::diagnose_reader_probe_with_state_home(
        agent_semantic_hook::semantic_shell_tokens(command),
        subject.to_owned(),
        Vec::new(),
        state_home,
    )
    .expect("candidate Reader component observation");
    assert_eq!(
        component.access,
        agent_semantic_hook::ReaderProbeAccess::Read,
        "component={component:?}"
    );
    assert!(
        matches!(
            component.terminal.as_str(),
            "open-entry-observed" | "reader-behavior-cache-hit"
        ),
        "component={component:?}"
    );
    assert!(
        component.elapsed_micros < 150_000,
        "component={component:?}"
    );
    assert!(component.cleanup_verified);
    let validation = run_codex_pre_tool_binding_probe_with_generation(
        &prepared.receipt.generation_path,
        &prepared.receipt.generation_digest,
        state_home,
        &payload,
    );
    assert!(
        validation.status.success(),
        "candidate Reader PreTool validation failed: status={} stdout={} stderr={}",
        validation.status,
        String::from_utf8_lossy(&validation.stdout),
        String::from_utf8_lossy(&validation.stderr)
    );
    let validation: serde_json::Value = serde_json::from_slice(&validation.stdout)
        .expect("candidate Reader PreTool validation JSON");
    let context = validation["hookSpecificOutput"]["additionalContext"]
        .as_str()
        .expect("candidate Reader PreTool validation context");
    assert!(context.contains("\"accessMode\":\"O_RDONLY\""), "{context}");
    assert!(context.contains("\"cleanupVerified\":true"), "{context}");
    assert!(context.contains("\"access\":\"read\""), "{context}");
    assert!(
        context.contains("\"evidence\":\"reader-behavior-dynamic-cache\"")
            || context.contains("\"evidence\":\"reader-probe-open-read-only\""),
        "{context}"
    );
    assert!(
        context.contains(prepared.receipt.generation_digest.as_str()),
        "{context}"
    );
    let publication =
        commit_hook_generation(state_home, &prepared).expect("commit HookGeneration fixture");
    publication.generation.generation_digest.to_string()
}

#[cfg(target_os = "macos")]
fn run_codex_pre_tool_binding_probe_with_generation(
    generation_path: &std::path::Path,
    generation_digest: &str,
    state_home: &std::path::Path,
    payload: &serde_json::Value,
) -> std::process::Output {
    use std::io::Write;
    use std::process::{Command, Stdio};

    let mut child = Command::new(
        generation_path
            .parent()
            .expect("HookGeneration root")
            .join("asp-hook"),
    )
    .args([
        "hook",
        "pre-tool",
        "--client",
        "codex",
        "--host-match",
        "Bash",
    ])
    .env(
        "ASP_HOOK_GENERATION_ROOT",
        generation_path.parent().expect("HookGeneration root"),
    )
    .env("ASP_HOOK_GENERATION_DIGEST", generation_digest)
    .env("ASP_STATE_HOME", state_home)
    .env_remove("ASP_NO_AGENT")
    .stdin(Stdio::piped())
    .stdout(Stdio::piped())
    .stderr(Stdio::piped())
    .spawn()
    .expect("spawn candidate Reader PreTool validation");
    serde_json::to_writer(
        child.stdin.as_mut().expect("candidate validation stdin"),
        payload,
    )
    .expect("write candidate validation payload");
    child
        .stdin
        .as_mut()
        .expect("candidate validation stdin")
        .flush()
        .expect("flush candidate validation payload");
    drop(child.stdin.take());
    child
        .wait_with_output()
        .expect("wait for candidate Reader PreTool validation")
}

fn run_codex_pre_tool_binding_probe(
    binding_args: &[&str],
    payload: &serde_json::Value,
) -> std::process::Output {
    use std::io::Write;
    use std::process::{Command, Stdio};

    let mut child = Command::new(env!("CARGO_BIN_EXE_asp"))
        .args(["hook", "pre-tool", "--client", "codex"])
        .args(binding_args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn Codex PreTool binding probe");
    serde_json::to_writer(child.stdin.as_mut().expect("binding probe stdin"), payload)
        .expect("write binding probe payload");
    child
        .stdin
        .as_mut()
        .expect("binding probe stdin")
        .flush()
        .expect("flush binding probe payload");
    drop(child.stdin.take());
    child.wait_with_output().expect("wait for binding probe")
}

async fn run_fixture_hook(
    workspace: &std::path::Path,
    home: &std::path::Path,
    state_home: &std::path::Path,
    payload: &serde_json::Value,
    trace: bool,
) -> agent_semantic_hook_testkit::HookScenarioReceipt {
    let mut spec = agent_semantic_hook_testkit::HookProcessSpec::new(
        workspace.join("asp-codex-plugin/bin/asp-hook"),
        workspace,
    );
    spec.args = vec![
        "pre-tool".to_owned(),
        "--client".to_owned(),
        "codex".to_owned(),
        "--host-match".to_owned(),
        "Bash".to_owned(),
    ];
    spec.env
        .push(("HOME".to_owned(), home.display().to_string()));
    spec.env.push((
        "ASP_STATE_HOME".to_owned(),
        state_home.display().to_string(),
    ));
    if trace {
        spec.env
            .push(("ASP_HOOK_BOOTSTRAP_TRACE".to_owned(), "1".to_owned()));
    }
    agent_semantic_hook_testkit::run_hook_process(&spec, payload)
        .await
        .expect("bounded Hook process terminal")
}

#[cfg(target_os = "macos")]
fn reader_probe_command(_root: &std::path::Path, subject: &str) -> String {
    let fixture = agent_semantic_hook::materialize_reader_probe_fixture()
        .expect("materialize canonical Reader probe fixture");
    format!("{} read {subject}", fixture.display())
}

#[test]
fn generic_runtime_hook_policy_plane_is_absent() {
    let hook = include_str!("../../src/command/hook.rs");
    let bootstrap = include_str!("../../src/hook_bootstrap.rs");
    let runtime = include_str!("../../src/server/runtime_server.rs");
    let daemon = include_str!("../../src/server/runtime_server_daemon.rs");
    let ipc = include_str!("../../../agent-semantic-client-db/src/workspace_db_ipc/protocol.rs");

    assert!(hook.contains("evaluate_hook_event_locally"));
    assert!(bootstrap.contains("codex_tool_event_requires_policy_evaluation"));
    assert!(bootstrap.contains("bootstrap-no-agent-bypass"));
    for source in [hook, bootstrap, runtime, daemon, ipc] {
        assert!(!source.contains("EvaluateHook"));
        assert!(!source.contains("HookEvaluationBuilder"));
        assert!(!source.contains("runtime_server_hook_evaluation_client"));
        assert!(!source.contains("ResidentHookSnapshotAuthority"));
    }
}

#[test]
fn process_recovery_has_one_outer_owner_and_no_platform_compatibility_variable() {
    let main = include_str!("../../src/main.rs");
    let bootstrap = include_str!("../../src/hook_bootstrap.rs");
    let runtime = include_str!("../../src/command/hook_runtime.rs");
    let config = agent_semantic_config::default_hook_client_config_template();

    assert!(bootstrap.contains("const NO_AGENT_ENV: &str = \"ASP_NO_AGENT\""));
    assert!(main.contains("process_entry_no_agent_bypass"));
    assert!(main.contains("new_no_agent_client"));
    assert!(main.contains("else if no_agent_client"));
    assert!(
        main.find("ProcessOutcome::Command(agent_semantic_client::run_binary_from_env().await)")
            .unwrap()
            < main.find("tokio::select!").unwrap(),
        "ASP_NO_AGENT ordinary CLI must avoid Host signal registration"
    );
    assert!(
        main.find("process_entry_no_agent_bypass").unwrap()
            < main.find("RuntimeServerRuntimeBuilder").unwrap()
    );
    assert!(!runtime.contains("ASP_NO_AGENT"));
    assert!(!config.contains("ASP_NO_AGENT"));
    for source in [main, bootstrap, runtime, config.as_str()] {
        assert!(!source.contains("ASP_NO_AGENT_PLATFORM"));
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
        include_str!("../../../agent-semantic-hook/src/aot_evaluator_cli.rs"),
    ] {
        for forbidden in [
            "RuntimeServerClientExecutor",
            "runtime_server_workspace_session",
            "ensure_runtime_generation",
            "submit_runtime_generation_mutation",
            "reconcile_pending_runtime_activation_for_client_bootstrap",
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

    let snapshot = include_str!("../../../agent-semantic-hook/src/aot_evaluator_cli.rs");
    assert!(snapshot.contains("compiled-hook-generation.json"));
    assert!(snapshot.contains("load_generation"));
    assert!(snapshot.contains("evaluate_payload_at_generation"));
    assert!(!snapshot.contains("matcher.bin"));
    assert!(!snapshot.contains("MmapOptions"));
    assert!(!snapshot.contains("managed_hook_config::materialize"));

    let event_state = include_str!("../../../agent-semantic-hook/src/event_state.rs");
    assert!(event_state.contains("try_lock_exclusive"));
    assert!(event_state.contains("HOOK_EVENT_STATE_LOCK_TIMEOUT"));
    assert!(event_state.contains("Duration::from_millis(100)"));
    assert!(!event_state.contains("OnceLock<Mutex"));
}

#[test]
fn codex_pre_tool_transport_preserves_one_native_host_match_per_entry() {
    let plugin: serde_json::Value = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../asp-codex-plugin/hooks/hooks.json"
    )))
    .expect("parse fixed Codex plugin Hook transport");
    let entries = plugin["hooks"]["PreToolUse"]
        .as_array()
        .expect("typed PreToolUse entries");
    let matchers = entries
        .iter()
        .map(|entry| entry["matcher"].as_str().expect("typed PreToolUse matcher"))
        .collect::<Vec<_>>();
    assert_eq!(
        matchers,
        ["^apply_patch$", "Bash", "spawn_agent", "^mcp__.*$"]
    );
    assert!(entries.iter().all(|entry| entry["matcher"] != "*"));
    for entry in entries {
        let command = entry["hooks"][0]["command"]
            .as_str()
            .expect("typed Hook command");
        assert!(
            command.contains("--host-match ") || command.contains("--host-match-prefix "),
            "{command}"
        );
        assert!(!command.contains("--host-action "), "{command}");
        assert!(!command.contains("--host-matcher"), "{command}");
    }
    assert_eq!(plugin["hooks"]["PermissionRequest"][0]["matcher"], "*");

    let bootstrap = include_str!("../../src/hook_bootstrap.rs");
    assert!(bootstrap.contains("bootstrap-local-action-passthrough"));
    assert!(bootstrap.contains("local-policy-evaluator"));
    assert!(!bootstrap.contains("evaluate_hook_event_via_runtime"));
}

#[tokio::test]
async fn explicit_no_agent_environment_bypasses_host_hook_before_payload_evaluation() {
    let binary = env!("CARGO_BIN_EXE_asp");
    let warm = std::process::Command::new(binary)
        .args(["hook", "pre-tool", "--client", "codex"])
        .env("ASP_NO_AGENT", "1")
        .env("ASP_HOOK_BOOTSTRAP_TRACE", "1")
        .output()
        .expect("warm process-entry recovery binary");
    assert!(
        warm.status.success(),
        "binary={binary} stderr={}",
        String::from_utf8_lossy(&warm.stderr)
    );
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
        let mut spec =
            agent_semantic_hook_testkit::HookProcessSpec::new(binary, env!("CARGO_MANIFEST_DIR"));
        spec.args = vec![
            "hook".to_owned(),
            event.to_owned(),
            "--client".to_owned(),
            "codex".to_owned(),
        ];
        spec.env = vec![("ASP_HOOK_BOOTSTRAP_TRACE".to_owned(), "1".to_owned())];
        spec.timeout = std::time::Duration::from_secs(2);
        let receipt = agent_semantic_hook_testkit::run_process_entry_no_agent_recovery_probe(&spec)
            .await
            .unwrap_or_else(|error| panic!("event={event}: {error}"));

        assert_eq!(receipt.decision, serde_json::json!({}), "event={event}");
        assert!(
            receipt
                .stderr
                .contains("route=process-entry-no-agent-bypass"),
            "event={event} stderr={}",
            receipt.stderr
        );
        assert!(
            !receipt.stderr.contains("local-policy-evaluator"),
            "event={event} stderr={}",
            receipt.stderr
        );
        assert!(
            !receipt.stderr.contains("runtime-server"),
            "event={event} stderr={}",
            receipt.stderr
        );
        assert!(receipt.elapsed < spec.timeout, "event={event}");
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
fn unrelated_host_actions_are_excluded_by_the_physical_plugin_matcher_set() {
    let plugin: serde_json::Value = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../asp-codex-plugin/hooks/hooks.json"
    )))
    .expect("parse fixed Codex plugin Hook transport");
    let matchers = plugin["hooks"]["PreToolUse"]
        .as_array()
        .expect("typed PreToolUse entries")
        .iter()
        .map(|entry| entry["matcher"].as_str().expect("matcher"))
        .collect::<Vec<_>>();
    assert!(!matchers.contains(&"*"));
    assert!(!matchers.contains(&"update_plan"));
}

#[test]
fn missing_or_mismatched_host_match_signal_is_a_json_fail_closed_terminal() {
    let _cold_process_guard = crate::install_binary_test_guard::acquire();
    let read_payload = serde_json::json!({
        "hook_event_name": "PreToolUse",
        "tool_name": "Read",
        "tool_input": {"file_path": "README.md"}
    });
    let cases = [
        (
            Vec::<&str>::new(),
            "plugin Host matcher requires exactly one --host-match or --host-match-prefix",
        ),
        (
            vec!["--host-match", "Bash"],
            "plugin Host matcher binding mismatch",
        ),
    ];
    for (binding_args, expected_message) in cases {
        let output = run_codex_pre_tool_binding_probe(&binding_args, &read_payload);
        let stdout = String::from_utf8(output.stdout).expect("binding stdout UTF-8");
        let stderr = String::from_utf8(output.stderr).expect("binding stderr UTF-8");
        assert!(output.status.success(), "stdout={stdout} stderr={stderr}");
        let response: serde_json::Value = serde_json::from_str(stdout.trim())
            .unwrap_or_else(|error| panic!("one JSON terminal required: {error}; {stdout}"));
        assert_eq!(
            response["hookSpecificOutput"]["permissionDecision"], "deny",
            "{response}"
        );
        assert!(
            response["hookSpecificOutput"]["additionalContext"]
                .as_str()
                .is_some_and(|context| {
                    context.contains("host-action-authority-unavailable")
                        && context.contains(expected_message)
                }),
            "{response}"
        );
        assert!(!stderr.contains("panicked"), "stderr={stderr}");
    }
}

#[test]
fn post_tool_observation_emits_one_json_document_without_runtime_receipt() {
    use std::io::Write;
    use std::process::{Command, Stdio};

    let state = tempfile::tempdir().expect("temporary malformed Runtime observation");
    let owner = state.path().join("runtime/server/owner-spawn.v1.json");
    std::fs::create_dir_all(owner.parent().unwrap()).expect("owner receipt parent");
    std::fs::write(&owner, b"{\"schemaId\":").expect("malformed owner receipt");
    let mut child = Command::new(env!("CARGO_BIN_EXE_asp"))
        .args(["hook", "post-tool", "--client", "codex"])
        .env_clear()
        .env("HOME", state.path())
        .env("ASP_STATE_HOME", state.path())
        .env("ASP_HOOK_BOOTSTRAP_TRACE", "1")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn post-tool Hook binary");
    child
        .stdin
        .as_mut()
        .expect("post-tool stdin")
        .write_all(br#"{"tool_name":"update_plan","tool_input":{"plan":[]}}"#)
        .expect("write post-tool payload");
    drop(child.stdin.take());
    let output = child.wait_with_output().expect("wait for post-tool Hook");
    let stdout = String::from_utf8(output.stdout).expect("post-tool stdout UTF-8");
    let stderr = String::from_utf8(output.stderr).expect("post-tool stderr UTF-8");
    assert!(output.status.success(), "stdout={stdout} stderr={stderr}");
    let response: serde_json::Value = serde_json::from_str(stdout.trim())
        .unwrap_or_else(|error| panic!("post-tool must emit one JSON document: {error}; {stdout}"));
    assert_eq!(response, serde_json::json!({}));
    assert!(!stdout.contains("runtime-server-client-bootstrap-receipt"));
    assert!(!stderr.contains("runtime-server"), "stderr={stderr}");
}

#[test]
fn dedicated_hook_control_post_tool_is_valid_and_inside_host_deadline() {
    use std::io::Write;
    use std::process::{Command, Stdio};
    use std::time::{Duration, Instant};

    let _cold_process_guard = crate::install_binary_test_guard::acquire();
    let state = tempfile::tempdir().expect("isolated Hook control state");
    let started = Instant::now();
    let mut child = Command::new(env!("CARGO_BIN_EXE_asp-hook"))
        .args(["hook", "post-tool", "--client", "codex"])
        .env_clear()
        .env("HOME", state.path())
        .env("ASP_STATE_HOME", state.path())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn dedicated Hook control");
    child
        .stdin
        .take()
        .expect("Hook control stdin")
        .write_all(br#"{"tool_name":"update_plan","tool_input":{"plan":[]}}"#)
        .expect("write PostTool payload and release writer");
    let output = child.wait_with_output().expect("wait for Hook control");
    let elapsed = started.elapsed();
    assert!(
        output.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&output.stdout)
            .expect("valid Hook control JSON"),
        serde_json::json!({})
    );
    assert!(
        elapsed < Duration::from_secs(1),
        "dedicated Hook control elapsed {elapsed:?} exceeded Host deadline"
    );
}

#[cfg(target_os = "macos")]
#[tokio::test]
async fn structured_rust_read_binary_path_is_local_bounded_and_runtime_free() {
    use std::time::{SystemTime, UNIX_EPOCH};

    let _cold_process_guard = crate::install_binary_test_guard::acquire();
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

    let publication = publish_fixture_generation(workspace, &root, &state_home);
    assert!(publication.starts_with("blake3-256:"), "{publication}");

    let subject = "crates/agent-semantic-hook/src/protocol.rs";
    let payload = serde_json::json!({
        "session_id": "local-structured-read-regression",
        "cwd": workspace,
        "hook_event_name": "PreToolUse",
        "tool_name": "Bash",
        "tool_input": {
            "command": reader_probe_command(&root, subject)
        }
    });
    let receipt = run_fixture_hook(workspace, &root, &state_home, &payload, true).await;
    assert!(
        receipt.elapsed < std::time::Duration::from_secs(1),
        "the first real plugin PreTool call after validation and commit exceeded the Host deadline: elapsed={:?}",
        receipt.elapsed
    );
    let response = receipt.decision;
    let stderr = receipt.stderr;
    assert_eq!(
        response["hookSpecificOutput"]["permissionDecision"], "deny",
        "response={response} stderr={stderr}"
    );
    let context = response["hookSpecificOutput"]["additionalContext"]
        .as_str()
        .expect("typed Hook decision context");
    let typed = typed_hook_decision_context(&response);
    assert!(
        context.contains("registered-source-route-required"),
        "{context}"
    );
    assert!(context.contains("\"accessMode\":\"O_RDONLY\""), "{context}");
    assert!(context.contains("\"cleanupVerified\":true"), "{context}");
    assert!(context.contains("\"access\":\"read\""), "{context}");
    assert!(
        context.contains("\"evidence\":\"reader-behavior-dynamic-cache\""),
        "{context}"
    );
    assert!(
        context.contains("\"operationIntent\":\"source-read\""),
        "{context}"
    );
    assert!(context.contains("blake3-256:"), "{context}");
    assert!(
        context.contains(&format!(
            "asp rust search owner crates/agent-semantic-hook/src/protocol.rs items --workspace {} --view seeds",
            workspace.display()
        )),
        "source deny must carry the exact parser-owned recovery command: {context}"
    );
    assert!(!context.contains("asp session @"), "{context}");
    assert!(!stderr.contains("runtime-server"), "stderr={stderr}");
    let execution_micros = typed["elapsedMicros"]
        .as_u64()
        .expect("typed AOT Hook execution latency");
    assert!(
        execution_micros < 100_000,
        "structured Read Hook execution exceeded 100ms: executionMicros={execution_micros} stderr={stderr}"
    );

    let warm = run_fixture_hook(workspace, &root, &state_home, &payload, true).await;
    assert!(
        warm.elapsed < std::time::Duration::from_secs(1),
        "warm plugin PreTool call exceeded the Host deadline: elapsed={:?}",
        warm.elapsed
    );
    let warm_response = warm.decision;
    let warm_stderr = warm.stderr;
    let warm_context = warm_response["hookSpecificOutput"]["additionalContext"]
        .as_str()
        .expect("warm typed Hook decision context");
    let warm_typed = typed_hook_decision_context(&warm_response);
    assert!(
        warm_context.contains("blake3-256:"),
        "warm Hook invocation must load the immutable generation: {warm_context}"
    );
    let warm_execution_micros = warm_typed["elapsedMicros"]
        .as_u64()
        .expect("warm typed AOT Hook execution latency");
    assert!(
        warm_execution_micros < 100_000,
        "warm structured Read Hook execution exceeded 100ms: executionMicros={warm_execution_micros} stderr={warm_stderr}"
    );

    let current =
        agent_semantic_artifacts::hook_generation::read_current_hook_generation(&state_home)
            .expect("read immutable HookGeneration")
            .expect("published HookGeneration");
    let immutable: serde_json::Value = serde_json::from_slice(
        &std::fs::read(&current.generation_path).expect("read AOT generation"),
    )
    .expect("decode AOT generation");
    assert_eq!(
        immutable["schemaId"],
        "agent.semantic-protocols.hook-generation"
    );
    assert_eq!(immutable["schemaVersion"], 1);
    assert!(
        !state_home.join("hooks/compiled/matcher.bin").exists(),
        "legacy mmap matcher publication must not coexist with HookGeneration"
    );
    std::fs::remove_dir_all(root).expect("cleanup isolated Hook state");
}

#[test]
fn ready_runtime_endpoint_is_published_before_optional_telemetry_starts() {
    let daemon = include_str!("../../src/server/runtime_server_daemon.rs");
    let endpoint = daemon
        .find("publish_endpoint_after_required_planes")
        .expect("ready Runtime Server endpoint publication");
    let telemetry = daemon
        .find("RuntimeServerOpenTelemetry::start")
        .expect("optional Runtime Server OpenTelemetry startup");

    assert!(
        endpoint < telemetry,
        "optional telemetry must not gate Runtime Server control-plane publication"
    );
}

#[cfg(target_os = "macos")]
#[tokio::test]
async fn config_source_edit_does_not_republish_or_change_active_hook_generation() {
    use sha2::{Digest, Sha256};
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

    let publication = publish_fixture_generation(workspace, &root, &state_home);
    assert!(publication.starts_with("blake3-256:"), "{publication}");
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

    let subject = "crates/agent-semantic-hook/src/protocol.rs";
    let payload = serde_json::json!({
        "session_id": "managed-config-auto-sync",
        "cwd": workspace,
        "hook_event_name": "PreToolUse",
        "tool_name": "Bash",
        "tool_input": {
            "command": reader_probe_command(&root, subject)
        }
    });
    let receipt = run_fixture_hook(workspace, &root, &state_home, &payload, false).await;
    let response = receipt.decision;
    assert!(
        !response
            .to_string()
            .contains("hook-local-policy-unavailable"),
        "published matcher must not emit a recovery failure: {response}"
    );
    let context = response["hookSpecificOutput"]["additionalContext"]
        .as_str()
        .expect("typed Hook decision context");
    assert!(context.contains(&publication), "{context}");
    assert!(
        context.contains("Confirmed rust source reads are denied"),
        "{context}"
    );
    assert!(
        context.contains(&format!(
            "asp rust search owner crates/agent-semantic-hook/src/protocol.rs items --workspace {} --view seeds",
            workspace.display()
        )),
        "{context}"
    );
    assert!(context.contains("\"accessMode\":\"O_RDONLY\""), "{context}");
    assert!(context.contains("\"cleanupVerified\":true"), "{context}");
    assert!(context.contains("\"access\":\"read\""), "{context}");
    assert!(
        context.contains("\"evidence\":\"reader-behavior-dynamic-cache\""),
        "{context}"
    );

    let unchanged_source = std::fs::read_to_string(&config_path).expect("read Hook config source");
    assert_eq!(unchanged_source, stale);
    let current =
        agent_semantic_artifacts::hook_generation::read_current_hook_generation(&state_home)
            .expect("read current HookGeneration")
            .expect("current HookGeneration");
    assert_eq!(current.generation_digest, publication);
    std::fs::remove_dir_all(root).expect("cleanup isolated Hook state");
}
