// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

use std::time::Duration;

use agent_semantic_hook::ReaderProbeAccess;
use agent_semantic_hook::ReaderProbeObservation;
use agent_semantic_hook::aot_evaluator::evaluate_pre_tool;
use agent_semantic_hook::aot_evaluator::reader_probe_request;
use agent_semantic_hook::bind_reader_probe_observation;
use agent_semantic_hook::diagnose_reader_probe;
use agent_semantic_hook::diagnose_reader_probe_with_state_home;
use agent_semantic_hook::evaluate_payload_with_policy_bundle_and_state_home_with_receipt;
use agent_semantic_hook::materialize_reader_probe_fixture;

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
            "asp search playbook --language rust 'owner symbol' --workspace .",
            Some("registered-asp-reasoning-search"),
        ),
        (
            "asp search playbook --language rust 'owner symbol' --workspace . --json",
            Some("registered-asp-reasoning-search"),
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
        (
            "just --list | rg hook",
            Some("deny-shell-search-before-execution"),
        ),
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

/// The exact `rtk read <unknown markdown path>` shape must enter the dynamic
/// detector, and its receipt must remain inspectable even if the policy allows
/// the command after an inconclusive probe.  This deliberately has no static
/// `rtk` reader catalog entry.
#[test]
fn exact_rtk_read_shape_persists_a_dynamic_probe_receipt() {
    #[cfg(target_os = "macos")]
    {
        let workspace = tempfile::tempdir().expect("isolated workspace");
        let state_home = tempfile::tempdir().expect("isolated Reader State Home");
        let subject = "/Users/guangtao/.codex/memories/skills/asp-hook-readonly-acceptance/xxxx.md";
        let payload = serde_json::json!({
            "session_id": "testkit-dynamic-rtk-read",
            "tool_use_id": "testkit-dynamic-rtk-read-tool-use",
            "cwd": workspace.path(),
            "hook_event_name": "PreToolUse",
            "tool_name": "Bash",
            "tool_input": { "command": format!("rtk read {subject}") },
        })
        .to_string();

        let receipt = evaluate_payload_with_policy_bundle_and_state_home_with_receipt(
            &canonical_generation(),
            &payload,
            "Bash",
            Some(state_home.path()),
        )
        .expect("evaluate exact dynamic rtk read shape");
        let host_output = receipt.host_output.expect("Codex pre-tool output");
        assert!(
            host_output == serde_json::json!({})
                || host_output["hookSpecificOutput"]["permissionDecision"] == "deny",
            "Host keeps its existing empty-allow or deny-only wire shape: {host_output:#}"
        );
        if host_output["hookSpecificOutput"]["permissionDecision"] == "deny" {
            let message = host_output["hookSpecificOutput"]["permissionDecisionReason"]
                .as_str()
                .expect("Markdown deny message");
            assert!(
                message.contains("Direct Markdown reads are denied because"),
                "message must state why the action is denied: {message}"
            );
            assert!(
                message.contains("registered Markdown Search Playbook route"),
                "message must route through the registered Search Playbook: {message}"
            );
            assert!(
                !message.contains("asp search playbook") && !message.contains("<path>"),
                "message must not expose an unresolved recovery placeholder: {message}"
            );
        }

        let state_path = receipt
            .reader_probe_event_path
            .expect("dynamic Reader probe receipt path");
        let events = std::fs::read_to_string(&state_path).expect("dynamic probe event state");
        let event = events
            .lines()
            .map(|line| serde_json::from_str::<serde_json::Value>(line).expect("event JSON"))
            .rev()
            .find(|event| event["fields"]["recordKind"] == "reader-probe-observation")
            .expect("dynamic Reader probe receipt");
        assert_eq!(event["schemaId"], "agent.semantic-protocols.hook.event");
        assert_eq!(event["schemaVersion"], "1");
        assert_eq!(event["subject"]["path"], subject);
        assert_eq!(event["fields"]["hostMatcher"], "Bash");
        assert_eq!(
            event["fields"]["readerProbe"]["schemaId"],
            "agent.semantic-protocols.reader-probe-observation"
        );
        assert!(
            matches!(
                event["fields"]["readerProbe"]["access"].as_str(),
                Some("read" | "unknown")
            ),
            "a dynamic probe must publish an explicit Read or Unknown observation: {event:#}"
        );
        if event["fields"]["readerProbe"]["access"] == "read" {
            assert_eq!(
                event["fields"]["policyDecision"]["generationDigest"],
                "blake3-256:testkit-canonical"
            );
            assert_eq!(
                event["fields"]["policyDecision"]["configRuleId"],
                "route-markdown-document-read-to-asp-explorer"
            );
        } else {
            assert_eq!(
                event["fields"]["policyDecision"]["state"],
                "no-matching-rule"
            );
        }
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
        decide("Bash", "Bash", serde_json::json!({"command":"asp search playbook --language rust owner"})),
        decide("Bash", "Bash", serde_json::json!({"command":"asp rust query --selector rust://owner"})),
        decide("Bash", "Bash", serde_json::json!({"command":"cargo test -p agent-semantic-hook"})),
        decide("Bash", "Bash", serde_json::json!({"command":"cargo fmt --all -- --check"})),
        decide("Bash", "Bash", serde_json::json!({"command":"cargo clippy -p agent-semantic-hook"})),
        decide("Bash", "Bash", serde_json::json!({"command":"git show HEAD:README.md"})),
        decide("Bash", "Bash", serde_json::json!({"command":"asp live-corpus qualify"})),
        decide("Bash", "Bash", serde_json::json!({"command":"gxc -O src/runtime.ss"})),
        decide("Bash", "Bash", serde_json::json!({"command":"asp rust search owner --json"})),
        decide("Bash", "Bash", serde_json::json!({"command":"rg Hook crates/agent-semantic-hook/src/lib.rs"})),
        decide("Bash", "Bash", confirmed_read("src/lib.rs")),
        decide("Bash", "Bash", confirmed_read("docs/acceptance.org")),
        decide("Bash", "Bash", confirmed_read("README.md")),
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

#[path = "aot_evaluator_contract_cases/reader_routes.rs"]
mod reader_routes;
