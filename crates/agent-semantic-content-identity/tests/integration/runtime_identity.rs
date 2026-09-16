// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

use agent_semantic_content_identity::HostWorkspaceInitializationBinding;
use agent_semantic_content_identity::ProjectWorkspaceBinding;
use agent_semantic_content_identity::content_binding::AuthorityStamp;
use agent_semantic_content_identity::content_binding::ContentBinding;
use agent_semantic_content_identity::content_binding::ContentIdentity;
use agent_semantic_content_identity::content_binding::ContentPublicationCommit;
use agent_semantic_content_identity::host_session_binding::HostSessionBinding;
use agent_semantic_content_identity::host_session_binding::HostSessionBindingError;
use agent_semantic_content_identity::runtime_execution::RuntimeExecutionBinding;
use agent_semantic_content_identity::runtime_execution::RuntimeExecutionBindingError;
use agent_semantic_content_identity::runtime_execution::RuntimeExecutionBindingInput;
use agent_semantic_content_identity::runtime_workspace_execution_pointer::RuntimeWorkspaceExecutionPointer;
use agent_semantic_content_identity::runtime_workspace_execution_pointer::RuntimeWorkspaceExecutionPointerError;
use agent_semantic_content_identity::runtime_workspace_execution_publication::RuntimeWorkspaceExecutionPublication;
use agent_semantic_content_identity::runtime_workspace_execution_publication::RuntimeWorkspaceExecutionPublicationError;
use agent_semantic_content_identity::runtime_workspace_execution_publication::RuntimeWorkspaceExecutionPublicationInput;

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
    commit.content_binding.clone()
}

fn project_workspace() -> ProjectWorkspaceBinding {
    ProjectWorkspaceBinding::new(
        "git+https://github.com/tao3k/agent-semantic-protocols.git#workspace/root",
        ".",
        "cross-machine",
        Vec::new(),
    )
    .expect("valid Project Workspace")
}

#[test]
fn host_workspace_initialization_is_an_exact_independent_product() {
    let admitted = HostWorkspaceInitializationBinding::new(project_workspace(), "worktree-a")
        .expect("Host workspace initialization binding");
    admitted
        .admit_exact(&admitted)
        .expect("exact Host product is admitted");

    let workspace_id_substitute =
        HostWorkspaceInitializationBinding::new(project_workspace(), "workspace-a")
            .expect("structurally valid but independently foreign Host product");
    let error = admitted
        .admit_exact(&workspace_id_substitute)
        .expect_err("workspaceId cannot substitute for the Host worktree instance");
    assert_eq!(
        error.reason_kind(),
        "host-workspace-initialization-binding-mismatch"
    );
}

#[test]
fn empty_host_worktree_identity_is_rejected() {
    let error = HostWorkspaceInitializationBinding::new(project_workspace(), "  ")
        .expect_err("Host must supply a worktree instance identity");
    assert_eq!(error.reason_kind(), "host-worktree-instance-missing");
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
        project_workspace: project_workspace(),
        worktree_instance_id: "workspace-a".into(),
        publication_nonce: "publication-a".into(),
        content_binding: binding(commit),
        runtime_artifact_digest: commit.identity().runtime_artifact_digest.clone().into(),
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
    assert_eq!(first.identity().digest(), replacement.identity().digest());
    assert_ne!(first.commit_digest, replacement.commit_digest);
    assert_eq!(first.predecessor_commit_digest, None);
    assert_eq!(
        replacement.predecessor_commit_digest,
        Some(first.commit_digest)
    );
    assert_ne!(first.mutation_id, replacement.mutation_id);
    assert_ne!(first.lease_id, replacement.lease_id);
}

