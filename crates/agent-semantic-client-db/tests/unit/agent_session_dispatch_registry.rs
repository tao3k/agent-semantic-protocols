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
fn dispatch_identity_is_stable_and_field_framed() {
    let argv = vec!["asp".to_string(), "rust".to_string()];
    let derive = |root_session_id, name, canonical_target, receipt_kind| {
        crate::agent_session_registry::derive_agent_session_dispatch_identity(
            crate::agent_session_registry::AgentSessionDispatchIdentityInput {
                root_session_id,
                name,
                canonical_target,
                receipt_kind,
                canonical_argv: &argv,
            },
        )
        .expect("derive dispatch identity")
    };
    let expected = derive(
        "root",
        "asp-explore",
        "/root/asp_explorer",
        "semantic-search-receipt.v1",
    );
    assert_eq!(
        expected,
        derive(
            "root",
            "asp-explore",
            "/root/asp_explorer",
            "semantic-search-receipt.v1",
        )
    );
    assert_ne!(
        expected.dispatch_identity,
        derive(
            "root-2",
            "asp-explore",
            "/root/asp_explorer",
            "semantic-search-receipt.v1",
        )
        .dispatch_identity
    );
    assert_ne!(
        expected.dispatch_identity,
        derive(
            "root",
            "asp-explore",
            "/root/asp_testing",
            "semantic-search-receipt.v1",
        )
        .dispatch_identity
    );
    assert_ne!(
        expected.dispatch_identity,
        derive(
            "root",
            "asp-explore",
            "/root/asp_explorer",
            "different-receipt.v1",
        )
        .dispatch_identity
    );
    assert_ne!(
        derive("ab", "c", "/root/asp_explorer", "receipt").dispatch_identity,
        derive("a", "bc", "/root/asp_explorer", "receipt").dispatch_identity
    );
}

#[test]
fn exactly_once_dispatch_receipt_recovers_after_verified_rebind() {
    let root = temp_root("dispatch-receipt-rebind");
    let state_root = root.join("state");
    let argv = vec!["/usr/bin/true".to_string()];
    let derived = crate::agent_session_registry::derive_agent_session_dispatch_identity(
        crate::agent_session_registry::AgentSessionDispatchIdentityInput {
            root_session_id: "root",
            name: "asp-explore",
            canonical_target: "/root/asp_explorer",
            receipt_kind: "semantic-search-receipt.v1",
            canonical_argv: &argv,
        },
    )
    .expect("derive dispatch identity");
    let registry =
        AgentSessionRegistry::open_or_create_state_root(&state_root).expect("create registry");
    registry
        .register_session(resident("child-1", "/root/asp_explorer", 10))
        .expect("register first generation");

    {
        let claim = |now| {
            registry.claim_dispatch(AgentSessionDispatchClaimRequest {
                project_id: "project",
                root_session_id: "root",
                name: "asp-explore",
                dispatch_identity: &derived.dispatch_identity,
                command_digest: &derived.command_digest,
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
                dispatch_identity: &derived.dispatch_identity,
                command_digest: &derived.command_digest,
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
                dispatch_identity: &derived.dispatch_identity,
                command_digest: &derived.command_digest,
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
    }

    drop(registry);
    let reopened =
        AgentSessionRegistry::open_or_create_state_root(&state_root).expect("reopen registry");
    let recovered = reopened
        .claim_dispatch(AgentSessionDispatchClaimRequest {
            project_id: "project",
            root_session_id: "root",
            name: "asp-explore",
            dispatch_identity: &derived.dispatch_identity,
            command_digest: &derived.command_digest,
            delivery_target_override: Some("resident-command-bridge:/root/asp_explorer"),
            now: 19,
        })
        .expect("recover terminal receipt after reopening registry");
    assert_eq!(recovered.action, "complete");
    assert_eq!(recovered.lease.status, "terminal");
    assert_eq!(recovered.lease.attempt_count, 2);
    assert_eq!(
        recovered.lease.evidence_ref.as_deref(),
        Some("search-receipt:1")
    );

    let _ = std::fs::remove_dir_all(root);
}
