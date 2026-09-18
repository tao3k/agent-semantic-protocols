// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

use crate::agent_session_registry::AgentSessionDispatchClaimRequest;
use crate::agent_session_registry::AgentSessionDispatchCompleteRequest;
use crate::agent_session_registry::AgentSessionDispatchMarkOrphanedRequest;
use crate::agent_session_registry::AgentSessionRegisterRequest;
use crate::agent_session_registry::AgentSessionRegistry;
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
        project_id: "project".into(),
        root_session_id: "root".into(),
        session_id: session_id.into(),
        message_target_id: Some(message_target_id.into()),
        parent_session_id: Some("root".into()),
        name: "asp-explore".into(),
        role: "asp_explorer".into(),
        model_observation: None,
        status: "active".into(),
        expires_at: None,
        metadata_json: "{}".into(),
        now,
    }
}

#[test]
fn dispatch_identity_is_stable_and_field_framed() {
    let argv = vec!["asp".to_string(), "rust".to_string()];
    let derive = |root_session_id, child_session_id, name, canonical_target, receipt_kind| {
        crate::agent_session_registry::derive_agent_session_dispatch_identity(
            crate::agent_session_registry::AgentSessionDispatchIdentityInput {
                root_session_id,
                child_session_id,
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
        "child",
        "asp-explore",
        "/root/asp_explorer",
        "semantic-search-receipt.v1",
    );
    assert_eq!(
        expected,
        derive(
            "root",
            "child",
            "asp-explore",
            "/root/asp_explorer",
            "semantic-search-receipt.v1",
        )
    );
    assert_ne!(
        expected.dispatch_identity,
        derive(
            "root-2",
            "child",
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
            "child",
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
            "child",
            "asp-explore",
            "/root/asp_explorer",
            "different-receipt.v1",
        )
        .dispatch_identity
    );
    assert_ne!(
        derive("ab", "child", "c", "/root/asp_explorer", "receipt").dispatch_identity,
        derive("a", "child", "bc", "/root/asp_explorer", "receipt").dispatch_identity
    );
    assert_ne!(
        expected.dispatch_identity,
        derive(
            "root",
            "child-2",
            "asp-explore",
            "/root/asp_explorer",
            "semantic-search-receipt.v1",
        )
        .dispatch_identity
    );
}

#[tokio::test]
async fn exactly_once_dispatch_receipt_recovers_after_verified_rebind() {
    let root = temp_root("dispatch-receipt-rebind");
    let state_root = root.join("state");
    let argv = vec!["/usr/bin/true".to_string()];
    let derived = crate::agent_session_registry::derive_agent_session_dispatch_identity(
        crate::agent_session_registry::AgentSessionDispatchIdentityInput {
            root_session_id: "root",
            child_session_id: "child-1",
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
        .await
        .expect("register first generation");

    {
        let claim = |child_session_id, now| {
            let registry = &registry;
            let derived = &derived;
            async move {
                registry
                    .claim_dispatch(AgentSessionDispatchClaimRequest {
                        project_id: "project",
                        root_session_id: "root",
                        child_session_id,
                        name: "asp-explore",
                        dispatch_identity: &derived.dispatch_identity,
                        command_digest: &derived.command_digest,
                        delivery_target_override: Some(
                            "resident-command-bridge:/root/asp_explorer",
                        ),
                        now,
                    })
                    .await
            }
        };
        let first = claim("child-1", 11).await.expect("claim first delivery");
        assert_eq!(first.action, "send");
        assert_eq!(first.lease.attempt_count, 1);
        let duplicate_poll = claim("child-1", 12).await.expect("poll first delivery");
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
            .register_session(resident("child-2", "/root/asp_explorer", 14))
            .await
            .expect("register second child instance");
        let rebound = claim("child-2", 15).await.expect("claim rebound delivery");
        assert_eq!(rebound.action, "send");
        assert_eq!(rebound.lease.attempt_count, 2);
        let rebound_poll = claim("child-2", 16).await.expect("poll rebound delivery");
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
            .await
            .expect("complete rebound delivery");
        assert_eq!(complete.status, "terminal");
        assert_eq!(complete.attempt_count, 2);

        let terminal_poll = claim("child-2", 18).await.expect("poll terminal receipt");
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
            child_session_id: "child-2",
            name: "asp-explore",
            dispatch_identity: &derived.dispatch_identity,
            command_digest: &derived.command_digest,
            delivery_target_override: Some("resident-command-bridge:/root/asp_explorer"),
            now: 19,
        })
        .await
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
