#[cfg(target_os = "macos")]
use std::sync::mpsc;
use std::time::{Duration, Instant};

use agent_semantic_hook::aot_evaluator::{evaluate_pre_tool, reader_probe_request};
use agent_semantic_hook::{
    ReaderProbeAccess, bind_reader_probe_observation, diagnose_reader_probe,
    diagnose_reader_probe_with_state_home, materialize_reader_probe_fixture,
};

const GENERATION: &str = r#"{"schemaId":"agent.semantic-protocols.hook-generation","schemaVersion":1,"generationDigest":"blake3-256:testkit-perf","rules":[{"id":"route-source","matchers":["Bash"],"actions":["read"],"registeredExtensions":["rs"],"decision":"deny","reasonKind":"registered-source-route-required","message":"Use ASP."}]}"#;

const CODEX_PRE_TOOL_OUTPUT_SCHEMA: &str = include_str!(
    "../../agent-semantic-config/schemas/codex-hooks/pre-tool-use-command-output.schema.json"
);
const CODEX_PERMISSION_REQUEST_OUTPUT_SCHEMA: &str = include_str!(
    "../../agent-semantic-config/schemas/codex-hooks/permission-request-command-output.schema.json"
);
const UPSTREAM_CODEX_PRE_TOOL_OUTPUT_SCHEMA: &str = include_str!(
    "../../../.data/codex/codex-rs/hooks/schema/generated/pre-tool-use.command.output.schema.json"
);
const UPSTREAM_CODEX_PERMISSION_REQUEST_OUTPUT_SCHEMA: &str = include_str!(
    "../../../.data/codex/codex-rs/hooks/schema/generated/permission-request.command.output.schema.json"
);
const HOOK_EXECUTION_FAILURE_SCHEMA: &str =
    include_str!("../../../schemas/semantic-agent-hook-execution-failure.schema.json");

fn assert_valid_against(schema: &str, value: &serde_json::Value) {
    let schema: serde_json::Value = serde_json::from_str(schema).expect("Codex output schema");
    let validator = jsonschema::validator_for(&schema).expect("compile Codex output schema");
    let errors = validator
        .iter_errors(value)
        .map(|error| error.to_string())
        .collect::<Vec<_>>();
    assert!(errors.is_empty(), "errors={errors:#?} value={value:#}");
}

fn assert_rejected_by(schema: &str, value: &serde_json::Value) {
    let schema: serde_json::Value = serde_json::from_str(schema).expect("Codex output schema");
    assert!(
        !jsonschema::validator_for(&schema)
            .expect("compile Codex output schema")
            .is_valid(value),
        "schema must reject unknown Host wire fields: {value:#}"
    );
}

#[test]
fn codex_host_output_schemas_are_the_authority_for_aot_envelopes() {
    for schema in [
        CODEX_PRE_TOOL_OUTPUT_SCHEMA,
        CODEX_PERMISSION_REQUEST_OUTPUT_SCHEMA,
    ] {
        let schema: serde_json::Value = serde_json::from_str(schema).expect("managed schema");
        assert_eq!(schema["x-agent-semantic-schema-version"], 1);
        assert_eq!(schema["additionalProperties"], false);
    }

    let typed = serde_json::json!({
        "schemaId": "agent.semantic-protocols.hook.decision",
        "schemaVersion": 1,
        "decision": "deny",
        "configRuleId": "route-read-to-asp-languages",
        "generationDigest": "blake3-256:test",
    });
    let pre_tool = agent_semantic_hook::render_codex_pre_tool_deny(&typed, "Use ASP.");
    for schema in [
        CODEX_PRE_TOOL_OUTPUT_SCHEMA,
        UPSTREAM_CODEX_PRE_TOOL_OUTPUT_SCHEMA,
    ] {
        assert_valid_against(schema, &pre_tool);
        let mut invalid = pre_tool.clone();
        invalid["configRuleId"] = "route-read-to-asp-languages".into();
        assert_rejected_by(schema, &invalid);
    }
    assert!(
        pre_tool["hookSpecificOutput"]["additionalContext"]
            .as_str()
            .is_some_and(|context| context.contains("route-read-to-asp-languages"))
    );

    let permission = agent_semantic_hook::render_codex_permission_request(
        "deny",
        Some("Blocked by ASP policy."),
    );
    for schema in [
        CODEX_PERMISSION_REQUEST_OUTPUT_SCHEMA,
        UPSTREAM_CODEX_PERMISSION_REQUEST_OUTPUT_SCHEMA,
    ] {
        assert_valid_against(schema, &permission);
        let mut invalid = permission.clone();
        invalid["reasonKind"] = "permission-request-denied".into();
        assert_rejected_by(schema, &invalid);
    }
}

