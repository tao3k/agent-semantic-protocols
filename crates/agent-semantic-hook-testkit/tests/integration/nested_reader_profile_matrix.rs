//! Cross-layer wrapped Reader scenarios owned independently from the AOT core contract.

use agent_semantic_hook::ReaderProbeAccess;
use agent_semantic_hook::aot_evaluator::evaluate_pre_tool;
use agent_semantic_hook::aot_evaluator::reader_probe_request;
use agent_semantic_hook::bind_reader_probe_observation;
use agent_semantic_hook::diagnose_reader_probe_with_state_home;

use super::aot_evaluator_contract::canonical_generation;

fn assert_static_reader_denied(generation: &str, session_id: &str, command: &str) {
    let mut payload = serde_json::json!({
        "session_id": session_id,
        "cwd": ".",
        "hook_event_name": "PreToolUse",
        "tool_name": "Bash",
        "tool_input": { "command": command }
    });
    let request = reader_probe_request(generation, &payload.to_string(), "Bash")
        .unwrap_or_else(|error| panic!("project Reader request for {command:?}: {error}"))
        .unwrap_or_else(|| panic!("missing Reader request for {command:?}"));
    let state_home = tempfile::tempdir().expect("state home");
    let observation = diagnose_reader_probe_with_state_home(
        request.command_tokens,
        request.subject,
        request.wrapped_command,
        request.reader_behavior_patterns,
        state_home.path(),
    )
    .unwrap_or_else(|| panic!("missing static Reader observation for {command:?}"));
    assert_eq!(
        observation.access,
        ReaderProbeAccess::Read,
        "command={command:?} observation={observation:?}"
    );
    assert_eq!(observation.terminal, "reader-behavior-catalog-hit");
    assert!(!observation.probe_process_launched);
    bind_reader_probe_observation(&mut payload, Some(&observation))
        .expect("bind static Reader observation");
    let observed_payload = payload.to_string();
    let decision = evaluate_pre_tool(generation, &observed_payload, "Bash")
        .unwrap_or_else(|error| panic!("evaluate {command:?}: {error}"))
        .unwrap_or_else(|| panic!("registered source Reader must deny: {command:?}"));
    assert_eq!(decision.config_rule_id, "route-read-to-asp-languages");
    assert_eq!(decision.language, Some("rust"));
    assert_eq!(decision.evidence, "reader-behavior-static-catalog");
}

#[test]
fn exact_nested_run_pipeline_routes_the_entire_host_call() {
    let generation = canonical_generation();
    let first = "crates/agent-semantic-content-identity/src/content_binding.rs";
    let second = "crates/agent-semantic-client-db/src/runtime_server_workspace/content_binding.rs";
    let command = format!(
        "/workspace/.devenv/devenv-profile-exec rtk run 'rg -n . {first} | sed -n \"238,300p\"; rg -n . {second} | head -n 90'"
    );
    assert_static_reader_denied(&generation, "testkit-nested-run-readers", &command);
}

#[test]
fn wrapper_and_argv_variants_share_one_reader_profile_composition() {
    let generation = canonical_generation();
    let registered_source = "crates/agent-semantic-hook/src/lib.rs";
    let positive_commands = [
        format!("future-wrapper run 'rg -n needle {registered_source} | sed -n \"1,4p\"'"),
        format!(
            "project-launcher alpha run '/usr/bin/rg -n needle {registered_source}; head -n 4 {registered_source}'"
        ),
        format!(
            "env TRACE=1 custom-launcher run 'sed -n \"1,4p\" {registered_source} | head -n 2'"
        ),
        format!("rg 'alpha|beta' {registered_source}"),
    ];

    for (index, command) in positive_commands.iter().enumerate() {
        assert_static_reader_denied(
            &generation,
            &format!("testkit-nested-reader-matrix-{index}"),
            command,
        );
    }
}

#[test]
fn commands_without_a_registered_source_operand_remain_allowed() {
    let generation = canonical_generation();
    for command in [
        "future-wrapper run 'just --list | rg hook'",
        "future-wrapper run 'rg needle Cargo.lock | head -n 4'",
    ] {
        let payload = serde_json::json!({
            "session_id": "testkit-nested-reader-negative",
            "cwd": ".",
            "hook_event_name": "PreToolUse",
            "tool_name": "Bash",
            "tool_input": { "command": command }
        })
        .to_string();
        assert!(
            reader_probe_request(&generation, &payload, "Bash")
                .unwrap_or_else(|error| panic!("project negative {command:?}: {error}"))
                .is_none(),
            "non-source command must not request a Reader probe: {command:?}"
        );
        assert!(
            evaluate_pre_tool(&generation, &payload, "Bash")
                .unwrap_or_else(|error| panic!("evaluate negative {command:?}: {error}"))
                .is_none(),
            "non-source command must remain allowed: {command:?}"
        );
    }
}
