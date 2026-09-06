// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-only

use agent_semantic_content_identity::content_binding::AuthorityStamp;
use agent_semantic_content_identity::content_binding::ContentBinding;
use agent_semantic_content_identity::content_binding::ContentIdentity;
use agent_semantic_content_identity::content_binding::ContentPublicationCommit;
use agent_semantic_content_identity::host_session_binding::HostSessionBinding;
use agent_semantic_content_identity::host_session_binding::HostSessionBindingError;
use agent_semantic_content_identity::runtime_execution::RuntimeExecutionBinding;
use agent_semantic_content_identity::runtime_execution::RuntimeExecutionBindingError;
use agent_semantic_content_identity::runtime_execution::RuntimeExecutionBindingInput;

fn identity(digest_char: char) -> ContentIdentity {
    let digest = format!(
        "blake3-256:{}",
        std::iter::repeat_n(digest_char, 64).collect::<String>()
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

fn runtime_binding(
    commit: &ContentPublicationCommit,
    digest_char: char,
) -> RuntimeExecutionBinding {
    let digest = format!(
        "blake3-256:{}",
        std::iter::repeat_n(digest_char, 64).collect::<String>()
    );
    RuntimeExecutionBinding::new(RuntimeExecutionBindingInput {
        project_id: "project-a".into(),
        workspace_id: "workspace-a".into(),
        publication_nonce: "publication-a".into(),
        content_binding: binding(commit),
        runtime_artifact_digest: commit.identity.runtime_artifact_digest.clone().into(),
        evaluator_policy_digest: digest.clone().into(),
        active_artifact_receipt_digest: digest.clone().into(),
        evaluator_abi_digest: digest.into(),
    })
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
    let error = RuntimeExecutionBinding::new(RuntimeExecutionBindingInput {
        project_id: left.project_id.clone(),
        workspace_id: left.workspace_id.clone(),
        publication_nonce: left.publication_nonce.clone(),
        content_binding: binding(&commit),
        runtime_artifact_digest:
            "blake3-256:bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb".into(),
        evaluator_policy_digest: left.evaluator_policy_digest.clone(),
        active_artifact_receipt_digest: left.active_artifact_receipt_digest.clone(),
        evaluator_abi_digest: left.evaluator_abi_digest.clone(),
    })
    .expect_err("content binding and Runtime artifact must be one identity");
    assert_eq!(error, RuntimeExecutionBindingError::RuntimeArtifactMismatch);
}

#[test]
fn project_workspace_and_publication_nonce_are_part_of_runtime_identity() {
    let commit = commit(identity('a'), None);
    let expected = runtime_binding(&commit, 'b');
    for observed in [
        RuntimeExecutionBinding {
            project_id: "project-b".into(),
            ..expected.clone()
        },
        RuntimeExecutionBinding {
            workspace_id: "workspace-b".into(),
            ..expected.clone()
        },
        RuntimeExecutionBinding {
            publication_nonce: "publication-b".into(),
            ..expected.clone()
        },
    ] {
        assert_eq!(
            expected.admits(&observed),
            Err(RuntimeExecutionBindingError::ContentMismatch)
        );
    }
}

#[test]
fn legacy_v1_binding_decodes_but_requires_runtime_refresh() {
    let commit = commit(identity('a'), None);
    let canonical = runtime_binding(&commit, 'b');
    let mut legacy = serde_json::to_value(&canonical).expect("encode canonical binding");
    let object = legacy
        .as_object_mut()
        .expect("runtime binding is a JSON object");
    object.remove("projectId");
    object.remove("workspaceId");
    object.remove("publicationNonce");
    let decoded: RuntimeExecutionBinding =
        serde_json::from_value(legacy).expect("legacy V1 document remains decodable");
    assert!(decoded.refresh_required());
    assert_eq!(
        decoded.validate(),
        Err(RuntimeExecutionBindingError::MissingIdentity { field: "projectId" })
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
