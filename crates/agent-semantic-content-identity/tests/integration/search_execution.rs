use agent_semantic_content_identity::content_binding::AuthorityStamp;
use agent_semantic_content_identity::content_binding::ContentBinding;
use agent_semantic_content_identity::content_binding::ContentIdentity;
use agent_semantic_content_identity::content_binding::ContentPublicationCommit;
use agent_semantic_content_identity::runtime_execution::RuntimeExecutionBinding;
use agent_semantic_content_identity::runtime_execution::RuntimeExecutionBindingInput;
use agent_semantic_content_identity::search_execution::SEARCH_EXECUTION_SCHEMA_ID;
use agent_semantic_content_identity::search_execution::SEARCH_EXECUTION_SCHEMA_VERSION;
use agent_semantic_content_identity::search_execution::SearchClientFrame;
use agent_semantic_content_identity::search_execution::SearchExecution;
use agent_semantic_content_identity::search_execution::SearchExecutionError;
use agent_semantic_content_identity::search_execution::SearchOperation;
use agent_semantic_content_identity::search_execution::TerminalStatus;

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
        request_id: format!("request-{operation:?}").into(),
        session_id: "session-1".into(),
        operation,
        context: execution.context().clone(),
        selector: selector.map(str::to_owned),
        cancellation_id: "cancel-1".into(),
    }
}

#[test]
fn binding_pins_all_search_operations_to_one_content_commit() {
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
    let execution = SearchExecution::bind(binding, &commit).expect("bind");

    for (operation, selector) in [
        (SearchOperation::Playbook, None),
        (SearchOperation::Query, Some("rust://item/1")),
        (SearchOperation::Exact, Some("rust://item/1")),
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
    let execution = SearchExecution::bind(binding, &first).expect("bind");
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
    let mut execution = SearchExecution::bind(binding, &commit).expect("bind");
    let receipt = execution.cancel("request-1".into()).expect("cancel");
    assert_eq!(receipt.status, TerminalStatus::Cancelled);
    assert_eq!(
        execution.finish("request-1".into(), TerminalStatus::Succeeded),
        Err(SearchExecutionError::AlreadyTerminal)
    );
}

#[test]
fn binding_can_pin_the_exact_runtime_and_evaluator_binding() {
    let commit = commit('a');
    let digest = "blake3-256:bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
    let runtime_binding = RuntimeExecutionBinding::new(RuntimeExecutionBindingInput {
        project_id: "project-a".into(),
        workspace_id: "workspace-a".into(),
        publication_nonce: "publication-a".into(),
        content_binding: ContentBinding::new(
            commit.identity.clone(),
            AuthorityStamp {
                key_id: "test-key".into(),
                canonical_digest: commit.commit_digest.clone(),
                signature: "test-signature".into(),
            },
        )
        .expect("valid binding"),
        runtime_artifact_digest: commit.identity.runtime_artifact_digest.clone().into(),
        evaluator_policy_digest: digest.into(),
        active_artifact_receipt_digest: digest.into(),
        evaluator_abi_digest: digest.into(),
    })
    .expect("valid runtime binding");
    let execution = SearchExecution::bind_with_runtime_binding(
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
    .expect("bind");
    assert_eq!(execution.context().runtime_binding, Some(runtime_binding));
}
