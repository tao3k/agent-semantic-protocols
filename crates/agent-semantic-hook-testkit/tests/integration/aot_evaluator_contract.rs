use std::time::Duration;

use agent_semantic_hook::aot_evaluator::{evaluate_pre_tool, reader_probe_request};
use agent_semantic_hook::{
    ReaderProbeAccess, ReaderProbeObservation, bind_reader_probe_observation,
    diagnose_reader_probe, diagnose_reader_probe_with_state_home, materialize_reader_probe_fixture,
};

pub(super) const GENERATION: &str = r#"{"schemaId":"agent.semantic-protocols.hook-policy-bundle","schemaVersion":1,"generationDigest":"blake3-256:testkit-perf","rules":[{"id":"route-source","matchers":["Bash"],"wrappedCommand":true,"actions":["read"],"registeredExtensions":["rs"],"decision":"deny","reasonKind":"registered-source-route-required","message":"Use ASP."}]}"#;

const CODEX_PRE_TOOL_OUTPUT_SCHEMA: &str = include_str!(
    "../../../agent-semantic-config/schemas/codex-hooks/pre-tool-use-command-output.schema.json"
);
const CODEX_PERMISSION_REQUEST_OUTPUT_SCHEMA: &str = include_str!(
    "../../../agent-semantic-config/schemas/codex-hooks/permission-request-command-output.schema.json"
);
const HOOK_EXECUTION_FAILURE_SCHEMA: &str =
    include_str!("../../../../schemas/semantic-agent-hook-execution-failure.schema.json");

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
    assert_valid_against(CODEX_PRE_TOOL_OUTPUT_SCHEMA, &pre_tool);
    let mut invalid = pre_tool.clone();
    invalid["configRuleId"] = "route-read-to-asp-languages".into();
    assert_rejected_by(CODEX_PRE_TOOL_OUTPUT_SCHEMA, &invalid);
    assert!(
        pre_tool["hookSpecificOutput"]["additionalContext"]
            .as_str()
            .is_some_and(|context| context == "Use ASP.")
    );

    let permission = agent_semantic_hook::render_codex_permission_request(
        "deny",
        Some("Blocked by ASP policy."),
    );
    assert_valid_against(CODEX_PERMISSION_REQUEST_OUTPUT_SCHEMA, &permission);
    let mut invalid = permission.clone();
    invalid["reasonKind"] = "permission-request-denied".into();
    assert_rejected_by(CODEX_PERMISSION_REQUEST_OUTPUT_SCHEMA, &invalid);
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
}

pub(super) fn canonical_generation() -> String {
    let config = agent_semantic_config::default_hook_client_config_file()
        .expect("load canonical Hook Config V1");
    String::from_utf8(
        agent_semantic_hook::aot_compiler::compile_aot_hook_policy_bundle(
            &config,
            "blake3-256:testkit-canonical",
        )
        .expect("compile canonical AOT HookPolicyBundle"),
    )
    .expect("canonical AOT HookPolicyBundle is UTF-8")
}

