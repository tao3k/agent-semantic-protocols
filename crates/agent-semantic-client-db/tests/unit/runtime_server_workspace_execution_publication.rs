// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

use agent_semantic_artifacts::blake3_content_digest::Blake3ContentDigest;
use agent_semantic_artifacts::runtime_artifact_activation::{
    RuntimeArtifactActivationEvent, RuntimeArtifactCandidateIdentityReceipt,
};
use agent_semantic_artifacts::runtime_artifact_slots::RuntimeArtifactBundleBinding;
use agent_semantic_client_db::runtime_server_opentelemetry::RuntimeSearchTelemetryIdentity;
use agent_semantic_client_db::runtime_server_workspace::RuntimeWorkspaceExecutionPublicationStore;
use agent_semantic_client_db::runtime_server_workspace::compose_runtime_workspace_execution_publication;
use agent_semantic_client_protocol::RuntimeSearchClientTimingWitness;
use agent_semantic_content_identity::content_binding::{
    AuthorityStamp, ContentIdentity, ContentPublicationCommit,
};
use agent_semantic_content_identity::runtime_execution::{
    RuntimeExecutionBinding, RuntimeExecutionBindingInput,
};
use agent_semantic_content_identity::runtime_workspace_execution_publication::{
    RuntimeWorkspaceExecutionPublication, RuntimeWorkspaceExecutionPublicationInput,
};
use agent_semantic_content_identity::{
    HostWorkspaceInitializationBinding, ProjectWorkspaceBinding,
};

fn digest(byte: char) -> String {
    format!(
        "blake3-256:{}",
        std::iter::repeat_n(byte, 64).collect::<String>()
    )
}

fn strong_digest(byte: char) -> Blake3ContentDigest {
    Blake3ContentDigest::parse(&digest(byte)).expect("canonical fixture digest")
}

fn bundle_binding(policy: char) -> RuntimeArtifactBundleBinding {
    RuntimeArtifactBundleBinding::new(
        strong_digest('a'),
        strong_digest('b'),
        strong_digest(policy),
        strong_digest('d'),
        strong_digest('5'),
    )
}

fn activation(artifact: char) -> RuntimeArtifactActivationEvent {
    let artifact_digest = strong_digest(artifact);
    RuntimeArtifactActivationEvent {
        schema_id: "agent.semantic-protocols.runtime-artifact-activation".into(),
        schema_version: 1,
        activation_generation: 42,
        bundle_digest: strong_digest('e'),
        artifact_digest: artifact_digest.clone(),
        artifact_path: "/runtime/asp".into(),
        candidate_slot_path: "/runtime/candidate".into(),
        previous_artifact_digest: None,
        artifact_mode: "release".into(),
        published_at_unix_millis: 123,
        publication_nonce: "publication-1".into(),
        candidate_identity: RuntimeArtifactCandidateIdentityReceipt {
            artifact_digest,
            artifact_path: "/runtime/asp".into(),
            stable_path: "/runtime/stable/asp".into(),
            artifact_mode: "release".into(),
            publication_nonce: "publication-1".into(),
        },
    }
}

fn project_workspace() -> ProjectWorkspaceBinding {
    ProjectWorkspaceBinding::new(
        "git+https://github.com/tao3k/agent-semantic-protocols.git#workspace/main",
        ".",
        "cross-machine",
        Vec::new(),
    )
    .expect("valid Project Workspace")
}

fn host_workspace() -> HostWorkspaceInitializationBinding {
    HostWorkspaceInitializationBinding::new(project_workspace(), "worktree-main")
        .expect("valid Host workspace binding")
}

fn committed_identity(bundle: &RuntimeArtifactBundleBinding) -> ContentPublicationCommit {
    let identity = ContentIdentity {
        runtime_artifact_digest: digest('1'),
        workspace_snapshot_digest: digest('2'),
        source_generation_digest: digest('3'),
        source_index_digest: digest('4'),
        schema_digest: bundle.schema_bundle_digest().as_str().to_owned(),
        provider_catalog_digest: bundle.provider_catalog_digest().as_str().to_owned(),
    };
    ContentPublicationCommit::linearize(
        identity.clone(),
        AuthorityStamp {
            key_id: "runtime-owner".into(),
            canonical_digest: identity.digest(),
            signature: "signature-1".into(),
        },
    )
    .expect("valid commit")
}