#[test]
fn codex_launcher_error_receipt_and_host_envelope_are_both_schema_bound() {
    let receipt = serde_json::json!({
        "schemaId": "agent.semantic-protocols.hook.execution-failure",
        "schemaVersion": 1,
        "state": "failed",
        "phase": "bootstrap",
        "failureKind": "launcher-target-unavailable",
        "event": "pre-tool",
        "client": "codex",
        "runtimeArtifactFingerprint": "blake3-256:test-generation",
        "exitCode": 127,
        "message": "ASP Hook launcher target is unavailable."
    });
    assert_valid_against(HOOK_EXECUTION_FAILURE_SCHEMA, &receipt);

    let failure = agent_semantic_hook::HookExecutionFailure::new(
        agent_semantic_hook::HookExecutionPhase::Bootstrap,
        agent_semantic_hook::HookExecutionFailureKind::LauncherTargetUnavailable,
        Some("pre-tool".to_owned()),
        Some("codex".to_owned()),
        "ASP Hook launcher target is unavailable.",
    )
    .with_exit_code(127);
    let host_output = agent_semantic_hook::render_codex_execution_failure(&failure);
    assert_valid_against(CODEX_PRE_TOOL_OUTPUT_SCHEMA, &host_output);
    assert_valid_against(UPSTREAM_CODEX_PRE_TOOL_OUTPUT_SCHEMA, &host_output);
}

fn canonical_generation() -> String {
    let config = agent_semantic_config::default_hook_client_config_file()
        .expect("load canonical Hook Config V1");
    String::from_utf8(
        agent_semantic_hook::aot_compiler::compile_aot_hook_generation(
            &config,
            "blake3-256:testkit-canonical",
        )
        .expect("compile canonical AOT HookGeneration"),
    )
    .expect("canonical AOT HookGeneration is UTF-8")
}

fn evaluate_canonical_bash<'a>(generation: &'a str, command: &str) -> Option<String> {
    let payload = serde_json::json!({
        "session_id": "testkit-canonical-matrix",
        "cwd": ".",
        "hook_event_name": "PreToolUse",
        "tool_name": "Bash",
        "tool_input": { "command": command }
    })
    .to_string();
    evaluate_pre_tool(generation, &payload, "Bash")
        .unwrap_or_else(|error| panic!("evaluate canonical command {command:?}: {error}"))
        .map(|decision| decision.config_rule_id.to_owned())
}

#[test]
fn canonical_aot_generation_preserves_config_rule_composition_and_dominance() {
    let generation = canonical_generation();
    for (command, expected_rule) in [
        (
            "asp rust search pipe owner symbol --workspace . --view seeds",
            Some("registered-asp-reasoning-search"),
        ),
        (
            "asp rust search pipe owner symbol --workspace . --view seeds --json",
            Some("deny-agent-search-json"),
        ),
        (
            "cargo test -p agent-semantic-hook",
            Some("testing-role-dispatch"),
        ),
        (
            "cargo fmt --all -- --check",
            Some("rust-format-check-role-dispatch"),
        ),
        (
            "git show HEAD:README.md",
            Some("git-history-inspection-dispatch"),
        ),
        ("just --list | rg hook", None),
        ("cargo fmt --all", None),
    ] {
        assert_eq!(
            evaluate_canonical_bash(&generation, command).as_deref(),
            expected_rule,
            "command={command:?}"
        );
    }
}

