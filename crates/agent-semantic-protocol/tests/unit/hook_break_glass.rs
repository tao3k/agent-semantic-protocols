use super::{
    HookBreakGlassIssue, evaluate_hook_break_glass_inner_at, inline_break_glass_request,
    issue_hook_break_glass_capability_at, protected_command_digest, validate_nonce,
};
use serde_json::json;
use tempfile::tempdir;

#[test]
fn inline_capability_extracts_only_the_bound_protected_command() {
    let nonce = "a".repeat(64);
    let command = format!("ASP_BREAK_GLASS_CAPABILITY={nonce} cargo test -p owner");
    let (actual_nonce, protected) = inline_break_glass_request(&command)
        .expect("parse request")
        .expect("break-glass request");
    assert_eq!(actual_nonce, nonce);
    assert_eq!(protected, "cargo test -p owner");
    assert_eq!(
        protected_command_digest(&protected),
        protected_command_digest("cargo test -p owner")
    );
}

#[test]
fn capability_nonce_is_canonical_lowercase_hex() {
    assert!(validate_nonce(&"a".repeat(64)).is_ok());
    assert!(validate_nonce(&"A".repeat(64)).is_err());
    assert!(validate_nonce(&"a".repeat(63)).is_err());
}

#[test]
fn capability_is_consumed_once_and_replay_is_audited_failure() {
    let temp = tempdir().expect("temporary break-glass state");
    let workspace = temp.path().join("workspace");
    let state_root = temp.path().join("state");
    std::fs::create_dir(&workspace).expect("workspace");
    let protected_command = "cargo test -p owner";
    let capability = issue_hook_break_glass_capability_at(
        HookBreakGlassIssue {
            workspace_root: &workspace,
            root_session_id: "root-session",
            protected_command,
            deny_evidence_ref: "hook-deny:evidence",
            defect_kind: "exhausted-non-progress-cycle",
            ttl_seconds: 30,
        },
        &state_root,
        1_000,
    )
    .expect("mint capability");
    let input = serde_json::to_vec(&json!({
        "cwd": workspace,
        "session_id": "root-session",
        "tool_input": {
            "cmd": format!(
                "ASP_BREAK_GLASS_CAPABILITY={} {protected_command}",
                capability.nonce
            )
        }
    }))
    .expect("hook payload");

    let authorized = evaluate_hook_break_glass_inner_at(&input, &state_root, 2_000, None)
        .expect("first evaluation");
    assert_eq!(authorized, Some(capability.clone()));
    assert!(
        !state_root
            .join("pending")
            .join(format!("{}.json", capability.nonce))
            .exists()
    );
    assert!(
        state_root
            .join("consumed")
            .join(format!("{}.json", capability.nonce))
            .exists()
    );
    let audit = std::fs::read_to_string(state_root.join("audit.jsonl")).expect("audit");
    assert_eq!(audit.lines().count(), 1);
    assert!(audit.contains("hook-deny:evidence"));

    let replay = evaluate_hook_break_glass_inner_at(&input, &state_root, 2_001, None)
        .expect_err("replay must fail after atomic consumption");
    assert!(replay.contains("read pending break-glass capability"));
    let audit = std::fs::read_to_string(state_root.join("audit.jsonl")).expect("audit");
    assert_eq!(audit.lines().count(), 1);
}

#[test]
fn provider_direct_deny_is_ledger_backed_and_break_glass_is_one_shot() {
    let temp = tempfile::tempdir().expect("temporary hook state");
    let workspace = std::env::current_dir().expect("managed repository workspace");
    let state_root = temp.path().join("state");
    let protected_command = "/Users/guangtao/.agent-semantic-protocols/runtime/bin/asp-rust search owner crates/example.rs items";
    let payload = serde_json::json!({
        "cwd": workspace,
        "session_id": "root-session",
        "tool_name": "Bash",
        "tool_input": { "cmd": protected_command },
    });
    let mut decision =
        agent_semantic_hook::runtime_binary_policy_decision_v1("codex", "pre-tool", &payload)
            .expect("provider direct execution is denied");
    crate::command::publish_hook_decision_before_emit(&workspace, &mut decision);
    let deny_evidence_ref = decision
        .fields
        .get("denyEvidenceRef")
        .and_then(serde_json::Value::as_str)
        .unwrap_or_else(|| panic!("ledger-backed deny evidence: {:?}", decision.fields))
        .to_owned();
    assert_eq!(
        decision
            .fields
            .get("hookEventProjectionStatus")
            .and_then(serde_json::Value::as_str),
        Some("project-ledger")
    );
    let ledger = std::fs::read_to_string(&deny_evidence_ref).expect("event ledger");
    assert!(ledger.contains("provider-binary-direct-execution"));
    assert!(ledger.contains(protected_command));
    assert!(ledger.contains("denyEvidenceRef"));

    let capability = issue_hook_break_glass_capability_at(
        HookBreakGlassIssue {
            workspace_root: &workspace,
            root_session_id: "root-session",
            protected_command,
            deny_evidence_ref: &deny_evidence_ref,
            defect_kind: "provider-bootstrap-deadlock",
            ttl_seconds: 30,
        },
        &state_root,
        1_000,
    )
    .expect("mint capability");
    let input = serde_json::to_vec(&serde_json::json!({
        "cwd": workspace,
        "session_id": "root-session",
        "tool_input": {
            "cmd": format!(
                "ASP_BREAK_GLASS_CAPABILITY={} {protected_command}",
                capability.nonce
            )
        }
    }))
    .expect("hook payload");
    assert_eq!(
        evaluate_hook_break_glass_inner_at(&input, &state_root, 2_000, None)
            .expect("first evaluation"),
        Some(capability.clone())
    );
    let replay = evaluate_hook_break_glass_inner_at(&input, &state_root, 2_001, None)
        .expect_err("replay must fail after atomic consumption");
    assert!(replay.contains("read pending break-glass capability"));
}