#[test]
fn compiled_generation_requires_and_preserves_wrapped_command_policy() {
    let config = agent_semantic_config::default_hook_client_config_file()
        .expect("load canonical Hook Config V1");
    let source_rule = config
        .rules
        .iter()
        .find(|rule| rule.id == "route-read-to-asp-languages")
        .expect("canonical Read route");
    assert_eq!(
        source_rule.actions,
        [agent_semantic_config::HookClientActionKind::Read]
    );
    assert!(
        source_rule.matcher_policies.is_empty(),
        "Read action must enable wrapped-command matching without a duplicate policy declaration"
    );
    let generation = canonical_generation();
    let value: serde_json::Value =
        serde_json::from_str(&generation).expect("decode canonical generation");
    let wrapped_rules = value["rules"]
        .as_array()
        .expect("compiled rules")
        .iter()
        .filter(|rule| rule["id"] == "route-read-to-asp-languages")
        .collect::<Vec<_>>();
    assert!(!wrapped_rules.is_empty());
    assert!(
        wrapped_rules
            .iter()
            .all(|rule| rule["wrappedCommand"] == true)
    );

    let mut missing = value;
    for rule in missing["rules"]
        .as_array_mut()
        .expect("mutable compiled rules")
    {
        rule.as_object_mut()
            .expect("compiled rule object")
            .remove("wrappedCommand");
    }
    let payload = serde_json::json!({
        "tool_name": "Bash",
        "tool_input": {"command": "rtk read crates/example.rs"}
    })
    .to_string();
    let error = match reader_probe_request(&missing.to_string(), &payload, "Bash") {
        Ok(_) => panic!("missing wrappedCommand must fail closed"),
        Err(error) => error,
    };
    assert!(error.contains("wrappedCommand"), "error={error}");
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
        if observation.access == ReaderProbeAccess::Unknown {
            assert!(
                matches!(
                    observation.terminal.as_str(),
                    "probe-deferred" | "probe-timeout"
                ),
                "observation={observation:?}"
            );
            assert!(!observation.probe_process_launched || observation.terminal == "probe-timeout");
            return;
        }
        assert_eq!(
            observation.access,
            ReaderProbeAccess::Read,
            "observation={observation:?}"
        );
        assert_eq!(observation.terminal, "read-permission-observed");
        assert!(observation.probe_process_launched);
        // The cold path is deadline-bounded by the probe itself.  Its receipt
        // includes durable-cache publication, so do not conflate that final
        // bookkeeping with the command's permission observation.
        assert_ne!(observation.terminal, "probe-timeout");

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
                    let observation = diagnose_reader_probe_with_state_home(
                        tokens,
                        subject,
                        probe_wrapped_command,
                        patterns,
                        &state_home,
                    )
                    .expect("wrapped Reader cache hit");
                    (
                        Duration::from_micros(observation.elapsed_micros),
                        observation,
                    )
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
            "mcp__codex_app__send_message_to_thread",
            "mcp__codex_app__send_message_to_thread",
            serde_json::json!({"threadId":"thread-1", "prompt":"continue"}),
        ),
        decide(
            "mcp__codex_app__list_projects",
            "mcp__codex_app__list_projects",
            serde_json::json!({}),
        ),
        decide(
            "mcp__codex_app__automation_update",
            "mcp__codex_app__automation_update",
            serde_json::json!({"mode":"view", "id":"automation-1"}),
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
fn unknown_reader_observation_does_not_produce_a_read_action() {
    let mut payload = serde_json::json!({
        "session_id": "testkit-reader-unknown",
        "cwd": ".",
        "hook_event_name": "PreToolUse",
        "tool_name": "Bash",
        "tool_input": { "command": "future-source-consumer src/lib.rs" }
    });
    let observation = ReaderProbeObservation {
        subject: "src/lib.rs".to_owned(),
        access: ReaderProbeAccess::Unknown,
        backend: "permission-differential".to_owned(),
        terminal: "probe-unknown".to_owned(),
        elapsed_micros: 0,
        probe_process_launched: true,
        cleanup_verified: true,
        cache_hit: false,
        behavior_key: None,
    };
    bind_reader_probe_observation(&mut payload, Some(&observation))
        .expect("bind typed Unknown observation");
    let payload = payload.to_string();
    assert!(
        evaluate_pre_tool(GENERATION, &payload, "Bash")
            .expect("evaluate typed Unknown observation")
            .is_none(),
        "Unknown SourceAccess must not be promoted to Read"
    );

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
fn static_reader_catalog_routes_git_show_to_asp() {
    let generation = canonical_generation();
    let payload = serde_json::json!({
        "session_id": "testkit-static-git-show",
        "cwd": ".",
        "hook_event_name": "PreToolUse",
        "tool_name": "Bash",
        "tool_input": { "command": "git show HEAD:src/lib.rs" }
    });
    let payload_json = payload.to_string();
    let decision = evaluate_pre_tool(&generation, &payload_json, "Bash")
        .expect("evaluate git show")
        .expect("git show must deny");
    assert_eq!(decision.decision, "deny");
    assert_eq!(decision.config_rule_id, "git-history-inspection-dispatch");
}

#[test]
fn static_reader_catalog_routes_wrapped_absolute_git_show_to_asp() {
    let generation = canonical_generation();
    let payload = serde_json::json!({
        "session_id": "testkit-static-wrapped-git-show",
        "cwd": ".",
        "hook_event_name": "PreToolUse",
        "tool_name": "Bash",
        "tool_input": { "command": ".devenv/devenv-profile-exec /usr/bin/git show HEAD:src/lib.rs" }
    });
    let payload_json = payload.to_string();
    let decision = evaluate_pre_tool(&generation, &payload_json, "Bash")
        .expect("evaluate wrapped absolute git show")
        .expect("wrapped absolute git show must route");
    assert_eq!(decision.decision, "deny");
    assert_eq!(decision.config_rule_id, "git-history-inspection-dispatch");
    assert!(!decision.probe_process_launched);
}

#[test]
fn static_reader_catalog_routes_git_diff_to_asp() {
    let generation = canonical_generation();
    let payload = serde_json::json!({
        "session_id": "testkit-static-git-diff",
        "cwd": ".",
        "hook_event_name": "PreToolUse",
        "tool_name": "Bash",
        "tool_input": { "command": "git diff -- src/lib.rs" }
    });
    let payload_json = payload.to_string();
    let decision = evaluate_pre_tool(&generation, &payload_json, "Bash")
        .expect("evaluate git diff")
        .expect("git diff must deny");
    assert_eq!(decision.decision, "deny");
    assert_eq!(decision.config_rule_id, "git-history-inspection-dispatch");
}

#[test]
fn git_subcommands_without_a_declared_reader_argument_do_not_inherit_read() {
    let generation = canonical_generation();
    let payload = serde_json::json!({
        "session_id": "testkit-git-non-reader",
        "cwd": ".",
        "hook_event_name": "PreToolUse",
        "tool_name": "Bash",
        "tool_input": { "command": "git cat-file -e HEAD:src/lib.rs" }
    });
    assert!(
        evaluate_pre_tool(&generation, &payload.to_string(), "Bash")
            .expect("evaluate non-reader git command")
            .is_none(),
        "a git executable name alone must not fabricate Read"
    );
}

#[test]
fn static_reader_catalog_routes_wrapped_gerbil_sed_to_asp() {
    let generation = canonical_generation();
    let command = ".devenv/devenv-profile-exec sed -n '130,180p;250,275p;318,365p' languages/gerbil-scheme-language-project-harness/src/language/evidence.ss";
    let payload = serde_json::json!({
        "session_id": "testkit-static-gerbil-sed",
        "cwd": ".",
        "hook_event_name": "PreToolUse",
        "tool_name": "Bash",
        "tool_input": { "command": command }
    });
    let payload_json = payload.to_string();
    let request = reader_probe_request(&generation, &payload_json, "Bash")
        .expect("build Reader observation request")
        .expect("static wrapped sed must require a Reader observation");
    let state_home = tempfile::tempdir().expect("state home");
    let observation = diagnose_reader_probe_with_state_home(
        request.command_tokens,
        request.subject,
        request.wrapped_command,
        request.reader_behavior_patterns,
        state_home.path(),
    )
    .expect("observe static wrapped sed");
    assert_eq!(observation.access, ReaderProbeAccess::Read);
    assert_eq!(observation.terminal, "reader-behavior-catalog-hit");
    assert!(!observation.probe_process_launched);

    let mut observed_payload = payload;
    bind_reader_probe_observation(&mut observed_payload, Some(&observation))
        .expect("bind static Reader observation");
    let observed_payload_json = observed_payload.to_string();
    let decision = evaluate_pre_tool(&generation, &observed_payload_json, "Bash")
        .expect("evaluate observed wrapped sed")
        .expect("registered Gerbil source read must deny");
    assert_eq!(decision.decision, "deny");
    assert_eq!(decision.config_rule_id, "route-read-to-asp-languages");
    assert_eq!(decision.language, Some("gerbil-scheme"));
    assert_eq!(decision.access, "read");
    assert_eq!(decision.backend, "hook-policy-bundle-reader-catalog");
}

#[test]
fn batched_static_reader_stages_route_the_entire_host_call() {
    let generation = canonical_generation();
    let payload = serde_json::json!({
        "session_id": "testkit-batched-static-readers",
        "cwd": ".",
        "hook_event_name": "PreToolUse",
        "tool_name": "Bash",
        "tool_input": {
            "command": "sed -n '1,3p' crates/agent-semantic-hook/src/lib.rs && sed -n '1,3p' crates/agent-semantic-client/src/lib.rs"
        }
    });
    let request = reader_probe_request(&generation, &payload.to_string(), "Bash")
        .expect("build batched Reader observation request")
        .expect("one static Reader stage must govern the complete Host call");
    assert_eq!(
        request.command_tokens.first().map(String::as_str),
        Some("sed")
    );
    let state_home = tempfile::tempdir().expect("state home");
    let observation = diagnose_reader_probe_with_state_home(
        request.command_tokens,
        request.subject,
        request.wrapped_command,
        request.reader_behavior_patterns,
        state_home.path(),
    )
    .expect("observe batched static Reader");
    assert_eq!(observation.access, ReaderProbeAccess::Read);
    assert!(!observation.probe_process_launched);
    let mut observed = payload;
    bind_reader_probe_observation(&mut observed, Some(&observation))
        .expect("bind batched Reader observation");
    let observed_json = observed.to_string();
    let decision = evaluate_pre_tool(&generation, &observed_json, "Bash")
        .expect("evaluate batched static Reader")
        .expect("batched registered source read must deny");
    assert_eq!(decision.config_rule_id, "route-read-to-asp-languages");
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
    const STATIC_GENERATION: &str = r#"{"schemaId":"agent.semantic-protocols.hook-policy-bundle","schemaVersion":1,"generationDigest":"blake3-256:static-reader","readerBehaviorPatterns":[["BATT","-s"]],"rules":[{"id":"route-source","matchers":["Bash"],"wrappedCommand":true,"actions":["read"],"registeredExtensions":["rs"],"decision":"deny","reasonKind":"registered-source-route-required","message":"Use ASP."}]}"#;
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
    assert_eq!(observation.backend, "hook-policy-bundle-reader-catalog");
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
        if observation.access == ReaderProbeAccess::Unknown
            && matches!(
                observation.terminal.as_str(),
                "probe-deferred" | "probe-timeout"
            )
        {
            bind_reader_probe_observation(&mut payload, Some(&observation))
                .unwrap_or_else(|error| panic!("bind deferred {mode} observation: {error}"));
            assert!(
                evaluate_pre_tool(GENERATION, &payload.to_string(), "Bash")
                    .expect("evaluate deferred Reader observation")
                    .is_none(),
                "unconfirmed Reader behavior must not deny"
            );
            continue;
        }
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
        if cold.access == ReaderProbeAccess::Unknown {
            assert_eq!(cold.terminal, "probe-deferred", "cold={cold:?}");
            assert!(!cold.probe_process_launched);
            return;
        }
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
                    let observation = diagnose_reader_probe_with_state_home(
                        tokens,
                        "fixture.rs".to_owned(),
                        false,
                        Vec::new(),
                        &state_home,
                    )
                    .expect("cached Reader observation");
                    (
                        Duration::from_micros(observation.elapsed_micros),
                        observation,
                    )
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