fn publication() -> RuntimeWorkspaceExecutionPublication {
    let identity = ContentIdentity {
        runtime_artifact_digest: digest('1'),
        workspace_snapshot_digest: digest('2'),
        source_generation_digest: digest('3'),
        source_index_digest: digest('4'),
        schema_digest: digest('5'),
        provider_catalog_digest: digest('6'),
    };
    let commit = ContentPublicationCommit::linearize(
        identity.clone(),
        AuthorityStamp {
            key_id: "runtime-owner".into(),
            canonical_digest: identity.digest(),
            signature: "signature-1".into(),
        },
    )
    .expect("valid commit");
    let runtime_execution_binding = RuntimeExecutionBinding::new(RuntimeExecutionBindingInput {
        project_workspace: ProjectWorkspaceBinding::new(
            "git+https://github.com/tao3k/agent-semantic-protocols.git#workspace/main",
            ".",
            "cross-machine",
            Vec::new(),
        )
        .expect("valid Project Workspace"),
        worktree_instance_id: "worktree-main".into(),
        publication_nonce: "publication-1".into(),
        content_binding: commit.content_binding.clone(),
        runtime_artifact_digest: digest('1').into(),
        evaluator_policy_digest: digest('7').into(),
        active_artifact_receipt_digest: digest('8').into(),
        evaluator_abi_digest: digest('9').into(),
    })
    .expect("valid Runtime binding");
    RuntimeWorkspaceExecutionPublication::new(RuntimeWorkspaceExecutionPublicationInput {
        workspace_identity: "workspace-main".into(),
        generation_digest: digest('3').into(),
        source_root_digest: digest('2').into(),
        content_publication_commit: commit,
        runtime_execution_binding,
        runtime_bundle_digest: digest('f').into(),
    })
    .expect("valid workspace execution publication")
}

#[test]
fn telemetry_identity_is_projected_only_from_the_validated_execution_publication() {
    let publication = publication();
    let identity = RuntimeSearchTelemetryIdentity::from_execution_publication(
        &publication,
        "session-1",
        "request-1",
        vec!["rust".into()],
        vec!["asp-rust".into()],
    )
    .expect("validated publication must project one exact telemetry identity");

    let content = &publication
        .runtime_execution_binding
        .content_binding
        .identity;
    assert_eq!(identity.workspace_identity, publication.workspace_identity);
    assert_eq!(
        identity.workspace_snapshot_digest,
        content.workspace_snapshot_digest
    );
    assert_eq!(
        identity.source_snapshot_digest,
        publication.source_root_digest.as_str()
    );
    assert_eq!(
        identity.source_generation_digest,
        content.source_generation_digest
    );
    assert_eq!(identity.source_index_digest, content.source_index_digest);
    assert_eq!(
        identity.runtime_artifact_digest,
        content.runtime_artifact_digest
    );
    assert_eq!(
        identity.runtime_bundle_digest,
        publication.runtime_bundle_digest.as_str()
    );
    assert_eq!(
        identity.execution_publication_digest,
        publication.publication_digest.as_str()
    );
    assert_eq!(
        identity.provider_catalog_digest,
        content.provider_catalog_digest
    );
}

#[test]
fn client_timings_cannot_be_enriched_under_a_foreign_request() {
    let publication = publication();
    let witness = RuntimeSearchClientTimingWitness::new("session-1", "request-1", [1, 2, 3])
        .expect("canonical client timing witness");
    let error = RuntimeSearchTelemetryIdentity::settle_client_timing(
        &publication,
        &witness,
        "session-1",
        "request-2",
        vec!["rust".into()],
        vec!["asp-rust".into()],
    )
    .expect_err("foreign request timing must not receive Runtime identity");
    assert_eq!(
        error.reason_kind(),
        "runtime-search-client-timing-identity-mismatch"
    );
}

#[tokio::test]
async fn canonical_store_publishes_sidecar_before_pointer_and_recovers_exact_product() {
    let temporary = tempfile::tempdir().expect("temporary workspace publication root");
    let store = RuntimeWorkspaceExecutionPublicationStore::open(temporary.path())
        .await
        .expect("open canonical publication store");
    let publication = publication();

    let pointer = store
        .publish(&publication)
        .await
        .expect("durably publish sidecar and pointer");
    let recovered = RuntimeWorkspaceExecutionPublicationStore::read_active(temporary.path())
        .await
        .expect("recover the exact pointer-bound publication");

    pointer
        .admits(&publication)
        .expect("pointer admits publication");
    assert_eq!(recovered, publication);
}

#[tokio::test]
async fn missing_sidecar_never_recovers_as_an_active_execution_product() {
    let temporary = tempfile::tempdir().expect("temporary workspace publication root");
    let store = RuntimeWorkspaceExecutionPublicationStore::open(temporary.path())
        .await
        .expect("open canonical publication store");
    let publication = publication();
    store.publish(&publication).await.expect("publish fixture");
    tokio::fs::remove_file(store.sidecar_path(&publication))
        .await
        .expect("remove only the temporary fixture sidecar");

    let error = RuntimeWorkspaceExecutionPublicationStore::read_active(temporary.path())
        .await
        .expect_err("a pointer cannot manufacture a missing durable sidecar");
    assert!(error.contains("execution publication sidecar"));
}

