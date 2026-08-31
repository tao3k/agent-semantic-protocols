use agent_semantic_content_identity::content_binding::{
    AuthorityStamp, ContentBinding, ContentIdentity, ContentPublicationCommit,
};
use agent_semantic_content_identity::runtime_execution::RuntimeExecutionBinding;
use agent_semantic_content_identity::search_execution::{
    SEARCH_EXECUTION_SCHEMA_ID, SEARCH_EXECUTION_SCHEMA_VERSION, SearchClientFrame,
    SearchExecution, SearchExecutionError, SearchOperation, TerminalStatus,
};

fn identity(seed: char) -> ContentIdentity {
    let digest = format!(
        "blake3-256:{}",
        std::iter::repeat(seed).take(64).collect::<String>()
    );
    ContentIdentity {
        runtime_artifact_digest: digest.clone(),
        workspace_snapshot_digest: digest.clone(),
        source_generation_digest: digest.clone(),
        source_index_digest: digest.clone(),
        schema_digest: digest.clone(),
        provider_catalog_digest: digest,
    }
}

fn commit(seed: char) -> ContentPublicationCommit {
    let identity = identity(seed);
    let digest = identity.digest();
    ContentPublicationCommit::linearize(
        identity,
        AuthorityStamp {
            key_id: "test-key".into(),
            canonical_digest: digest,
            signature: "test-signature".into(),
        },
    )
    .expect("valid content commit")
}

fn frame(
    execution: &SearchExecution,
    operation: SearchOperation,
    selector: Option<&str>,
) -> SearchClientFrame {
    SearchClientFrame {
        frame_schema_id: SEARCH_EXECUTION_SCHEMA_ID.into(),
        frame_schema_version: SEARCH_EXECUTION_SCHEMA_VERSION.into(),
        request_id: format!("request-{operation:?}"),
        session_id: "session-1".into(),
        operation,
        context: execution.context().clone(),
        selector: selector.map(str::to_owned),
        cancellation_id: "cancel-1".into(),
    }
}

#[test]
fn prime_pins_all_search_operations_to_one_content_commit() {
    let commit = commit('a');
    let binding = ContentBinding::new(
        commit.identity.clone(),
        AuthorityStamp {
            key_id: "test-key".into(),
            canonical_digest: commit.commit_digest.clone(),
            signature: "test-signature".into(),
        },
    )
    .expect("valid binding");
    let execution = SearchExecution::prime(binding, &commit).expect("prime");

    for (operation, selector) in [
        (SearchOperation::QuerySet, None),
        (SearchOperation::Search, None),
        (SearchOperation::Query, Some("rust://item/1")),
        (SearchOperation::Exact, Some("rust://item/1")),
        (SearchOperation::Pipe, Some("rust://item/1")),
    ] {
        execution
            .admit(&frame(&execution, operation, selector), &commit)
            .expect("same exact content must admit");
    }
}

#[test]
fn a_cross_commit_frame_is_rejected_even_when_its_selector_is_valid() {
    let first = commit('a');
    let second = commit('b');
    let binding = ContentBinding::new(
        first.identity.clone(),
        AuthorityStamp {
            key_id: "test-key".into(),
            canonical_digest: first.commit_digest.clone(),
            signature: "test-signature".into(),
        },
    )
    .expect("valid binding");
    let execution = SearchExecution::prime(binding, &first).expect("prime");
    let cross_commit = frame(&execution, SearchOperation::Exact, Some("rust://item/1"));

    assert_eq!(
        execution.admit(&cross_commit, &second),
        Err(SearchExecutionError::Binding(
            agent_semantic_content_identity::content_binding::ContentBindingError::ContentMismatch,
        ))
    );
}

#[test]
fn cancellation_emits_the_only_terminal_and_blocks_late_success() {
    let commit = commit('a');
    let binding = ContentBinding::new(
        commit.identity.clone(),
        AuthorityStamp {
            key_id: "test-key".into(),
            canonical_digest: commit.commit_digest.clone(),
            signature: "test-signature".into(),
        },
    )
    .expect("valid binding");
    let mut execution = SearchExecution::prime(binding, &commit).expect("prime");
    let receipt = execution.cancel("request-1".into()).expect("cancel");
    assert_eq!(receipt.status, TerminalStatus::Cancelled);
    assert_eq!(
        execution.finish("request-1".into(), TerminalStatus::Succeeded),
        Err(SearchExecutionError::AlreadyTerminal)
    );
}

#[test]
fn prime_can_pin_the_exact_runtime_and_evaluator_binding() {
    let commit = commit('a');
    let digest = "blake3-256:bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
    let runtime_binding = RuntimeExecutionBinding::new(
        ContentBinding::new(
            commit.identity.clone(),
            AuthorityStamp {
                key_id: "test-key".into(),
                canonical_digest: commit.commit_digest.clone(),
                signature: "test-signature".into(),
            },
        )
        .expect("valid binding"),
        digest,
        digest,
        digest,
        digest,
    )
    .expect("valid runtime binding");
    let execution = SearchExecution::prime_with_runtime_binding(
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
        runtime_binding.clone(),
    )
    .expect("prime");
    assert_eq!(execution.context().runtime_binding, Some(runtime_binding));
}
