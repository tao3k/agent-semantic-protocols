// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

use agent_semantic_client_db::SessionControlPlaneAgentRegistration;
use agent_semantic_client_db::SessionControlPlaneDelegationProposal;
use agent_semantic_client_db::SessionControlPlaneRuntime;
use agent_semantic_client_db::SessionControlPlaneRuntimeRegistry;
use agent_semantic_context_product::agent_session_delegation_admission::AgentSessionDelegationCapability;
use agent_semantic_context_product::agent_session_delegation_admission::AgentSessionDelegationDecision;
use std::time::Duration;
use std::time::Instant;

async fn owner() -> (tempfile::TempDir, SessionControlPlaneRuntime) {
    let root = tempfile::tempdir().expect("session control-plane tempdir");
    let owner = SessionControlPlaneRuntime::start_in_client_dir(root.path())
        .await
        .expect("open session control plane");
    (root, owner)
}

async fn register(
    owner: &SessionControlPlaneRuntime,
    session_id: &str,
    parent_session_id: Option<&str>,
    resident_name: &str,
    capability: AgentSessionDelegationCapability,
) {
    owner
        .register_agent(&SessionControlPlaneAgentRegistration {
            project_id: "project-1".to_owned(),
            root_session_id: "root-1".to_owned(),
            session_id: session_id.to_owned(),
            parent_session_id: parent_session_id.map(str::to_owned),
            resident_name: resident_name.to_owned(),
            capability,
        })
        .await
        .expect("register control-plane agent");
}

fn proposal(
    event_id: &str,
    current_session_id: &str,
    child_session_id: &str,
    child_capability: AgentSessionDelegationCapability,
    expected_generation: u64,
) -> SessionControlPlaneDelegationProposal {
    SessionControlPlaneDelegationProposal {
        event_id: event_id.to_owned(),
        project_id: "project-1".to_owned(),
        root_session_id: "root-1".to_owned(),
        current_session_id: current_session_id.to_owned(),
        proposed_child_session_id: child_session_id.to_owned(),
        proposed_child_resident_name: child_session_id.to_owned(),
        proposed_child_capability: child_capability,
        expected_generation,
        evidence_refs: vec!["codex-v2:verified-rollout-parent".to_owned()],
        observed_at_ms: 1,
    }
}

#[tokio::test]
async fn focused_leaf_denial_commits_receipt_without_state_mutation_and_replays_once() {
    let (_root, owner) = owner().await;
    register(
        &owner,
        "root-1",
        None,
        "normal",
        AgentSessionDelegationCapability::Standard,
    )
    .await;
    register(
        &owner,
        "asp-testing-1",
        Some("root-1"),
        "asp_testing",
        AgentSessionDelegationCapability::FocusedLeaf,
    )
    .await;
    let before = owner
        .snapshot("project-1", "root-1")
        .await
        .expect("before snapshot");
    let request = proposal(
        "event-focused-1",
        "asp-testing-1",
        "nested-child-1",
        AgentSessionDelegationCapability::Standard,
        0,
    );

    let committed = owner
        .admit_delegation(&request)
        .await
        .expect("commit focused denial");
    assert!(!committed.replayed);
    assert_eq!(
        committed.admission.decision,
        AgentSessionDelegationDecision::Denied
    );
    let after = owner
        .snapshot("project-1", "root-1")
        .await
        .expect("after snapshot");
    assert_eq!(after.generation, before.generation);
    assert_eq!(after.state_digest, before.state_digest);
    assert_eq!(after.agent_count, before.agent_count);
    assert_eq!(after.delegation_count, before.delegation_count);
    assert_eq!(after.event_count, before.event_count + 1);

    let replayed = owner
        .admit_delegation(&request)
        .await
        .expect("replay focused denial");
    assert!(replayed.replayed);
    assert_eq!(replayed.admission, committed.admission);
    assert_eq!(
        owner
            .snapshot("project-1", "root-1")
            .await
            .expect("replayed snapshot")
            .event_count,
        after.event_count
    );
}

#[tokio::test]
async fn standard_delegation_atomically_publishes_child_edge_generation_and_receipt() {
    let (_root, owner) = owner().await;
    register(
        &owner,
        "root-1",
        None,
        "normal",
        AgentSessionDelegationCapability::Standard,
    )
    .await;
    let before = owner
        .snapshot("project-1", "root-1")
        .await
        .expect("before snapshot");

    let committed = owner
        .admit_delegation(&proposal(
            "event-standard-1",
            "root-1",
            "asp-explorer-1",
            AgentSessionDelegationCapability::FocusedLeaf,
            before.generation,
        ))
        .await
        .expect("commit standard delegation");
    assert_eq!(
        committed.admission.decision,
        AgentSessionDelegationDecision::Accepted
    );
    let after = owner
        .snapshot("project-1", "root-1")
        .await
        .expect("after snapshot");
    assert_eq!(after.generation, before.generation + 1);
    assert_ne!(after.state_digest, before.state_digest);
    assert_eq!(after.agent_count, before.agent_count + 1);
    assert_eq!(after.delegation_count, before.delegation_count + 1);
    assert_eq!(after.event_count, before.event_count + 1);

    let stale_before = after.clone();
    let error = owner
        .admit_delegation(&proposal(
            "event-stale-1",
            "root-1",
            "child-stale-1",
            AgentSessionDelegationCapability::Standard,
            before.generation,
        ))
        .await
        .expect_err("stale generation must fail closed");
    assert!(error.contains("generation is stale"));
    assert_eq!(
        owner
            .snapshot("project-1", "root-1")
            .await
            .expect("stale snapshot"),
        stale_before
    );
}

