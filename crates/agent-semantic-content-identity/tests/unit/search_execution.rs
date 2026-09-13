// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

use super::SEARCH_EXECUTION_SCHEMA_ID;
use super::SEARCH_EXECUTION_SCHEMA_VERSION;
use super::SearchClientFrame;
use super::SearchExecution;
use super::SearchExecutionError;
use super::SearchOperation;
use super::TerminalStatus;
use crate::content_binding::AuthorityStamp;
use crate::content_binding::ContentBinding;
use crate::content_binding::ContentIdentity;
use crate::content_binding::ContentPublicationCommit;

fn identity() -> ContentIdentity {
    let digest = "blake3-256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
    ContentIdentity {
        runtime_artifact_digest: digest.into(),
        workspace_snapshot_digest: digest.into(),
        source_generation_digest: digest.into(),
        source_index_digest: digest.into(),
        schema_digest: digest.into(),
        provider_catalog_digest: digest.into(),
    }
}

fn commit() -> ContentPublicationCommit {
    let identity = identity();
    let digest = identity.digest();
    ContentPublicationCommit::linearize(
        identity,
        AuthorityStamp {
            key_id: "test-key".into(),
            canonical_digest: digest,
            signature: "test-signature".into(),
        },
    )
    .expect("valid commit")
}

#[test]
fn operation_wire_names_match_the_search_execution_schema() {
    for (operation, expected) in [
        (SearchOperation::Playbook, "playbook"),
        (SearchOperation::Query, "query"),
        (SearchOperation::Exact, "exact"),
    ] {
        assert_eq!(
            serde_json::to_value(operation).expect("serialize Search operation"),
            expected
        );
    }
}

#[test]
fn binding_pins_one_exact_context_for_all_operations() {
    let commit = commit();
    let execution = SearchExecution::bind(commit.content_binding.clone(), &commit).expect("bind");
    let frame = SearchClientFrame {
        frame_schema_id: SEARCH_EXECUTION_SCHEMA_ID.into(),
        frame_schema_version: SEARCH_EXECUTION_SCHEMA_VERSION.into(),
        request_id: "request-1".into(),
        session_id: "session-1".into(),
        operation: SearchOperation::Query,
        context: execution.context().clone(),
        selector: Some("rust://item/1".into()),
        cancellation_id: "cancel-1".into(),
    };
    execution
        .admit(&frame, &commit)
        .expect("same content admits");
}

#[test]
fn a_foreign_authority_cannot_reuse_an_admitted_content_identity() {
    let commit = commit();
    let forged = ContentBinding::new(
        commit.identity().clone(),
        AuthorityStamp {
            key_id: "foreign-authority".into(),
            canonical_digest: commit.identity().digest(),
            signature: "foreign-signature".into(),
        },
    )
    .expect("foreign stamp is structurally valid");
    assert!(matches!(
        SearchExecution::bind(forged, &commit),
        Err(SearchExecutionError::Binding(
            crate::content_binding::ContentBindingError::ContentMismatch
        ))
    ));
}

#[test]
fn cancellation_is_terminal_and_cannot_be_followed_by_success() {
    let commit = commit();
    let mut execution =
        SearchExecution::bind(commit.content_binding.clone(), &commit).expect("bind");
    execution.cancel("request-1".into()).expect("cancel");
    assert_eq!(
        execution.finish("request-1".into(), TerminalStatus::Succeeded),
        Err(SearchExecutionError::AlreadyTerminal)
    );
    assert_eq!(
        execution.terminal().map(|receipt| receipt.status),
        Some(TerminalStatus::Cancelled)
    );
}