#[tokio::test]
async fn invalid_sidecar_is_rejected_before_the_active_pointer_exists() {
    let temporary = tempfile::tempdir().expect("temporary workspace publication root");
    let store = RuntimeWorkspaceExecutionPublicationStore::open(temporary.path())
        .await
        .expect("open canonical publication store");
    let mut publication = publication();
    publication.publication_digest = digest('f').into();

    let error = store
        .publish(&publication)
        .await
        .expect_err("invalid sidecar must fail before pointer publication");
    assert!(error.contains("PublicationDigestMismatch"));
    let recovery_error = RuntimeWorkspaceExecutionPublicationStore::read_active(temporary.path())
        .await
        .expect_err("failed publication must leave no active execution product");
    assert!(recovery_error.contains("runtime workspace execution pointer"));
}

#[test]
fn composer_requires_one_exact_source_activation_and_bundle_product() {
    let bundle = bundle_binding('c');
    let commit = committed_identity(&bundle);
    let activation_receipt = activation('1');
    let publication = compose_runtime_workspace_execution_publication(
        "workspace-main".into(),
        digest('3'),
        digest('2'),
        host_workspace(),
        commit.clone(),
        &activation_receipt,
        &activation_receipt.bundle_digest,
        &bundle,
    )
    .expect("exact authorities compose");
    assert_eq!(
        publication
            .runtime_execution_binding
            .active_artifact_receipt_digest
            .as_str(),
        activation_receipt.content_digest().as_str()
    );
    assert_eq!(
        publication
            .runtime_execution_binding
            .evaluator_policy_digest
            .as_str(),
        bundle.evaluator_policy_digest().as_str()
    );
    assert_eq!(
        publication.runtime_bundle_digest.as_str(),
        activation_receipt.bundle_digest.as_str()
    );

    let artifact_error = compose_runtime_workspace_execution_publication(
        "workspace-main".into(),
        digest('3'),
        digest('2'),
        host_workspace(),
        commit.clone(),
        &activation('9'),
        &activation_receipt.bundle_digest,
        &bundle,
    )
    .expect_err("a foreign Runtime artifact must not join the source commit");
    assert!(artifact_error.contains("field=runtimeArtifactDigest"));

    let foreign_catalog = RuntimeArtifactBundleBinding::new(
        strong_digest('f'),
        strong_digest('b'),
        strong_digest('c'),
        strong_digest('d'),
        strong_digest('5'),
    );
    let catalog_error = compose_runtime_workspace_execution_publication(
        "workspace-main".into(),
        digest('3'),
        digest('2'),
        host_workspace(),
        commit,
        &activation_receipt,
        &activation_receipt.bundle_digest,
        &foreign_catalog,
    )
    .expect_err("a foreign provider catalog must not join the source commit");
    assert!(catalog_error.contains("field=providerCatalogDigest"));

    let bundle_digest_error = compose_runtime_workspace_execution_publication(
        "workspace-main".into(),
        digest('3'),
        digest('2'),
        host_workspace(),
        committed_identity(&bundle),
        &activation_receipt,
        &strong_digest('f'),
        &bundle,
    )
    .expect_err("activation cannot authorize another Runtime bundle");
    assert!(bundle_digest_error.contains("field=runtimeBundleDigest"));
}

#[test]
fn evaluator_policy_refresh_mints_a_distinct_execution_publication() {
    let old_bundle = bundle_binding('c');
    let new_bundle = bundle_binding('f');
    let commit = committed_identity(&old_bundle);
    let activation_receipt = activation('1');
    let mut refreshed_activation = activation('1');
    refreshed_activation.bundle_digest = strong_digest('f');
    refreshed_activation.publication_nonce = "publication-2".into();
    refreshed_activation.candidate_identity.publication_nonce = "publication-2".into();
    let old = compose_runtime_workspace_execution_publication(
        "workspace-main".into(),
        digest('3'),
        digest('2'),
        host_workspace(),
        commit.clone(),
        &activation_receipt,
        &activation_receipt.bundle_digest,
        &old_bundle,
    )
    .expect("old publication");
    let new = compose_runtime_workspace_execution_publication(
        "workspace-main".into(),
        digest('3'),
        digest('2'),
        host_workspace(),
        commit,
        &refreshed_activation,
        &refreshed_activation.bundle_digest,
        &new_bundle,
    )
    .expect("policy-only refresh keeps the same source and provider catalog");
    assert_ne!(old.publication_digest, new.publication_digest);
    assert_ne!(old.runtime_bundle_digest, new.runtime_bundle_digest);
    assert_ne!(
        old.runtime_execution_binding.evaluator_policy_digest,
        new.runtime_execution_binding.evaluator_policy_digest
    );
}
