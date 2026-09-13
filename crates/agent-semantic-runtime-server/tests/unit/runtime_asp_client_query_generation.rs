// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

use agent_semantic_client_db::runtime_server_workspace::WorkspaceRuntimeSelectorRead;
use agent_semantic_client_protocol::AspClientExactQueryRequest;

use super::QueryNotReadyContext;
use super::RUNTIME_CLIENT_DISPATCH_BUDGET;
use super::classify_exact_query_failure;
use super::dispatch_budget_for_method;
use super::enforce_completed_dispatch_budget;
use super::query_generation_not_ready_error;

#[tokio::test(start_paused = true)]
async fn ready_request_keeps_one_ms_deadline_independent_of_another_first_request() {
    let method = agent_semantic_client_protocol::WORKSPACE_SEARCH_PLAYBOOK_METHOD;
    let ready = super::RequestDispatchBudget::for_method(method);
    let first = super::RequestDispatchBudget::for_method(method);
    ready.observe_resident_hit();
    first.observe_miss();
    first.observe_miss();
    assert_eq!(ready.limit(), Some(RUNTIME_CLIENT_DISPATCH_BUDGET));
    assert_eq!(
        first.limit(),
        Some(super::FIRST_COMPUTATION_OBSERVATION_BUDGET)
    );
    let started = tokio::time::Instant::now();
    ready.expired(started).await;
    assert_eq!(started.elapsed(), RUNTIME_CLIENT_DISPATCH_BUDGET);
}

#[tokio::test(start_paused = true)]
async fn classification_cannot_cancel_a_late_first_computation_as_resident() {
    let budget = super::RequestDispatchBudget::for_method(
        agent_semantic_client_protocol::WORKSPACE_SEARCH_PLAYBOOK_METHOD,
    );
    let started = tokio::time::Instant::now();
    let expired = budget.expired(started);
    tokio::pin!(expired);

    tokio::time::advance(RUNTIME_CLIENT_DISPATCH_BUDGET).await;
    assert!(
        futures_util::poll!(expired.as_mut()).is_pending(),
        "classification is not proof of a resident hit"
    );

    budget.observe_miss();
    tokio::time::advance(
        super::FIRST_COMPUTATION_OBSERVATION_BUDGET - RUNTIME_CLIENT_DISPATCH_BUDGET,
    )
    .await;
    expired.await;
    assert_eq!(
        started.elapsed(),
        super::FIRST_COMPUTATION_OBSERVATION_BUDGET
    );
}

#[tokio::test(start_paused = true)]
async fn late_resident_classification_retains_the_original_one_ms_deadline() {
    let budget = super::RequestDispatchBudget::for_method(
        agent_semantic_client_protocol::WORKSPACE_SEARCH_PLAYBOOK_METHOD,
    );
    let started = tokio::time::Instant::now();
    tokio::time::advance(RUNTIME_CLIENT_DISPATCH_BUDGET * 2).await;
    budget.observe_resident_hit();
    budget.expired(started).await;
    assert_eq!(budget.limit(), Some(RUNTIME_CLIENT_DISPATCH_BUDGET));
    assert_eq!(started.elapsed(), RUNTIME_CLIENT_DISPATCH_BUDGET * 2);
}

#[tokio::test(start_paused = true)]
async fn first_computation_deadline_is_absolute_not_restarted_at_a_later_stage() {
    let budget = super::RequestDispatchBudget::for_method(
        agent_semantic_client_protocol::WORKSPACE_QUERY_PLAYBOOK_METHOD,
    );
    let started = tokio::time::Instant::now();
    budget.observe_miss();
    tokio::time::advance(std::time::Duration::from_secs(45)).await;
    budget.observe_miss();
    budget.expired(started).await;
    assert_eq!(
        started.elapsed(),
        super::FIRST_COMPUTATION_OBSERVATION_BUDGET
    );
    assert!(
        super::enforce_completed_dispatch_budget(
            Ok(serde_json::json!({"state":"ready"})),
            budget.limit(),
            started.elapsed(),
        )
        .is_err(),
        "equality with the first-computation boundary is still failure"
    );
}