#[test]
fn same_policy_with_different_runtime_binary_is_rejected() {
    let commit = commit(identity('a'), None);
    let left = runtime_binding(&commit, 'a');
    let error = RuntimeExecutionBinding::new(RuntimeExecutionBindingInput {
        project_workspace: left.project_workspace.clone(),
        worktree_instance_id: left.worktree_instance_id.clone(),
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
fn project_workspace_worktree_and_publication_nonce_are_part_of_runtime_identity() {
    let commit = commit(identity('a'), None);
    let expected = runtime_binding(&commit, 'b');
    for observed in [
        RuntimeExecutionBinding {
            project_workspace: ProjectWorkspaceBinding::new(
                "git+https://github.com/tao3k/another-project.git#workspace/root",
                ".",
                "cross-machine",
                Vec::new(),
            )
            .expect("alternate Project Workspace"),
            ..expected.clone()
        },
        RuntimeExecutionBinding {
            project_workspace: ProjectWorkspaceBinding::new(
                "git+https://github.com/tao3k/agent-semantic-protocols.git#workspace/root",
                "nested",
                "cross-machine",
                Vec::new(),
            )
            .expect("alternate workspace root"),
            ..expected.clone()
        },
        RuntimeExecutionBinding {
            project_workspace: ProjectWorkspaceBinding::new(
                "git+https://github.com/tao3k/agent-semantic-protocols.git#workspace/root",
                ".",
                "cross-machine",
                vec!["git+ssh://github.com/tao3k/agent-semantic-protocols.git".to_owned()],
            )
            .expect("alternate repository aliases"),
            ..expected.clone()
        },
        RuntimeExecutionBinding {
            worktree_instance_id: "workspace-b".into(),
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
fn content_authority_stamp_is_part_of_runtime_identity() {
    let commit = commit(identity('a'), None);
    let expected = runtime_binding(&commit, 'b');
    let mut observed = expected.clone();
    observed.content_binding.authority_stamp.signature = "alternate-signature".into();

    observed
        .validate()
        .expect("alternate nonempty signature remains a valid test authority stamp");
    assert_eq!(
        expected.admits(&observed),
        Err(RuntimeExecutionBindingError::ContentMismatch)
    );
    assert_ne!(
        expected.digest(),
        observed.digest(),
        "the content-addressed Runtime product must include its complete authority stamp"
    );
}

#[test]
fn legacy_v1_binding_is_not_a_runtime_v2_execution_binding() {
    let commit = commit(identity('a'), None);
    let canonical = runtime_binding(&commit, 'b');
    let mut legacy = serde_json::to_value(&canonical).expect("encode canonical binding");
    let object = legacy
        .as_object_mut()
        .expect("runtime binding is a JSON object");
    object.insert(
        "schemaId".to_owned(),
        serde_json::json!("asp.runtime-execution-binding"),
    );
    object.insert("schemaVersion".to_owned(), serde_json::json!("1"));
    object.remove("projectWorkspace");
    object.remove("worktreeInstanceId");
    object.insert("projectId".to_owned(), serde_json::json!("project-a"));
    object.insert("workspaceId".to_owned(), serde_json::json!("workspace-a"));
    assert!(serde_json::from_value::<RuntimeExecutionBinding>(legacy).is_err());
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

#[test]
fn runtime_v2_binds_the_complete_project_workspace_product() {
    let project_workspace = ProjectWorkspaceBinding::new(
        "git+https://github.com/tao3k/agent-semantic-protocols.git#workspace/root",
        ".",
        "cross-machine",
        Vec::new(),
    )
    .expect("manifest-admitted Project Workspace");
    let commit = commit(identity('a'), None);
    let binding = RuntimeExecutionBinding::new(RuntimeExecutionBindingInput {
        project_workspace,
        worktree_instance_id: "worktree-a".into(),
        publication_nonce: "publication-a".into(),
        content_binding: binding(&commit),
        runtime_artifact_digest: commit.identity().runtime_artifact_digest.clone().into(),
        evaluator_policy_digest: format!("blake3-256:{}", "b".repeat(64)).into(),
        active_artifact_receipt_digest: format!("blake3-256:{}", "c".repeat(64)).into(),
        evaluator_abi_digest: format!("blake3-256:{}", "d".repeat(64)).into(),
    })
    .expect("complete Runtime execution binding");

    let encoded = serde_json::to_value(binding).expect("Runtime binding JSON");
    assert_eq!(encoded["schemaVersion"], "2");
    assert_eq!(encoded["projectWorkspace"]["workspaceRootPath"], ".");
    assert!(encoded.get("projectId").is_none());
    assert!(encoded.get("workspaceId").is_none());
}

fn execution_publication(
    commit: &ContentPublicationCommit,
    runtime: RuntimeExecutionBinding,
) -> RuntimeWorkspaceExecutionPublication {
    RuntimeWorkspaceExecutionPublication::new(RuntimeWorkspaceExecutionPublicationInput {
        workspace_identity: "workspace-main".into(),
        generation_digest: commit.identity().source_generation_digest.clone().into(),
        source_root_digest: commit.identity().workspace_snapshot_digest.clone().into(),
        content_publication_commit: commit.clone(),
        runtime_execution_binding: runtime,
        runtime_bundle_digest: format!("blake3-256:{}", "f".repeat(64)).into(),
    })
    .expect("source generation and Runtime product form one publication")
}

#[test]
fn workspace_execution_publication_binds_source_and_runtime_as_one_product() {
    let commit = commit(identity('a'), None);
    let publication = execution_publication(&commit, runtime_binding(&commit, 'b'));

    publication.validate().expect("valid execution publication");
    assert_eq!(
        publication.generation_digest.as_str(),
        commit.identity().source_generation_digest
    );
    assert_eq!(
        publication.source_root_digest.as_str(),
        commit.identity().workspace_snapshot_digest
    );
    assert_eq!(
        publication.publication_digest.as_str(),
        publication.digest()
    );
}

#[test]
fn runtime_refresh_reuses_source_generation_but_mints_a_new_publication() {
    let commit = commit(identity('a'), None);
    let first = execution_publication(&commit, runtime_binding(&commit, 'b'));
    let mut refreshed_runtime = runtime_binding(&commit, 'c');
    refreshed_runtime.publication_nonce = "publication-b".into();
    let refreshed = execution_publication(&commit, refreshed_runtime);

    assert_eq!(first.generation_digest, refreshed.generation_digest);
    assert_eq!(first.source_root_digest, refreshed.source_root_digest);
    assert_ne!(
        first.runtime_execution_binding,
        refreshed.runtime_execution_binding
    );
    assert_ne!(first.publication_digest, refreshed.publication_digest);
    assert_eq!(
        refreshed.admits(&first),
        Err(RuntimeWorkspaceExecutionPublicationError::ContentMismatch)
    );
}

#[test]
fn execution_publication_rejects_source_generation_drift() {
    let commit = commit(identity('a'), None);
    let runtime = runtime_binding(&commit, 'b');
    let error =
        RuntimeWorkspaceExecutionPublication::new(RuntimeWorkspaceExecutionPublicationInput {
            workspace_identity: "workspace-main".into(),
            generation_digest: format!("blake3-256:{}", "f".repeat(64)).into(),
            source_root_digest: commit.identity().workspace_snapshot_digest.clone().into(),
            content_publication_commit: commit.clone(),
            runtime_execution_binding: runtime,
            runtime_bundle_digest: format!("blake3-256:{}", "f".repeat(64)).into(),
        })
        .expect_err("a stale source generation must not compose with the Runtime binding");

    assert_eq!(
        error,
        RuntimeWorkspaceExecutionPublicationError::SourceGenerationMismatch
    );
}

#[test]
fn activation_generation_is_not_execution_publication_identity() {
    let commit = commit(identity('a'), None);
    let publication = execution_publication(&commit, runtime_binding(&commit, 'b'));
    let encoded = serde_json::to_value(publication).expect("publication JSON");

    assert!(encoded.get("activationGeneration").is_none());
}

#[test]
fn predecessor_fence_is_part_of_workspace_execution_publication_identity() {
    let first_commit = commit(identity('a'), None);
    let successor_commit = commit(identity('a'), Some(first_commit.commit_digest.as_str()));
    let first = execution_publication(&first_commit, runtime_binding(&first_commit, 'b'));
    let successor =
        execution_publication(&successor_commit, runtime_binding(&successor_commit, 'b'));

    assert_eq!(
        first.runtime_execution_binding,
        successor.runtime_execution_binding
    );
    assert_ne!(
        first.content_publication_commit,
        successor.content_publication_commit
    );
    assert_ne!(first.publication_digest, successor.publication_digest);
    assert_eq!(
        successor.admits(&first),
        Err(RuntimeWorkspaceExecutionPublicationError::ContentMismatch)
    );
}

#[test]
fn execution_publication_rejects_a_commit_from_another_content_authority() {
    let admitted_commit = commit(identity('a'), None);
    let foreign_identity = admitted_commit.identity().clone();
    let foreign_commit = ContentPublicationCommit::linearize(
        foreign_identity.clone(),
        AuthorityStamp {
            key_id: "foreign-authority".into(),
            canonical_digest: foreign_identity.digest(),
            signature: "foreign-signature".into(),
        },
    )
    .expect("foreign authority can mint its own internally valid commit");
    let runtime = runtime_binding(&admitted_commit, 'b');

    let error =
        RuntimeWorkspaceExecutionPublication::new(RuntimeWorkspaceExecutionPublicationInput {
            workspace_identity: "workspace-main".into(),
            generation_digest: admitted_commit
                .identity()
                .source_generation_digest
                .clone()
                .into(),
            source_root_digest: admitted_commit
                .identity()
                .workspace_snapshot_digest
                .clone()
                .into(),
            content_publication_commit: foreign_commit,
            runtime_execution_binding: runtime,
            runtime_bundle_digest: format!("blake3-256:{}", "f".repeat(64)).into(),
        })
        .expect_err("the durable commit and Runtime product must share one exact binding");

    assert_eq!(
        error,
        RuntimeWorkspaceExecutionPublicationError::ContentCommitMismatch
    );
}

#[test]
fn execution_pointer_admits_only_its_exact_sidecar() {
    let commit = commit(identity('a'), None);
    let publication = execution_publication(&commit, runtime_binding(&commit, 'b'));
    let pointer = RuntimeWorkspaceExecutionPointer::from_publication(&publication)
        .expect("valid canonical pointer");

    pointer
        .admits(&publication)
        .expect("pointer admits its exact sidecar");

    let mut refreshed_runtime = runtime_binding(&commit, 'c');
    refreshed_runtime.publication_nonce = "publication-b".into();
    let refreshed = execution_publication(&commit, refreshed_runtime);
    assert_eq!(
        pointer.admits(&refreshed),
        Err(RuntimeWorkspaceExecutionPointerError::PublicationMismatch)
    );
}

#[test]
fn activation_generation_is_not_execution_pointer_identity() {
    let commit = commit(identity('a'), None);
    let publication = execution_publication(&commit, runtime_binding(&commit, 'b'));
    let pointer = RuntimeWorkspaceExecutionPointer::from_publication(&publication)
        .expect("valid canonical pointer");
    let encoded = serde_json::to_value(pointer).expect("pointer JSON");

    assert!(encoded.get("activationGeneration").is_none());
}
