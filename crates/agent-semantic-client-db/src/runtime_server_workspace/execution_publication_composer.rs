// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Single composition boundary for the canonical source/Runtime product.

use std::path::Path;

use agent_semantic_artifacts::blake3_content_digest::Blake3ContentDigest;
use agent_semantic_artifacts::runtime_artifact_activation::RuntimeArtifactActivationEvent;
use agent_semantic_artifacts::runtime_artifact_slots::RuntimeArtifactBundleBinding;
use agent_semantic_content_identity::HostWorkspaceInitializationBinding;
use agent_semantic_content_identity::content_binding::{
    AuthorityStamp, ContentIdentity, ContentPublicationCommit,
};
use agent_semantic_content_identity::runtime_execution::{
    RuntimeExecutionBinding, RuntimeExecutionBindingInput,
};
use agent_semantic_content_identity::runtime_workspace_execution_publication::{
    RuntimeWorkspaceExecutionPublication, RuntimeWorkspaceExecutionPublicationInput,
};

use super::{RuntimeWorkspaceExecutionPublicationStore, WorkspaceGenerationPointerReader};

const COMPLETE_GENERATION_AUTHORITY_KEY: &str = "runtime-server-complete-generation";

/// Verify the already durable source pointer, linearize its complete content
/// commit, then publish the immutable execution sidecar and joined pointer.
///
/// This is deliberately outside the Query path. Callers cannot provide source
/// digests directly: the canonical source pointer is reopened and validated at
/// the commit boundary.
pub async fn publish_runtime_workspace_execution_product(
    source_pointer_path: &Path,
    execution_root: &Path,
    host_workspace: HostWorkspaceInitializationBinding,
    activation: &RuntimeArtifactActivationEvent,
    runtime_bundle_digest: &Blake3ContentDigest,
    bundle: &RuntimeArtifactBundleBinding,
) -> Result<RuntimeWorkspaceExecutionPublication, String> {
    let source_reader = WorkspaceGenerationPointerReader::open(source_pointer_path).await?;
    let source = source_reader.read()?;
    source.validate()?;
    activation.validate_identity()?;
    bundle.validate()?;
    if &activation.bundle_digest != runtime_bundle_digest {
        return Err(format!(
            "reasonKind=runtime-workspace-execution-binding-mismatch field=runtimeBundleDigest expected={} actual={}",
            activation.bundle_digest, runtime_bundle_digest,
        ));
    }
    host_workspace
        .validate()
        .map_err(|error| error.to_string())?;

    let identity = ContentIdentity {
        runtime_artifact_digest: activation.artifact_digest.as_str().to_owned(),
        workspace_snapshot_digest: source.source_root_digest.clone(),
        source_generation_digest: source.generation_digest.clone(),
        source_index_digest: source.durable_commit_digest.clone(),
        schema_digest: bundle.schema_bundle_digest().as_str().to_owned(),
        provider_catalog_digest: bundle.provider_catalog_digest().as_str().to_owned(),
    };
    identity
        .validate()
        .map_err(|error| format!("validate complete content identity: {error:?}"))?;
    let authority_stamp = complete_generation_authority_stamp(
        &identity,
        &source.durable_commit_digest,
        activation,
        runtime_bundle_digest,
        bundle,
    );
    let previous =
        RuntimeWorkspaceExecutionPublicationStore::read_active_optional(execution_root).await?;
    let content_publication_commit = if let Some(previous) = previous.as_ref()
        && previous.content_publication_commit.identity() == &identity
        && previous.content_publication_commit.authority_stamp() == &authority_stamp
    {
        previous.content_publication_commit.clone()
    } else {
        ContentPublicationCommit::linearize_with_expected(
            identity,
            authority_stamp,
            previous.as_ref().map(|publication| {
                publication
                    .content_publication_commit
                    .commit_digest
                    .as_str()
            }),
        )
        .map_err(|error| format!("linearize complete content publication: {error:?}"))?
    };
    let publication = compose_runtime_workspace_execution_publication(
        source.workspace_identity,
        source.generation_digest,
        source.source_root_digest,
        host_workspace,
        content_publication_commit,
        activation,
        runtime_bundle_digest,
        bundle,
    )?;
    if previous.as_ref() == Some(&publication) {
        return Ok(publication);
    }
    let store = RuntimeWorkspaceExecutionPublicationStore::open(execution_root).await?;
    store.publish(&publication).await?;
    Ok(publication)
}

