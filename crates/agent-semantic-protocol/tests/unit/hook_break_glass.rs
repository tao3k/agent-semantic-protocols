use super::{
    HookBreakGlassIssue, evaluate_hook_break_glass_inner_at, inline_break_glass_request,
    issue_hook_break_glass_capability_at, protected_command_digest, validate_nonce,
};
use serde_json::json;
use tempfile::tempdir;

#[test]
fn naked_no_agent_assignment_is_not_authority() {
    let error = inline_break_glass_request("ASP_NO_AGENT=1 cargo test")
        .expect_err("naked bypass must be rejected");
    assert!(error.contains("ASP_BREAK_GLASS_CAPABILITY"));
}

#[test]
fn inline_capability_extracts_only_the_bound_protected_command() {
    let nonce = "a".repeat(64);
    let command = format!("ASP_NO_AGENT=1 ASP_BREAK_GLASS_CAPABILITY={nonce} cargo test -p owner");
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
                "ASP_NO_AGENT=1 ASP_BREAK_GLASS_CAPABILITY={} {protected_command}",
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
