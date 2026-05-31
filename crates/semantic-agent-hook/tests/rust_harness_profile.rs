use std::io::Write;
use std::process::{Command, Stdio};

use semantic_agent_hook::{
    classify_hook, parse_profiles, DecisionKind, ProfileRegistry, ReasonKind,
};
use serde_json::json;

fn generated_rust_profile_path() -> &'static str {
    env!("SEMANTIC_AGENT_HOOK_RUST_PROFILE_REGISTRY")
}

fn rust_harness_profile_registry() -> ProfileRegistry {
    let contents = std::fs::read_to_string(generated_rust_profile_path())
        .expect("generated rust profile registry");
    parse_profiles(&contents).expect("valid generated rust profile registry")
}

#[test]
fn build_script_uses_rust_harness_source_roots() {
    assert_eq!(env!("SEMANTIC_AGENT_HOOK_RUST_SOURCE_ROOTS"), "src");
}

#[test]
fn generated_rust_harness_profile_uses_provider_identity() {
    let registry = rust_harness_profile_registry();
    assert_eq!(registry.profiles.len(), 1);
    let profile = &registry.profiles[0];
    assert_eq!(profile.language_id, "rust");
    assert_eq!(profile.provider_id, "rs-harness");
    assert_eq!(profile.binary, "rs-harness");
    assert!(profile.source_roots.iter().any(|root| root == "src"));
    assert!(profile
        .source_extensions
        .iter()
        .any(|extension| extension == ".rs"));
}

#[test]
fn rust_harness_profile_routes_direct_reads_to_owner_search() {
    let decision = classify_hook(
        &rust_harness_profile_registry(),
        "codex",
        "pre-tool",
        &json!({
            "tool_name": "Read",
            "tool_input": {"path": "src/lib.rs"}
        }),
    );

    assert_eq!(decision.decision, DecisionKind::Deny);
    assert_eq!(decision.reason_kind, ReasonKind::DirectSourceRead);
    assert_eq!(
        decision.routes[0].argv,
        [
            "rs-harness",
            "search",
            "owner",
            "src/lib.rs",
            "items",
            "--view",
            "seeds",
            "."
        ]
    );
}

#[test]
fn rust_harness_profile_routes_raw_root_search_to_ingest() {
    let decision = classify_hook(
        &rust_harness_profile_registry(),
        "codex",
        "pre-tool",
        &json!({
            "tool_name": "functions.exec_command",
            "tool_input": {"cmd": "rg -n \"HookDecision\" ."}
        }),
    );

    assert_eq!(decision.decision, DecisionKind::Deny);
    assert_eq!(decision.reason_kind, ReasonKind::RawBroadSearch);
    assert_eq!(decision.routes[0].kind, "ingest");
    assert_eq!(
        decision.routes[0].stdin_mode.as_deref(),
        Some("pipe-candidates")
    );
}

#[test]
fn cli_doctor_accepts_generated_rust_profile_registry() {
    let output = Command::new(env!("CARGO_BIN_EXE_semantic-agent-hook"))
        .args(["doctor", "--profiles", generated_rust_profile_path()])
        .output()
        .expect("run semantic-agent-hook doctor");

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).expect("doctor stdout");
    assert!(stdout.contains("semantic-agent-hook profiles=1 projectRoot=."));
}

#[test]
fn cli_hook_emits_decision_for_generated_rust_profile_registry() {
    let mut child = Command::new(env!("CARGO_BIN_EXE_semantic-agent-hook"))
        .args([
            "hook",
            "--client",
            "codex",
            "pre-tool",
            "--profiles",
            generated_rust_profile_path(),
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("run semantic-agent-hook hook");
    child
        .stdin
        .as_mut()
        .expect("hook stdin")
        .write_all(br#"{"tool_name":"Read","tool_input":{"path":"src/lib.rs"}}"#)
        .expect("write hook payload");

    let output = child.wait_with_output().expect("wait for hook output");

    assert!(output.status.success());
    let value: serde_json::Value = serde_json::from_slice(&output.stdout).expect("hook JSON");
    assert_eq!(value["hookSpecificOutput"]["permissionDecision"], "deny");
    assert_eq!(value["agentHookDecision"]["decision"], "deny");
    assert_eq!(
        value["agentHookDecision"]["reasonKind"],
        "direct-source-read"
    );
    assert_eq!(
        value["agentHookDecision"]["routes"][0]["binary"],
        "rs-harness"
    );
    assert_eq!(
        value["agentHookDecision"]["routes"][0]["argv"][3],
        "src/lib.rs"
    );
}

#[test]
fn cli_hook_can_emit_raw_decision_for_schema_tests() {
    let mut child = Command::new(env!("CARGO_BIN_EXE_semantic-agent-hook"))
        .args([
            "hook",
            "--client",
            "codex",
            "pre-tool",
            "--profiles",
            generated_rust_profile_path(),
            "--emit",
            "decision",
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("run semantic-agent-hook hook");
    child
        .stdin
        .as_mut()
        .expect("hook stdin")
        .write_all(br#"{"tool_name":"Read","tool_input":{"path":"src/lib.rs"}}"#)
        .expect("write hook payload");

    let output = child.wait_with_output().expect("wait for hook output");

    assert!(output.status.success());
    let value: serde_json::Value = serde_json::from_slice(&output.stdout).expect("hook JSON");
    assert_eq!(value["decision"], "deny");
    assert_eq!(value["reasonKind"], "direct-source-read");
}