fn complete_generation_authority_stamp(
    identity: &ContentIdentity,
    durable_source_commit_digest: &str,
    activation: &RuntimeArtifactActivationEvent,
    runtime_bundle_digest: &Blake3ContentDigest,
    bundle: &RuntimeArtifactBundleBinding,
) -> AuthorityStamp {
    let mut hasher = blake3::Hasher::new();
    hasher.update(b"agent.semantic-protocols.complete-generation-authority.v1\0");
    for field in [
        identity.digest(),
        durable_source_commit_digest.to_owned(),
        activation.content_digest().as_str().to_owned(),
        runtime_bundle_digest.as_str().to_owned(),
        bundle.provider_registration_digest().as_str().to_owned(),
        bundle.provider_artifact_set_digest().as_str().to_owned(),
        bundle.evaluator_policy_digest().as_str().to_owned(),
        bundle.evaluator_abi_digest().as_str().to_owned(),
        bundle.schema_bundle_digest().as_str().to_owned(),
    ] {
        hasher.update(&(field.len() as u64).to_le_bytes());
        hasher.update(field.as_bytes());
    }
    AuthorityStamp {
        key_id: COMPLETE_GENERATION_AUTHORITY_KEY.to_owned(),
        canonical_digest: identity.digest(),
        signature: format!("blake3-256:{}", hasher.finalize().to_hex()),
    }
}

/// Compose the only Query-admissible execution product from independently
/// admitted source, workspace, activation, and bound-bundle authorities.
#[allow(clippy::too_many_arguments)]
pub fn compose_runtime_workspace_execution_publication(
    workspace_identity: String,
    generation_digest: String,
    source_root_digest: String,
    host_workspace: HostWorkspaceInitializationBinding,
    content_publication_commit: ContentPublicationCommit,
    activation: &RuntimeArtifactActivationEvent,
    runtime_bundle_digest: &Blake3ContentDigest,
    bundle: &RuntimeArtifactBundleBinding,
) -> Result<RuntimeWorkspaceExecutionPublication, String> {
    activation.validate_identity()?;
    bundle.validate()?;
    if &activation.bundle_digest != runtime_bundle_digest {
        return Err(format!(
            "reasonKind=runtime-workspace-execution-binding-mismatch field=runtimeBundleDigest expected={} actual={}",
            activation.bundle_digest, runtime_bundle_digest,
        ));
    }
    host_workspace
        .validate()
        .map_err(|error| error.to_string())?;
    content_publication_commit
        .validate()
        .map_err(|error| format!("validate content publication commit: {error:?}"))?;

    let identity = content_publication_commit.identity();
    let provider_catalog_digest = bundle.provider_catalog_digest();
    for (field, expected, actual) in [
        (
            "runtimeArtifactDigest",
            activation.artifact_digest.as_str(),
            identity.runtime_artifact_digest.as_str(),
        ),
        (
            "workspaceSnapshotDigest",
            source_root_digest.as_str(),
            identity.workspace_snapshot_digest.as_str(),
        ),
        (
            "sourceGenerationDigest",
            generation_digest.as_str(),
            identity.source_generation_digest.as_str(),
        ),
        (
            "schemaDigest",
            bundle.schema_bundle_digest().as_str(),
            identity.schema_digest.as_str(),
        ),
        (
            "providerCatalogDigest",
            provider_catalog_digest.as_str(),
            identity.provider_catalog_digest.as_str(),
        ),
    ] {
        if expected != actual {
            return Err(format!(
                "reasonKind=runtime-workspace-execution-binding-mismatch field={field} expected={expected} actual={actual}"
            ));
        }
    }

    let runtime_execution_binding = RuntimeExecutionBinding::new(RuntimeExecutionBindingInput {
        project_workspace: host_workspace.project_workspace().clone(),
        worktree_instance_id: host_workspace.worktree_instance_id().to_owned(),
        publication_nonce: activation.publication_nonce.clone(),
        content_binding: content_publication_commit.content_binding.clone(),
        runtime_artifact_digest: activation.artifact_digest.as_str().to_owned().into(),
        evaluator_policy_digest: bundle.evaluator_policy_digest().as_str().to_owned().into(),
        active_artifact_receipt_digest: activation.content_digest().as_str().to_owned().into(),
        evaluator_abi_digest: bundle.evaluator_abi_digest().as_str().to_owned().into(),
    })
    .map_err(|error| format!("compose Runtime execution binding: {error:?}"))?;

    RuntimeWorkspaceExecutionPublication::new(RuntimeWorkspaceExecutionPublicationInput {
        workspace_identity,
        generation_digest: generation_digest.into(),
        source_root_digest: source_root_digest.into(),
        content_publication_commit,
        runtime_execution_binding,
        runtime_bundle_digest: runtime_bundle_digest.as_str().to_owned().into(),
    })
    .map_err(|error| format!("compose Runtime workspace execution publication: {error:?}"))
}