#[tokio::test]
async fn concurrent_same_event_is_single_flight_and_exactly_once() {
    let (_root, owner) = owner().await;
    register(
        &owner,
        "asp-testing-1",
        Some("root-1"),
        "asp_testing",
        AgentSessionDelegationCapability::FocusedLeaf,
    )
    .await;
    let request = proposal(
        "event-concurrent-1",
        "asp-testing-1",
        "nested-child-1",
        AgentSessionDelegationCapability::Standard,
        0,
    );
    let barrier = std::sync::Arc::new(tokio::sync::Barrier::new(256));
    let mut tasks = Vec::with_capacity(256);
    for _ in 0..256 {
        let owner = owner.clone();
        let request = request.clone();
        let barrier = barrier.clone();
        tasks.push(tokio::spawn(async move {
            barrier.wait().await;
            owner.admit_delegation(&request).await
        }));
    }
    let mut committed = 0;
    let mut replayed = 0;
    for task in tasks {
        let receipt = task
            .await
            .expect("join concurrent admission")
            .expect("concurrent admission");
        if receipt.replayed {
            replayed += 1;
        } else {
            committed += 1;
        }
    }
    assert_eq!(committed, 1);
    assert_eq!(replayed, 255);
    let metrics = owner.runtime_metrics().await;
    assert_eq!(metrics.durable_admission_transactions, 1);
    assert_eq!(metrics.resident_receipt_replays, 255);
    assert_eq!(metrics.resident_lane_count, 1);
    assert_eq!(
        owner
            .snapshot("project-1", "root-1")
            .await
            .expect("concurrent snapshot")
            .event_count,
        1
    );
}

#[tokio::test]
async fn warm_committed_receipt_replay_p99_is_sub_millisecond() {
    let (_root, owner) = owner().await;
    register(
        &owner,
        "asp-explorer-1",
        Some("root-1"),
        "asp_explorer",
        AgentSessionDelegationCapability::FocusedLeaf,
    )
    .await;
    let request = proposal(
        "event-warm-1",
        "asp-explorer-1",
        "nested-child-1",
        AgentSessionDelegationCapability::Standard,
        0,
    );
    owner
        .admit_delegation(&request)
        .await
        .expect("commit warm receipt");
    let mut samples = Vec::with_capacity(30);
    for _ in 0..30 {
        let started = Instant::now();
        let receipt = owner
            .admit_delegation(&request)
            .await
            .expect("warm receipt replay");
        assert!(receipt.replayed);
        samples.push(started.elapsed());
    }
    samples.sort_unstable();
    let p99 = samples[samples.len() - 1];
    assert!(
        p99 < Duration::from_millis(1),
        "warm delegation receipt replay p99 {p99:?} exceeds 1ms"
    );
}

#[tokio::test]
async fn committed_receipt_replays_after_owner_reconstruction() {
    let root = tempfile::tempdir().expect("session control-plane restart tempdir");
    let owner = SessionControlPlaneRuntime::start_in_client_dir(root.path())
        .await
        .expect("open initial session control plane");
    register(
        &owner,
        "asp-testing-1",
        Some("root-1"),
        "asp_testing",
        AgentSessionDelegationCapability::FocusedLeaf,
    )
    .await;
    let request = proposal(
        "event-restart-1",
        "asp-testing-1",
        "nested-child-1",
        AgentSessionDelegationCapability::Standard,
        0,
    );
    let committed = owner
        .admit_delegation(&request)
        .await
        .expect("commit before reconstruction");
    let snapshot = owner
        .snapshot("project-1", "root-1")
        .await
        .expect("snapshot before reconstruction");
    drop(owner);

    let reconstructed = SessionControlPlaneRuntime::start_in_client_dir(root.path())
        .await
        .expect("reconstruct session control plane");
    let replayed = reconstructed
        .admit_delegation(&request)
        .await
        .expect("replay durable receipt");
    assert!(replayed.replayed);
    assert_eq!(replayed.admission, committed.admission);
    assert_eq!(
        reconstructed
            .snapshot("project-1", "root-1")
            .await
            .expect("snapshot after reconstruction"),
        snapshot
    );
    let metrics = reconstructed.runtime_metrics().await;
    assert_eq!(metrics.durable_admission_transactions, 1);
    assert_eq!(metrics.resident_receipt_replays, 0);
}