#[test]
fn wrapped_command_profile_uses_dynamic_reader_observation_without_wrapper_vocabulary() {
    #[cfg(target_os = "macos")]
    {
        let generation = canonical_generation();
        let subject = "crates/agent-semantic-client/src/client_cli.rs";
        let command = format!(
            "{}/.devenv/devenv-profile-exec rtk read -n --max-lines 110 {subject}",
            env!("CARGO_MANIFEST_DIR").trim_end_matches("/crates/agent-semantic-hook-testkit")
        );
        let mut payload = serde_json::json!({
            "session_id": "testkit-wrapped-reader",
            "cwd": ".",
            "hook_event_name": "PreToolUse",
            "tool_name": "Bash",
            "tool_input": { "command": command }
        });
        let request = reader_probe_request(&generation, &payload.to_string(), "Bash")
            .expect("project wrapped Reader request")
            .expect("wrapped Reader request");
        assert!(request.wrapped_command);
        assert_eq!(request.subject, subject);
        assert_eq!(request.command_tokens[1..3], ["rtk", "read"]);

        let probe_tokens = request.command_tokens.clone();
        let probe_subject = request.subject.clone();
        let probe_wrapped_command = request.wrapped_command;
        let probe_patterns = request.reader_behavior_patterns.clone();

        let state_home = tempfile::tempdir().expect("isolated Reader State Home");
        let observation = diagnose_reader_probe_with_state_home(
            probe_tokens.clone(),
            probe_subject.clone(),
            probe_wrapped_command,
            probe_patterns.clone(),
            state_home.path(),
        )
        .expect("dynamic wrapped Reader observation");
        assert_eq!(
            observation.access,
            ReaderProbeAccess::Read,
            "observation={observation:?}"
        );
        assert_eq!(observation.terminal, "read-permission-observed");
        assert!(observation.probe_process_launched);
        assert!(observation.elapsed_micros < 100_000);

        const WORKERS: usize = 32;
        let barrier = std::sync::Arc::new(std::sync::Barrier::new(WORKERS));
        let workers = (0..WORKERS)
            .map(|_| {
                let barrier = barrier.clone();
                let tokens = probe_tokens.clone();
                let subject = probe_subject.clone();
                let patterns = probe_patterns.clone();
                let state_home = state_home.path().to_owned();
                std::thread::spawn(move || {
                    barrier.wait();
                    let started = Instant::now();
                    let observation = diagnose_reader_probe_with_state_home(
                        tokens,
                        subject,
                        probe_wrapped_command,
                        patterns,
                        &state_home,
                    )
                    .expect("wrapped Reader cache hit");
                    (started.elapsed(), observation)
                })
            })
            .collect::<Vec<_>>();
        let mut cache_hits = workers
            .into_iter()
            .map(|worker| worker.join().expect("wrapped Reader cache worker"))
            .collect::<Vec<_>>();
        cache_hits.sort_by_key(|(elapsed, _)| *elapsed);
        assert!(cache_hits.iter().all(|(_, hit)| {
            hit.access == ReaderProbeAccess::Read && hit.cache_hit && !hit.probe_process_launched
        }));
        let p99 = cache_hits[(cache_hits.len() * 99).div_ceil(100) - 1].0;
        assert!(p99 < Duration::from_millis(1), "wrapped cache p99={p99:?}");

        bind_reader_probe_observation(&mut payload, Some(&observation))
            .expect("bind wrapped Reader observation");
        let observed_payload = payload.to_string();
        let decision = evaluate_pre_tool(&generation, &observed_payload, "Bash")
            .expect("evaluate wrapped Reader observation")
            .expect("wrapped Reader deny");
        assert_eq!(decision.config_rule_id, "route-read-to-asp-languages");
        assert_eq!(decision.decision, "deny");
        assert_eq!(decision.profile, Some("rust"));
        assert_eq!(decision.route, Some("asp_explorer"));
        assert_eq!(decision.evidence, "reader-probe-read-permission");
    }
}

#[test]
fn canonical_aot_generation_preserves_the_universal_process_bound_escape() {
    let generation = canonical_generation();
    for command in [
        "ASP_NO_AGENT=1 arbitrary-command --unknown-option fixture.rs",
        "/usr/bin/env ASP_NO_AGENT=1 arbitrary-command --unknown-option fixture.rs",
        "export ASP_NO_AGENT=1; exec arbitrary-command --unknown-option fixture.rs",
    ] {
        let payload = serde_json::json!({
            "session_id": "testkit-no-agent",
            "cwd": ".",
            "hook_event_name": "PreToolUse",
            "tool_name": "Bash",
            "tool_input": { "command": command }
        })
        .to_string();
        let decision = evaluate_pre_tool(&generation, &payload, "Bash")
            .unwrap_or_else(|error| panic!("evaluate {command:?}: {error}"))
            .expect("terminal no-agent allow");
        assert_eq!(decision.config_rule_id, "allow-explicit-no-agent");
        assert_eq!(decision.decision, "allow");
    }
}

