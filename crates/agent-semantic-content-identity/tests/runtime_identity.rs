use agent_semantic_content_identity::content_binding::{
    AuthorityStamp, ContentBinding, ContentIdentity, ContentPublicationCommit,
};
use agent_semantic_content_identity::host_session_binding::{
    HostSessionBinding, HostSessionBindingError,
};
use agent_semantic_content_identity::runtime_execution::{
    RuntimeExecutionBinding, RuntimeExecutionBindingError,
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

fn commit(identity: ContentIdentity, expected: Option<&str>) -> ContentPublicationCommit {
    let digest = identity.digest();
    ContentPublicationCommit::linearize_with_expected(
        identity,
        AuthorityStamp {
            key_id: "test-key".into(),
            canonical_digest: digest,
            signature: "test-signature".into(),
        },
        expected,
    )
    .expect("valid content commit")
}

fn binding(commit: &ContentPublicationCommit) -> ContentBinding {
    ContentBinding::new(
        commit.identity.clone(),
        AuthorityStamp {
            key_id: "test-key".into(),
            canonical_digest: commit.commit_digest.clone(),
            signature: "test-signature".into(),
        },
    )
    .expect("valid content binding")
}

fn runtime_binding(commit: &ContentPublicationCommit, seed: char) -> RuntimeExecutionBinding {
    let digest = format!(
        "blake3-256:{}",
        std::iter::repeat(seed).take(64).collect::<String>()
    );
    RuntimeExecutionBinding::new(
        binding(commit),
        digest.clone(),
        digest.clone(),
        digest.clone(),
        digest,
    )
    .expect("valid runtime binding")
}

#[test]
fn same_content_keeps_identity_when_publication_sequence_changes() {
    let first = commit(identity('a'), None);
    let replacement = commit(identity('a'), Some(&first.commit_digest));
    assert_eq!(first.identity.digest(), replacement.identity.digest());
    assert_eq!(first.commit_digest, replacement.commit_digest);
    assert_eq!(first.expected_digest, None);
    assert_eq!(replacement.expected_digest, Some(first.commit_digest));
    assert_ne!(first.mutation_id, replacement.mutation_id);
    assert_ne!(first.lease_id, replacement.lease_id);
}

#[test]
fn same_policy_with_different_runtime_binary_is_rejected() {
    let commit = commit(identity('a'), None);
    let left = runtime_binding(&commit, 'a');
    let right = RuntimeExecutionBinding::new(
        binding(&commit),
        "blake3-256:bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
        left.evaluator_policy_digest.clone(),
        left.active_artifact_receipt_digest.clone(),
        left.evaluator_abi_digest.clone(),
    )
    .expect("valid alternate runtime binding");
    assert_eq!(
        left.admits(&right),
        Err(RuntimeExecutionBindingError::ContentMismatch)
    );
}

#[test]
fn host_binding_reads_identity_once_and_rejects_parent_current_aliasing() {
    let mut values = std::collections::BTreeMap::from([
        ("ASP_HOST_PLATFORM", "macos"),
        ("ASP_ROOT_SESSION_ID", "root"),
        ("CODEX_SESSION_ID", "parent"),
        ("CODEX_THREAD_ID", "current"),
    ]);
    let bound = HostSessionBinding::from_environment(|key| values.get(key).map(|v| (*v).into()))
        .expect("valid host binding");
    values.insert("CODEX_THREAD_ID", "changed");
    assert_eq!(bound.current_session_id, "current");

    assert_eq!(
        HostSessionBinding::new("macos", "root", "same", "same"),
        Err(HostSessionBindingError::ParentEqualsCurrent)
    );
}