#[tokio::test]
async fn generation_wait_retains_failure_and_does_not_accept_another_workspace() {
    use crate::runtime_query_generation_key::RuntimeProjectWorkspaceKey;
    use std::{collections::HashMap, sync::Arc};
    let key = RuntimeProjectWorkspaceKey::new(
        agent_semantic_client_protocol::ClientProjectId::new("project-test").unwrap(),
        agent_semantic_client_protocol::ClientWorkspaceIdentity::new("workspace-test").unwrap(),
    );
    let other = RuntimeProjectWorkspaceKey::new(
        key.project_id().clone(),
        agent_semantic_client_protocol::ClientWorkspaceIdentity::new("other-workspace").unwrap(),
    );
    let failed = crate::RuntimeQueryGenerationState::Failed {
        expected_generation_digest: Arc::from("expected"),
        reason: Arc::from("source binding changed"),
    };
    let (sender, receiver) = tokio::sync::watch::channel(Arc::new(HashMap::new()));
    let wait = super::await_runtime_query_generation(&receiver, &key);
    tokio::pin!(wait);
    assert!(futures_util::poll!(&mut wait).is_pending());
    sender.send_replace(Arc::new(HashMap::from([(other, failed.clone())])));
    assert!(futures_util::poll!(&mut wait).is_pending());
    sender.send_replace(Arc::new(HashMap::from([(key.clone(), failed)])));
    assert_eq!(wait.await.err().unwrap(), "source binding changed");
    assert_eq!(
        super::await_runtime_query_generation(&receiver, &key)
            .await
            .err()
            .unwrap(),
        "source binding changed"
    );
}

#[tokio::test]
async fn missing_generation_with_closed_publication_is_terminal() {
    let key = crate::runtime_query_generation_key::RuntimeProjectWorkspaceKey::new(
        agent_semantic_client_protocol::ClientProjectId::new("project-test").unwrap(),
        agent_semantic_client_protocol::ClientWorkspaceIdentity::new("workspace-test").unwrap(),
    );
    let (sender, receiver) =
        tokio::sync::watch::channel(std::sync::Arc::new(std::collections::HashMap::new()));
    drop(sender);
    assert!(
        super::await_runtime_query_generation(&receiver, &key)
            .await
            .err()
            .unwrap()
            .contains("closed")
    );
}

#[test]
fn projection_missing_is_a_precise_runtime_terminal() {
    let failure = classify_exact_query_failure(&WorkspaceRuntimeSelectorRead::ProjectionMissing {
        generation_digest: format!("blake3-256:{}", "a".repeat(64)),
        root_digest: "b".repeat(64),
        resolved_selector: "rust://src/lib.rs#item/function/missing".to_owned(),
    })
    .expect("typed failure");

    assert_eq!(failure.reason_kind, "projection-missing");
    assert_eq!(
        failure.resolved_selector.as_deref(),
        Some("rust://src/lib.rs#item/function/missing")
    );
}

#[test]
fn payload_bearing_projection_is_not_classified_as_failure() {
    let ready = WorkspaceRuntimeSelectorRead::Projection {
        generation_digest: format!("blake3-256:{}", "a".repeat(64)),
        root_digest: "b".repeat(64),
        resolved_selector: "rust://src/lib.rs#item/function/ready".to_owned(),
        bytes: b"fn ready() {}".to_vec(),
    };
    assert!(classify_exact_query_failure(&ready).is_none());
}