#[test]
fn canonical_aot_generation_preserves_structured_projection_allow_and_deny() {
    let generation = canonical_generation();
    let root = tempfile::tempdir().expect("structured projection workspace");
    std::fs::write(root.path().join("fixture.json"), br#"{"name":"asp"}"#)
        .expect("structured projection fixture");
    for (command, expected_rule, expected_decision) in [
        (
            "jq -c '.name' fixture.json",
            "allow-bounded-json-projection",
            "allow",
        ),
        (
            "jq '.' fixture.json",
            "deny-unbounded-structured-projection",
            "deny",
        ),
    ] {
        let payload = serde_json::json!({
            "session_id": "testkit-structured-projection",
            "cwd": root.path(),
            "hook_event_name": "PreToolUse",
            "tool_name": "Bash",
            "tool_input": { "command": command }
        })
        .to_string();
        let decision = evaluate_pre_tool(&generation, &payload, "Bash")
            .unwrap_or_else(|error| panic!("evaluate {command:?}: {error}"))
            .unwrap_or_else(|| panic!("missing structured decision for {command:?}"));
        assert_eq!(
            decision.config_rule_id, expected_rule,
            "command={command:?}"
        );
        assert_eq!(decision.decision, expected_decision, "command={command:?}");
    }
}

#[test]
fn canonical_aot_generation_preserves_native_edit_and_mcp_host_boundaries() {
    let generation = canonical_generation();
    let edit_payload = serde_json::json!({
        "session_id": "testkit-native-edit",
        "cwd": ".",
        "hook_event_name": "PreToolUse",
        "tool_name": "apply_patch",
        "tool_input": {"patch": "*** Begin Patch\n*** Update File: README.md\n*** End Patch"}
    })
    .to_string();
    let edit = evaluate_pre_tool(&generation, &edit_payload, "apply_patch")
        .expect("evaluate native Edit")
        .expect("owner-scoped native Edit decision");
    assert_eq!(edit.config_rule_id, "allow-owner-scoped-mutation");
    assert_eq!(edit.operation_intent, "owner-scoped-mutation");
    assert_eq!(edit.decision, "allow");

    let mcp_payload = serde_json::json!({
        "session_id": "testkit-native-mcp",
        "cwd": ".",
        "hook_event_name": "PreToolUse",
        "tool_name": "mcp__filesystem__metadata",
        "tool_input": {}
    })
    .to_string();
    assert!(
        evaluate_pre_tool(&generation, &mcp_payload, "mcp__")
            .expect("evaluate MCP prefix Host match")
            .is_none(),
        "unconfigured MCP action must remain an explicit allow/no-decision"
    );

    let spawn_payload = serde_json::json!({
        "session_id": "testkit-native-agent",
        "cwd": ".",
        "hook_event_name": "PreToolUse",
        "tool_name": "spawn_agent",
        "tool_input": {"agent_type": "asp_explorer"}
    })
    .to_string();
    assert!(
        evaluate_pre_tool(&generation, &spawn_payload, "spawn_agent")
            .expect("evaluate spawn_agent Host match")
            .is_none(),
        "spawn admission is owned by the lifecycle control plane, not an AOT command rule"
    );
}

#[test]
fn every_canonical_config_rule_has_an_aot_decision_witness() {
    let generation = canonical_generation();
    let root = tempfile::tempdir().expect("canonical rule witness workspace");
    std::fs::write(root.path().join("fixture.json"), br#"{"name":"asp"}"#).expect("JSON witness");
    std::fs::write(root.path().join("fixture.toml"), b"name = \"asp\"\n").expect("TOML witness");

    let decide = |tool_name: &str, host_matcher: &str, tool_input: serde_json::Value| {
        let payload = serde_json::json!({
            "session_id": "testkit-all-config-rules",
            "cwd": root.path(),
            "hook_event_name": "PreToolUse",
            "tool_name": tool_name,
            "tool_input": tool_input,
        })
        .to_string();
        evaluate_pre_tool(&generation, &payload, host_matcher)
            .unwrap_or_else(|error| panic!("evaluate {tool_name}/{host_matcher}: {error}"))
            .unwrap_or_else(|| panic!("missing decision for {tool_name}/{host_matcher}: {payload}"))
            .config_rule_id
            .to_owned()
    };
    let confirmed_read = |subject: &str| {
        serde_json::json!({
            "command": format!("opaque-reader {subject}"),
            "_aspReaderProbe": {
                "schemaId": "agent.semantic-protocols.reader-probe-observation",
                "schemaVersion": 1,
                "subject": subject,
                "access": "read",
                "accessMode": "read-permission",
                "backend": "state-home-reader-catalog",
                "terminal": "reader-behavior-cache-hit",
                "elapsedMicros": 9,
                "processLaunched": false,
                "probeProcessLaunched": false,
                "timeout": false,
                "policyFastPath": true,
                "cleanupVerified": true
            }
        })
    };
    let witnesses = [
        decide(
            "Bash",
            "Bash",
            serde_json::json!({"command":"ASP_NO_AGENT=1 arbitrary-command fixture.rs"}),
        ),
        decide(
            "apply_patch",
            "apply_patch",
            serde_json::json!({"patch":"*** Begin Patch\n*** Update File: README.md\n*** End Patch"}),
        ),
        decide("Bash", "Bash", serde_json::json!({"command":"asp rust search pipe owner"})),
        decide("Bash", "Bash", serde_json::json!({"command":"asp rust query --selector rust://owner"})),
        decide("Bash", "Bash", serde_json::json!({"command":"cargo test -p agent-semantic-hook"})),
        decide("Bash", "Bash", serde_json::json!({"command":"cargo fmt --all -- --check"})),
        decide("Bash", "Bash", serde_json::json!({"command":"cargo clippy -p agent-semantic-hook"})),
        decide("Bash", "Bash", serde_json::json!({"command":"git show HEAD:README.md"})),
        decide("Bash", "Bash", serde_json::json!({"command":"asp live-corpus qualify"})),
        decide("Bash", "Bash", serde_json::json!({"command":"gxc -O src/runtime.ss"})),
        decide("Bash", "Bash", serde_json::json!({"command":"asp rust search pipe owner --json"})),
        decide("Bash", "Bash", confirmed_read("src/lib.rs")),
        decide("Bash", "Bash", confirmed_read("fixture.json")),
        decide("Bash", "Bash", serde_json::json!({"command":"jq -c '.name' fixture.json"})),
        decide("Bash", "Bash", serde_json::json!({"command":"yq -p=toml '.name' fixture.toml"})),
        decide("Bash", "Bash", serde_json::json!({"command":"jq '.' fixture.json"})),
    ]
    .into_iter()
    .collect::<std::collections::BTreeSet<_>>();

    let config = agent_semantic_config::default_hook_client_config_file()
        .expect("load canonical Hook Config V1");
    let expected = serde_json::to_value(config).expect("project canonical config")["rules"]
        .as_array()
        .expect("canonical rules")
        .iter()
        .map(|rule| rule["id"].as_str().expect("canonical rule id").to_owned())
        .collect::<std::collections::BTreeSet<_>>();
    assert_eq!(
        witnesses, expected,
        "Config rule lacks a serving AOT witness"
    );
}

#[test]
fn borrowed_aot_evaluator_meets_submillisecond_p99_without_probe() {
    const SAMPLES: usize = 128;
    const TEST_DEADLINE: Duration = Duration::from_secs(1);
    const PAYLOAD: &str = r#"{"session_id":"testkit-perf","cwd":".","hook_event_name":"PreToolUse","tool_name":"Bash","tool_input":{"command":"< src/lib.rs"}}"#;

    let (sender, receiver) = mpsc::sync_channel(1);
    let worker = std::thread::spawn(move || {
        let mut samples = Vec::with_capacity(SAMPLES);
        for _ in 0..SAMPLES {
            let started = Instant::now();
            let decision = evaluate_pre_tool(GENERATION, PAYLOAD, "Bash")
                .expect("evaluate borrowed HookGeneration")
                .expect("registered source decision");
            samples.push(started.elapsed().as_micros());
            assert_eq!(decision.decision, "deny");
            assert!(!decision.probe_process_launched);
            assert_eq!(decision.elapsed_micros, 0);
            assert_eq!(decision.reader_observation_micros, 0);
        }
        samples.sort_unstable();
        sender.send(samples).expect("publish performance receipt");
    });
    let samples = receiver
        .recv_timeout(TEST_DEADLINE)
        .expect("borrowed evaluator exceeded the 1s test deadline");
    worker.join().expect("borrowed evaluator worker");

    let percentile = |percent: usize| samples[(samples.len() * percent).div_ceil(100) - 1];
    let p50 = percentile(50);
    let p95 = percentile(95);
    let p99 = percentile(99);
    let max = *samples.last().expect("wall samples");
    eprintln!(
        "Hook borrowed evaluator wall micros: n={SAMPLES} p50={p50} p95={p95} p99={p99} max={max}"
    );
    assert!(p99 < 1_000, "Hook borrowed evaluator p99={p99}us");
}

#[test]
fn canonical_config_aot_policy_kernel_meets_submillisecond_p99() {
    const SAMPLES: usize = 128;
    let generation = canonical_generation();
    let payload = serde_json::json!({
        "session_id": "testkit-canonical-perf",
        "cwd": ".",
        "hook_event_name": "PreToolUse",
        "tool_name": "Bash",
        "tool_input": {"command": "cargo test -p agent-semantic-hook"}
    })
    .to_string();
    let mut samples = Vec::with_capacity(SAMPLES);
    for _ in 0..SAMPLES {
        let started = Instant::now();
        let decision = evaluate_pre_tool(&generation, &payload, "Bash")
            .expect("evaluate canonical policy")
            .expect("canonical Testing route");
        samples.push(started.elapsed().as_micros());
        assert_eq!(decision.config_rule_id, "testing-role-dispatch");
    }
    samples.sort_unstable();
    let p99 = samples[(samples.len() * 99).div_ceil(100) - 1];
    let max = *samples.last().expect("canonical policy samples");
    eprintln!("Canonical Hook AOT kernel micros: n={SAMPLES} p99={p99} max={max}");
    assert!(p99 < 1_000, "canonical Hook AOT p99={p99}us");
}

#[test]
fn bare_registered_operands_do_not_authorize_a_read_decision() {
    let unknown_source_witnesses = [
        "cat src/lib.rs",
        "sed -n '1p' src/lib.rs",
        "grep needle src/lib.rs",
        "rg needle src/lib.rs",
        "git show HEAD:src/lib.rs",
        "git diff -- src/lib.rs",
    ];
    for command in unknown_source_witnesses {
        let payload = serde_json::json!({
            "session_id": "testkit-reader-matrix",
            "cwd": ".",
            "hook_event_name": "PreToolUse",
            "tool_name": "Bash",
            "tool_input": { "command": command }
        })
        .to_string();
        assert!(
            evaluate_pre_tool(GENERATION, &payload, "Bash")
                .unwrap_or_else(|error| panic!("evaluate {command:?}: {error}"))
                .is_none(),
            "bare operand {command:?} must not be promoted to Read"
        );
    }

    let explicit_read_payload = serde_json::json!({
        "session_id": "testkit-reader-redirection",
        "cwd": ".",
        "hook_event_name": "PreToolUse",
        "tool_name": "Bash",
        "tool_input": { "command": "< src/lib.rs" }
    })
    .to_string();
    let explicit_read = evaluate_pre_tool(GENERATION, &explicit_read_payload, "Bash")
        .expect("evaluate explicit read redirection")
        .expect("explicit registered source read must deny");
    assert_eq!(explicit_read.decision, "deny");
    assert_eq!(explicit_read.config_rule_id, "route-source");
    assert_eq!(explicit_read.subject.as_deref(), Some("src/lib.rs"));
    assert_eq!(explicit_read.access, "read");
    assert_eq!(explicit_read.evidence, "shell-redirection-read");
    assert!(!explicit_read.probe_process_launched);
    assert_eq!(explicit_read.elapsed_micros, 0);
    assert!(explicit_read.policy_fast_path);

    for command in ["just --list | rg hook", "rg hook", "git status --short"] {
        let payload = serde_json::json!({
            "session_id": "testkit-reader-allow",
            "cwd": ".",
            "hook_event_name": "PreToolUse",
            "tool_name": "Bash",
            "tool_input": { "command": command }
        })
        .to_string();
        assert!(
            evaluate_pre_tool(GENERATION, &payload, "Bash")
                .unwrap_or_else(|error| panic!("evaluate allow witness {command:?}: {error}"))
                .is_none(),
            "metadata witness {command:?} unexpectedly matched a source rule"
        );
    }
}

#[test]
fn unknown_command_names_allow_without_confirmed_read_evidence() {
    for command in ["BATT -s src/lib.rs", "BATT-random-7f3 -s src/lib.rs"] {
        let payload = serde_json::json!({
            "session_id": "testkit-unknown-command",
            "cwd": ".",
            "hook_event_name": "PreToolUse",
            "tool_name": "Bash",
            "tool_input": { "command": command }
        })
        .to_string();
        assert!(
            evaluate_pre_tool(GENERATION, &payload, "Bash")
                .unwrap_or_else(|error| panic!("evaluate {command:?}: {error}"))
                .is_none(),
            "unknown command name {command:?} must remain Allow(None)"
        );
    }
}

#[test]
fn declared_reader_behavior_pattern_routes_without_process_launch() {
    const STATIC_GENERATION: &str = r#"{"schemaId":"agent.semantic-protocols.hook-generation","schemaVersion":1,"generationDigest":"blake3-256:static-reader","readerBehaviorPatterns":[["BATT","-s"]],"rules":[{"id":"route-source","matchers":["Bash"],"actions":["read"],"registeredExtensions":["rs"],"decision":"deny","reasonKind":"registered-source-route-required","message":"Use ASP."}]}"#;
    let mut payload = serde_json::json!({
        "session_id": "testkit-static-reader",
        "cwd": ".",
        "hook_event_name": "PreToolUse",
        "tool_name": "Bash",
        "tool_input": {"command": "BATT -s src/lib.rs"}
    });
    let request = reader_probe_request(STATIC_GENERATION, &payload.to_string(), "Bash")
        .expect("project static Reader request")
        .expect("static Reader request");
    let observation = diagnose_reader_probe(
        request.command_tokens,
        request.subject,
        request.wrapped_command,
        request.reader_behavior_patterns,
    )
    .expect("static Reader observation");
    assert_eq!(observation.access, ReaderProbeAccess::Read);
    assert!(!observation.probe_process_launched);
    assert_eq!(observation.backend, "hook-generation-reader-catalog");
    bind_reader_probe_observation(&mut payload, Some(&observation))
        .expect("bind static Reader observation");
    let payload_json = payload.to_string();
    let decision = evaluate_pre_tool(STATIC_GENERATION, &payload_json, "Bash")
        .expect("evaluate static Reader")
        .expect("static Reader deny");
    assert_eq!(decision.evidence, "reader-behavior-static-catalog");
    assert_eq!(decision.decision, "deny");

    let wrapped = diagnose_reader_probe(
        vec![
            "future-wrapper".to_owned(),
            "BATT".to_owned(),
            "-s".to_owned(),
            "src/lib.rs".to_owned(),
        ],
        "src/lib.rs".to_owned(),
        true,
        vec![vec!["BATT".to_owned(), "-s".to_owned()]],
    )
    .expect("wrapped static Reader observation");
    assert_eq!(wrapped.access, ReaderProbeAccess::Read);
    assert_eq!(wrapped.terminal, "reader-behavior-catalog-hit");
    assert!(!wrapped.probe_process_launched);
}

#[test]
fn reader_probe_permission_differential_authorizes_only_read_behavior() {
    let fixture = materialize_reader_probe_fixture().expect("Reader behavior fixture");
    let random_root = tempfile::tempdir().expect("random Reader fixture root");
    let random_fixture = random_root.path().join("BATT-random-7f3");
    std::fs::hard_link(&fixture, &random_fixture).expect("link random Reader fixture");
    std::fs::set_permissions(
        &random_fixture,
        std::fs::metadata(&fixture)
            .expect("Reader fixture metadata")
            .permissions(),
    )
    .expect("random Reader fixture mode");
    let state_home = tempfile::tempdir().expect("isolated Reader State Home");
    for (mode, expected_access, denied) in [
        ("read", ReaderProbeAccess::Read, true),
        ("write", ReaderProbeAccess::Unknown, false),
        ("read-write", ReaderProbeAccess::Unknown, false),
    ] {
        let mut payload = serde_json::json!({
            "session_id": format!("testkit-reader-{mode}"),
            "cwd": ".",
            "hook_event_name": "PreToolUse",
            "tool_name": "Bash",
            "tool_input": {
                "command": format!("{} {mode} src/lib.rs", random_fixture.display())
            }
        });
        let payload_json = payload.to_string();
        let request = reader_probe_request(GENERATION, &payload_json, "Bash")
            .unwrap_or_else(|error| panic!("project {mode} Reader request: {error}"))
            .unwrap_or_else(|| panic!("Reader request was ambiguous for {mode}"));
        let observation = diagnose_reader_probe_with_state_home(
            request.command_tokens,
            request.subject,
            request.wrapped_command,
            request.reader_behavior_patterns,
            state_home.path(),
        )
        .unwrap_or_else(|| panic!("Reader probe produced no {mode} observation"));
        assert_eq!(
            observation.access, expected_access,
            "{mode}: backend={} terminal={} elapsedMicros={}",
            observation.backend, observation.terminal, observation.elapsed_micros
        );
        assert!(observation.cleanup_verified, "{mode}");
        bind_reader_probe_observation(&mut payload, Some(&observation))
            .unwrap_or_else(|error| panic!("bind {mode} observation: {error}"));
        let payload = payload.to_string();
        let decision = evaluate_pre_tool(GENERATION, &payload, "Bash")
            .unwrap_or_else(|error| panic!("evaluate {mode} observation: {error}"));
        assert_eq!(decision.is_some(), denied, "{mode}");
        if let Some(decision) = decision {
            assert_eq!(decision.access, "read");
            assert_eq!(decision.access_mode, "read-permission");
            assert_eq!(decision.evidence, "reader-probe-read-permission");
            assert!(decision.cleanup_verified);
        }
        if mode == "read" {
            let request = reader_probe_request(GENERATION, &payload_json, "Bash")
                .expect("project cached Reader request")
                .expect("cached Reader request");
            let cached = diagnose_reader_probe_with_state_home(
                request.command_tokens,
                request.subject,
                request.wrapped_command,
                request.reader_behavior_patterns,
                state_home.path(),
            )
            .expect("cached Reader observation");
            assert_eq!(cached.access, ReaderProbeAccess::Read);
            assert!(cached.cache_hit);
            assert!(!cached.probe_process_launched);
            assert_eq!(cached.backend, "process-memory-reader-catalog");
        }
    }
}

#[test]
fn concurrent_dynamic_cache_hits_are_submillisecond_and_process_free() {
    #[cfg(target_os = "macos")]
    {
        const WORKERS: usize = 32;
        let fixture = materialize_reader_probe_fixture().expect("Reader behavior fixture");
        let state_home = tempfile::tempdir().expect("isolated Reader State Home");
        let tokens = vec![
            fixture.to_string_lossy().into_owned(),
            "read".to_owned(),
            "fixture.rs".to_owned(),
        ];
        let cold = diagnose_reader_probe_with_state_home(
            tokens.clone(),
            "fixture.rs".to_owned(),
            false,
            Vec::new(),
            state_home.path(),
        )
        .expect("cold Reader observation");
        assert_eq!(cold.access, ReaderProbeAccess::Read);
        assert!(cold.probe_process_launched);

        let barrier = std::sync::Arc::new(std::sync::Barrier::new(WORKERS));
        let workers = (0..WORKERS)
            .map(|_| {
                let barrier = barrier.clone();
                let tokens = tokens.clone();
                let state_home = state_home.path().to_owned();
                std::thread::spawn(move || {
                    barrier.wait();
                    let started = Instant::now();
                    let observation = diagnose_reader_probe_with_state_home(
                        tokens,
                        "fixture.rs".to_owned(),
                        false,
                        Vec::new(),
                        &state_home,
                    )
                    .expect("cached Reader observation");
                    (started.elapsed(), observation)
                })
            })
            .collect::<Vec<_>>();
        let mut receipts = workers
            .into_iter()
            .map(|worker| worker.join().expect("Reader cache worker"))
            .collect::<Vec<_>>();
        assert!(receipts.iter().all(|(_, observation)| {
            observation.access == ReaderProbeAccess::Read
                && observation.cache_hit
                && !observation.probe_process_launched
        }));
        receipts.sort_by_key(|(elapsed, _)| *elapsed);
        let p99 = receipts[WORKERS * 99 / 100].0;
        eprintln!("Reader dynamic cache hit concurrency: n={WORKERS} p99={p99:?}");
        assert!(p99 < Duration::from_millis(1), "cache-hit p99={p99:?}");
    }
}
