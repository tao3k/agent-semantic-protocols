// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

use agent_semantic_hook::ReaderProbeAccess;
use agent_semantic_hook::ReaderProbeObservation;
use agent_semantic_hook::aot_evaluator::evaluate_pre_tool;
use agent_semantic_hook::bind_reader_probe_observation;
use agent_semantic_hook_testkit::CodexPreToolContext;
use agent_semantic_hook_testkit::project_codex_exec_command_calls;

const CODEX_PRE_TOOL_INPUT_SCHEMA: &str =
    include_str!("../fixtures/codex-pre-tool-use-command-input.v1.schema.json");
const SYNTHETIC_ROLLOUT_RESPONSE: &str =
    include_str!("../fixtures/codex-rollout-exec-command.v1.json");

fn canonical_generation() -> String {
    let config = agent_semantic_config::default_hook_client_config_file()
        .expect("load canonical Hook Config V1");
    String::from_utf8(
        agent_semantic_hook::aot_compiler::compile_aot_hook_policy_bundle(
            &config,
            "blake3-256:testkit-codex-rollout",
        )
        .expect("compile canonical HookPolicyBundle"),
    )
    .expect("canonical generation UTF-8")
}

#[test]
fn synthetic_codex_exec_command_rollout_projects_to_schema_valid_pre_tool_input() {
    let rollout: serde_json::Value =
        serde_json::from_str(SYNTHETIC_ROLLOUT_RESPONSE).expect("synthetic rollout JSON");
    let calls = project_codex_exec_command_calls(
        &rollout,
        &CodexPreToolContext {
            cwd: "/synthetic/workspace",
            model: "gpt-test",
            permission_mode: "default",
            session_id: "session-synthetic-v1",
            transcript_path: None,
            turn_id: "turn-synthetic-v1",
        },
    )
    .expect("project canonical Codex exec_command item");
    assert_eq!(calls.len(), 1);
    let payload = &calls[0];

    let schema: serde_json::Value =
        serde_json::from_str(CODEX_PRE_TOOL_INPUT_SCHEMA).expect("Codex PreToolUse schema");
    let validator = jsonschema::validator_for(&schema).expect("compile Codex input schema");
    let errors = validator
        .iter_errors(payload)
        .map(|error| error.to_string())
        .collect::<Vec<_>>();
    assert!(errors.is_empty(), "errors={errors:#?} payload={payload:#}");
    assert_eq!(payload["tool_name"], "Bash");
    assert_eq!(
        payload["tool_input"]["command"],
        "rtk read --max-lines 1 docs/hook-contract.md"
    );
    assert_eq!(payload["tool_use_id"], "call-synthetic-rtk-read-v1");
}

#[test]
fn projected_dynamic_reader_receipt_promotes_read_before_action_rule_selection() {
    let rollout: serde_json::Value =
        serde_json::from_str(SYNTHETIC_ROLLOUT_RESPONSE).expect("synthetic rollout JSON");
    let mut payload = project_codex_exec_command_calls(
        &rollout,
        &CodexPreToolContext {
            cwd: "/synthetic/workspace",
            model: "gpt-test",
            permission_mode: "default",
            session_id: "session-synthetic-v1",
            transcript_path: None,
            turn_id: "turn-synthetic-v1",
        },
    )
    .expect("project synthetic Codex call")
    .pop()
    .expect("one synthetic call");
    bind_reader_probe_observation(
        &mut payload,
        Some(&ReaderProbeObservation {
            subject: "docs/hook-contract.md".to_owned(),
            access: ReaderProbeAccess::Read,
            backend: "permission-differential".to_owned(),
            terminal: "read-permission-observed".to_owned(),
            elapsed_micros: 100,
            probe_process_launched: true,
            cleanup_verified: true,
            cache_hit: false,
            behavior_key: Some("synthetic-fixture-v1".to_owned()),
        }),
    )
    .expect("bind synthetic Reader receipt");

    let generation = canonical_generation();
    let payload_json = payload.to_string();
    let decision = evaluate_pre_tool(&generation, &payload_json, "Bash")
        .expect("evaluate projected PreTool input")
        .expect("validated dynamic reader must reach Markdown policy");
    assert_eq!(decision.decision, "deny");
    assert_eq!(
        decision.config_rule_id,
        "route-markdown-document-read-to-asp-explorer"
    );
    assert_eq!(decision.evidence, "reader-probe-read-permission");
}
