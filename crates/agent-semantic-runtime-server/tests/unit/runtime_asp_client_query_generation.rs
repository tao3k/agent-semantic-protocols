// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

use agent_semantic_client_db::runtime_server_workspace::WorkspaceRuntimeSelectorRead;
use agent_semantic_client_protocol::AspClientExactQueryRequest;

use super::QueryNotReadyContext;
use super::classify_exact_query_failure;
use super::dispatch_budget_for_method;
use super::query_generation_not_ready_error;

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
fn cold_generation_admission_has_no_interactive_dispatch_deadline() {
    assert_eq!(
        dispatch_budget_for_method(
            agent_semantic_client_protocol::WORKSPACE_GENERATION_ENSURE_READY_METHOD
        ),
        None
    );
    assert_eq!(dispatch_budget_for_method("rust.query"), None);
    assert_eq!(dispatch_budget_for_method("rust.search"), None);
}
