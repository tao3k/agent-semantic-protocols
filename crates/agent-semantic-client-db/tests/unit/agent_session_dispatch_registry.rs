use crate::agent_session_registry::{
    AgentSessionDispatchClaimRequest, AgentSessionDispatchCompleteRequest,
    AgentSessionDispatchMarkOrphanedRequest, AgentSessionRegisterRequest, AgentSessionRegistry,
};
use std::path::PathBuf;
use std::time::SystemTime;
use std::time::UNIX_EPOCH;

fn temp_root(label: &str) -> PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    std::env::temp_dir().join(format!("asp-{label}-{}-{nonce}", std::process::id()))
}

fn resident<'a>(
    session_id: &'a str,
    message_target_id: &'a str,
    now: i64,
) -> AgentSessionRegisterRequest<'a> {
    AgentSessionRegisterRequest {
        project_id: "project",
        root_session_id: "root",
        session_id,
        message_target_id: Some(message_target_id),
        parent_session_id: Some("root"),
        name: "asp-explore",
        role: "asp_explorer",
        model_observation: None,
        status: "active",
        expires_at: None,
        metadata_json: "{}",
        now,
    }
}

#[test]
fn exactly_once_dispatch_receipt_recovers_after_verified_rebind() {
    let root = temp_root("dispatch-receipt-rebind");
    let registry = AgentSessionRegistry::open_or_create_state_root(root.join("state"))
        .expect("create registry");
    registry
        .register_session(resident("child-1", "/root/asp_explorer", 10))
        .expect("register first generation");

    let claim = |now| {
        registry.claim_dispatch(AgentSessionDispatchClaimRequest {
            project_id: "project",
            root_session_id: "root",
            name: "asp-explore",
            dispatch_identity: "dispatch-1",
            command_digest: "digest-1",
            delivery_target_override: Some("resident-command-bridge:/root/asp_explorer"),
            now,
        })
    };
    let first = claim(11).expect("claim first delivery");
    assert_eq!(first.action, "send");
    assert_eq!(first.lease.attempt_count, 1);
    let duplicate_poll = claim(12).expect("poll first delivery");
    assert_eq!(duplicate_poll.action, "wait");
    assert_eq!(duplicate_poll.lease.attempt_count, 1);

    let orphaned = registry
        .mark_dispatch_orphaned(AgentSessionDispatchMarkOrphanedRequest {
            project_id: "project",
            root_session_id: "root",
            name: "asp-explore",
            dispatch_identity: "dispatch-1",
            command_digest: "digest-1",
            now: 13,
        })
        .expect("mark delivery orphaned");
    assert_eq!(orphaned.status, "orphaned-awaiting-rebind");

    registry
        .replace_resident_session("child-1", resident("child-2", "/root/asp_explorer", 14))
        .expect("replace resident generation");
    let rebound = claim(15).expect("claim rebound delivery");
    assert_eq!(rebound.action, "send");
    assert_eq!(rebound.lease.attempt_count, 2);
    let rebound_poll = claim(16).expect("poll rebound delivery");
    assert_eq!(rebound_poll.action, "wait");
    assert_eq!(rebound_poll.lease.attempt_count, 2);

    let complete = registry
        .complete_dispatch(AgentSessionDispatchCompleteRequest {
            project_id: "project",
            root_session_id: "root",
            name: "asp-explore",
            dispatch_identity: "dispatch-1",
            command_digest: "digest-1",
            evidence_ref: "search-receipt:1",
            now: 17,
        })
        .expect("complete rebound delivery");
    assert_eq!(complete.status, "terminal");
    assert_eq!(complete.attempt_count, 2);

    let terminal_poll = claim(18).expect("poll terminal receipt");
    assert_eq!(terminal_poll.action, "complete");
    assert_eq!(terminal_poll.lease.attempt_count, 2);
    assert_eq!(
        terminal_poll.lease.evidence_ref.as_deref(),
        Some("search-receipt:1")
    );

    let _ = std::fs::remove_dir_all(root);
}