#[tokio::test]
async fn independent_workspaces_do_not_share_writer_lanes() {
    let (_root_a, owner_a) = owner().await;
    let (_root_b, owner_b) = owner().await;
    register(
        &owner_a,
        "asp-testing-1",
        Some("root-1"),
        "asp_testing",
        AgentSessionDelegationCapability::FocusedLeaf,
    )
    .await;
    register(
        &owner_b,
        "asp-testing-1",
        Some("root-1"),
        "asp_testing",
        AgentSessionDelegationCapability::FocusedLeaf,
    )
    .await;
    let request_a = proposal(
        "event-workspace-a",
        "asp-testing-1",
        "nested-child-a",
        AgentSessionDelegationCapability::Standard,
        0,
    );
    let request_b = proposal(
        "event-workspace-b",
        "asp-testing-1",
        "nested-child-b",
        AgentSessionDelegationCapability::Standard,
        0,
    );
    let barrier = std::sync::Arc::new(tokio::sync::Barrier::new(256));
    let mut tasks = Vec::with_capacity(256);
    for index in 0..256 {
        let (owner, request) = if index % 2 == 0 {
            (owner_a.clone(), request_a.clone())
        } else {
            (owner_b.clone(), request_b.clone())
        };
        let barrier = barrier.clone();
        tasks.push(tokio::spawn(async move {
            barrier.wait().await;
            owner.admit_delegation(&request).await
        }));
    }
    for task in tasks {
        task.await
            .expect("join multi-workspace admission")
            .expect("multi-workspace admission");
    }
    for owner in [&owner_a, &owner_b] {
        let metrics = owner.runtime_metrics().await;
        assert_eq!(metrics.durable_admission_transactions, 1);
        assert_eq!(metrics.resident_receipt_replays, 127);
        assert_eq!(metrics.resident_lane_count, 1);
    }
}

#[tokio::test]
async fn runtime_registry_starts_one_workspace_runtime_under_concurrency() {
    let root = tempfile::tempdir().expect("runtime registry tempdir");
    let registry = SessionControlPlaneRuntimeRegistry::default();
    let barrier = std::sync::Arc::new(tokio::sync::Barrier::new(256));
    let mut tasks = Vec::with_capacity(256);
    for _ in 0..256 {
        let registry = registry.clone();
        let client_dir = root.path().to_path_buf();
        let barrier = barrier.clone();
        tasks.push(tokio::spawn(async move {
            barrier.wait().await;
            registry.runtime_for_client_dir(client_dir).await
        }));
    }
    let mut runtimes = Vec::with_capacity(256);
    for task in tasks {
        runtimes.push(
            task.await
                .expect("join runtime lookup")
                .expect("workspace runtime"),
        );
    }
    let first = runtimes.first().expect("first runtime");
    assert!(
        runtimes
            .iter()
            .all(|runtime| std::sync::Arc::ptr_eq(first, runtime))
    );
    assert_eq!(registry.resident_workspace_count().await, 1);
    registry.shutdown_all().await.expect("shutdown registry");
    assert_eq!(registry.resident_workspace_count().await, 0);
}

#[tokio::test]
async fn shutdown_closes_transition_stream_and_joins_actor() {
    let (_root, owner) = owner().await;
    register(
        &owner,
        "asp-testing-1",
        Some("root-1"),
        "asp_testing",
        AgentSessionDelegationCapability::FocusedLeaf,
    )
    .await;
    let running_metrics = owner.runtime_metrics().await;
    assert_eq!(running_metrics.actor_starts, 1);
    assert_eq!(running_metrics.actor_stops, 0);
    assert_eq!(running_metrics.queue_depth, 0);
    assert!(running_metrics.queue_capacity >= 256);
    assert!(running_metrics.queue_high_watermark >= 1);
    assert_eq!(running_metrics.enqueued_transitions, 1);
    assert_eq!(running_metrics.completed_transitions, 1);
    owner.shutdown().await.expect("shutdown transition stream");
    let stopped_metrics = owner.runtime_metrics().await;
    assert_eq!(stopped_metrics.actor_starts, 1);
    assert_eq!(stopped_metrics.actor_stops, 1);
    assert_eq!(stopped_metrics.queue_depth, 0);
    assert_eq!(
        stopped_metrics.enqueued_transitions,
        stopped_metrics.completed_transitions
    );
    let error = owner
        .admit_delegation(&proposal(
            "event-after-shutdown",
            "asp-testing-1",
            "nested-child-1",
            AgentSessionDelegationCapability::Standard,
            0,
        ))
        .await
        .expect_err("closed stream must reject new transition");
    assert_eq!(error, "session control-plane transition stream is closed");
}
