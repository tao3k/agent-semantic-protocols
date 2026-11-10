// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

use super::parse_args;
use super::require_policy_decision;

fn args(values: &[&str]) -> Vec<String> {
    values.iter().map(|value| (*value).to_owned()).collect()
}

#[test]
fn preflight_and_host_acceptance_share_one_enablement_command() {
    let preflight = parse_args(&args(&[".", "--json"])).expect("parse preflight");
    assert!(preflight.json);
    assert!(preflight.host_evidence.is_none());

    let final_acceptance = parse_args(&args(&[
        ".",
        "--host-rollout",
        "task.jsonl",
        "--hook-events",
        "events.jsonl",
        "--host-probe-path",
        "probe.rs",
        "--host-sentinel",
        "TOKEN",
    ]))
    .expect("parse final acceptance");
    let host = final_acceptance
        .host_evidence
        .expect("complete Host evidence");
    assert_eq!(host.rollout, std::path::Path::new("task.jsonl"));
    assert_eq!(host.hook_events, std::path::Path::new("events.jsonl"));
    assert_eq!(host.probe_path, "probe.rs");
    assert_eq!(host.sentinel, "TOKEN");
}

#[test]
fn partial_host_evidence_fails_closed() {
    let error = parse_args(&args(&[
        "--host-rollout",
        "task.jsonl",
        "--hook-events",
        "events.jsonl",
    ]))
    .expect_err("partial Host evidence must fail");
    assert!(error.contains("reasonKind=host-delivery-evidence-incomplete"));
}

#[test]
fn policy_budget_sample_uses_the_borrowed_evaluator_without_host_state() {
    let generation = agent_semantic_hook::aot_compiler::compile_embedded_hook_policy_bundle()
        .expect("compile embedded Hook policy");
    let generation = std::str::from_utf8(&generation).expect("Hook policy JSON");
    let payload = serde_json::json!({
        "session_id": "hook-enablement-unit",
        "cwd": ".",
        "hook_event_name": "PreToolUse",
        "tool_name": "Bash",
        "tool_input": {"command": "cargo test -p agent-semantic-hook"}
    })
    .to_string();

    require_policy_decision(generation, &payload).expect("borrowed policy decision");
}
