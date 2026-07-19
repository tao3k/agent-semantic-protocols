use super::{AgentSessionDispatchLeaseRecord, dispatch_claim_action};

fn lease(status: &str, digest: &str) -> AgentSessionDispatchLeaseRecord {
    AgentSessionDispatchLeaseRecord {
        project_id: "project".into(),
        root_session_id: "root".into(),
        name: "asp-testing".into(),
        dispatch_identity: "identity".into(),
        command_digest: digest.into(),
        delivery_target_id: Some("resident-command-bridge:/root/asp_testing".into()),
        delivery_generation_id: Some("generation-1".into()),
        status: status.into(),
        attempt_count: 1,
        created_at: 1,
        updated_at: 1,
        completed_at: (status == "terminal").then_some(1),
        evidence_ref: (status == "terminal").then(|| "receipt".into()),
    }
}

#[test]
fn same_identity_and_digest_waits_without_resend() {
    assert_eq!(
        dispatch_claim_action(
            Some(&lease("in-flight", "digest")),
            "identity",
            "digest",
            "resident-command-bridge:/root/asp_testing",
            "generation-1",
        )
        .unwrap(),
        "wait"
    );
}

#[test]
fn orphaned_awaiting_rebind_replays_once_after_verified_generation() {
    assert_eq!(
        dispatch_claim_action(
            Some(&lease("orphaned-awaiting-rebind", "digest")),
            "identity",
            "digest",
            "resident-command-bridge:/root/asp_testing",
            "generation-2",
        )
        .unwrap(),
        "send"
    );
}

#[test]
fn orphaned_awaiting_rebind_waits_for_new_generation() {
    let error = dispatch_claim_action(
        Some(&lease("orphaned-awaiting-rebind", "digest")),
        "identity",
        "digest",
        "resident-command-bridge:/root/asp_testing",
        "generation-1",
    )
    .unwrap_err();
    assert!(error.contains("not deliverable"));
}

#[test]
fn terminal_identity_and_digest_completes_without_resend() {
    assert_eq!(
        dispatch_claim_action(
            Some(&lease("terminal", "digest")),
            "identity",
            "digest",
            "resident-command-bridge:/root/asp_testing",
            "generation-1",
        )
        .unwrap(),
        "complete"
    );
}

#[test]
fn same_identity_with_new_digest_rejects_stale_receipt_replay() {
    let error = dispatch_claim_action(
        Some(&lease("terminal", "old-digest")),
        "identity",
        "new-digest",
        "resident-command-bridge:/root/asp_testing",
        "generation-1",
    )
    .unwrap_err();
    assert!(error.contains("digest mismatch"));
}
