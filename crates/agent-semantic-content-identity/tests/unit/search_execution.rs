use super::{
    SEARCH_EXECUTION_SCHEMA_ID, SEARCH_EXECUTION_SCHEMA_VERSION, SearchClientFrame,
    SearchExecution, SearchExecutionError, SearchOperation, TerminalStatus,
};
use crate::content_binding::{
    AuthorityStamp, ContentBinding, ContentIdentity, ContentPublicationCommit,
};

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
fn binding_pins_one_exact_context_for_all_operations() {
    let commit = commit();
    let execution = SearchExecution::bind(
        ContentBinding::new(
            commit.identity.clone(),
            AuthorityStamp {
                key_id: "test-key".into(),
                canonical_digest: commit.commit_digest.clone(),
                signature: "test-signature".into(),
            },
        )
        .expect("valid binding"),
        &commit,
    )
    .expect("bind");
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
fn cancellation_is_terminal_and_cannot_be_followed_by_success() {
    let commit = commit();
    let mut execution = SearchExecution::bind(
        ContentBinding::new(
            commit.identity.clone(),
            AuthorityStamp {
                key_id: "test-key".into(),
                canonical_digest: commit.commit_digest.clone(),
                signature: "test-signature".into(),
            },
        )
        .expect("valid binding"),
        &commit,
    )
    .expect("bind");
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