#[test]
fn unpublished_generation_returns_typed_query_not_ready_after_readiness_submission() {
    let request = AspClientExactQueryRequest {
        schema_id: "agent.semantic-protocols.asp-client-exact-query-request".to_owned(),
        schema_version: "1".to_owned(),
        selector: "rust://src/lib.rs#item/function/ready".to_owned(),
        projection: "source".to_owned(),
    };
    let error = query_generation_not_ready_error(QueryNotReadyContext {
        operation_id: "request-1",
        project_id: "repo-1",
        workspace_id: "workspace-1",
        language_id: "rust",
        provider_id: "asp-rust",
        exact_query: Some(&request),
        generation_state: "unpublished",
        publication_error: None,
        elapsed_micros: 7,
    })
    .expect("typed query readiness terminal");

    assert_eq!(error.reason_kind, "query-not-ready");
    assert_eq!(
        error.message,
        "no immutable CompleteGeneration is published for this workspace: state=unpublished"
    );
    let terminal = error.details.expect("typed exact-query terminal");
    assert_eq!(terminal["state"], "failed");
    assert_eq!(terminal["phase"], "runtime-generation-authority");
    assert_eq!(terminal["reasonKind"], "query-not-ready");
    assert_eq!(terminal["workCounters"]["filesystemReadCount"], 0);
    assert_eq!(terminal["workCounters"]["databaseReadCount"], 0);
    assert_eq!(terminal["workCounters"]["providerProcessCount"], 0);
    assert!(terminal.get("recommendedNext").is_none());
}

#[test]
fn readiness_submission_failure_remains_one_typed_query_not_ready_terminal() {
    let request = AspClientExactQueryRequest {
        schema_id: "agent.semantic-protocols.asp-client-exact-query-request".to_owned(),
        schema_version: "1".to_owned(),
        selector: "rust://src/lib.rs#item/function/ready".to_owned(),
        projection: "source".to_owned(),
    };
    let error = query_generation_not_ready_error(QueryNotReadyContext {
        operation_id: "request-1",
        project_id: "repo-1",
        workspace_id: "workspace-1",
        language_id: "rust",
        provider_id: "asp-rust",
        exact_query: Some(&request),
        generation_state: "submission-failed",
        publication_error: Some("runtime generation admission dispatcher is closed"),
        elapsed_micros: 7,
    })
    .expect("typed readiness submission failure");

    assert_eq!(error.reason_kind, "query-not-ready");
    assert_eq!(
        error.message,
        "no immutable CompleteGeneration is published for this workspace: state=submission-failed cause=runtime generation admission dispatcher is closed"
    );
    let terminal = error.details.expect("typed exact-query terminal");
    assert_eq!(terminal["details"]["generationState"], "submission-failed");
    assert_eq!(
        terminal["details"]["publicationError"],
        "runtime generation admission dispatcher is closed"
    );
    assert_eq!(terminal["workCounters"]["providerProcessCount"], 0);
}

#[test]
fn detached_generation_has_no_request_deadline_but_resident_reads_do() {
    assert_eq!(
        dispatch_budget_for_method(
            agent_semantic_client_protocol::WORKSPACE_GENERATION_ENSURE_READY_METHOD
        ),
        None
    );
    assert_eq!(
        dispatch_budget_for_method("rust.query"),
        Some(std::time::Duration::from_micros(1_000))
    );
    assert_eq!(
        dispatch_budget_for_method(
            agent_semantic_client_protocol::WORKSPACE_SEARCH_PLAYBOOK_METHOD
        ),
        Some(RUNTIME_CLIENT_DISPATCH_BUDGET),
        "Search and Query share the strict resident request-plane deadline"
    );
}

#[test]
fn synchronous_overrun_cannot_escape_the_resident_dispatch_deadline() {
    let error = enforce_completed_dispatch_budget(
        Ok(serde_json::json!({"state": "ready"})),
        Some(std::time::Duration::from_micros(1_000)),
        std::time::Duration::from_micros(1_001),
    )
    .expect_err("a completed synchronous operation must still be checked against the deadline");

    assert_eq!(error.reason_kind, "client-request-deadline-exceeded");
    assert_eq!(
        error.details.expect("typed deadline terminal")["phase"],
        "runtime-client-dispatch-completion"
    );
}

#[test]
fn strict_dispatch_deadline_accepts_only_elapsed_time_below_the_budget() {
    assert!(
        enforce_completed_dispatch_budget(
            Ok(serde_json::json!({"state": "ready"})),
            Some(std::time::Duration::from_micros(1_000)),
            std::time::Duration::from_micros(999),
        )
        .is_ok()
    );
    assert!(
        enforce_completed_dispatch_budget(
            Ok(serde_json::json!({"state": "ready"})),
            Some(std::time::Duration::from_micros(1_000)),
            std::time::Duration::from_micros(1_000),
        )
        .is_err()
    );
}
