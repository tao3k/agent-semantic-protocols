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

/// Verify the already durable source pointer, linearize its complete logical
/// content commit, then publish the immutable execution sidecar and joined
/// pointer.
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

    let source_binding = source
        .runtime_provider_execution_binding
        .as_ref()
        .ok_or_else(|| {
            "Runtime workspace execution publication requires a provider execution binding"
                .to_owned()
        })?;
    validate_logical_source_binding(
        source_binding,
        &source.workspace_identity,
        &source.source_root_digest,
        &source.module_graph_digest,
        runtime_bundle_digest,
        bundle,
    )?;
    let identity = ContentIdentity {
        runtime_artifact_digest: activation.artifact_digest.as_str().to_owned(),
        workspace_snapshot_digest: source.source_root_digest.clone(),
        source_generation_digest: source.generation_digest.clone(),
        source_index_digest: source_binding.source_index_digest.clone(),
        schema_digest: bundle.schema_bundle_digest().as_str().to_owned(),
        provider_catalog_digest: bundle.provider_catalog_digest().as_str().to_owned(),
    };
    identity
        .validate()
        .map_err(|error| format!("validate complete content identity: {error:?}"))?;
    let authority_stamp =
        complete_generation_authority_stamp(&identity, activation, runtime_bundle_digest, bundle);
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

/// Compose the Query-admissible execution product directly from an already
/// validated process-resident generation.  Physical segment durability is a
/// restart attachment and is intentionally absent from this logical identity.
pub fn compose_runtime_workspace_execution_product_from_resident(
    resident: &crate::runtime_resident_read::RuntimeResidentReadClient,
    host_workspace: HostWorkspaceInitializationBinding,
    activation: &RuntimeArtifactActivationEvent,
    runtime_bundle_digest: &Blake3ContentDigest,
    bundle: &RuntimeArtifactBundleBinding,
) -> Result<RuntimeWorkspaceExecutionPublication, String> {
    let source = resident.search_generation_authority();
    source.validate_binding(&source.project_id, &source.workspace_id)?;
    let source_binding = source
        .runtime_provider_execution_binding
        .as_ref()
        .ok_or_else(|| {
            "Runtime resident execution publication requires a provider execution binding"
                .to_owned()
        })?;
    let canonical_source_root =
        agent_semantic_search::canonical_blake3_digest(&source.source_snapshot.root_digest)?;
    validate_logical_source_binding(
        source_binding,
        &source.workspace_id,
        &canonical_source_root,
        &source_binding.source_index_digest,
        runtime_bundle_digest,
        bundle,
    )?;
    activation.validate_identity()?;
    if &activation.bundle_digest != runtime_bundle_digest {
        return Err(format!(
            "reasonKind=runtime-workspace-execution-binding-mismatch field=runtimeBundleDigest expected={} actual={}",
            activation.bundle_digest, runtime_bundle_digest,
        ));
    }
    let identity = ContentIdentity {
        runtime_artifact_digest: activation.artifact_digest.as_str().to_owned(),
        workspace_snapshot_digest: canonical_source_root.clone(),
        source_generation_digest: source.generation_digest.clone(),
        source_index_digest: source_binding.source_index_digest.clone(),
        schema_digest: bundle.schema_bundle_digest().as_str().to_owned(),
        provider_catalog_digest: bundle.provider_catalog_digest().as_str().to_owned(),
    };
    identity
        .validate()
        .map_err(|error| format!("validate resident complete content identity: {error:?}"))?;
    let authority_stamp =
        complete_generation_authority_stamp(&identity, activation, runtime_bundle_digest, bundle);
    let commit = ContentPublicationCommit::linearize(identity, authority_stamp)
        .map_err(|error| format!("linearize resident content publication: {error:?}"))?;
    compose_runtime_workspace_execution_publication(
        source.workspace_id.clone(),
        source.generation_digest.clone(),
        canonical_source_root,
        host_workspace,
        commit,
        activation,
        runtime_bundle_digest,
        bundle,
    )
}

fn validate_logical_source_binding(
    binding: &agent_semantic_artifacts::runtime_provider_execution_binding::RuntimeProviderExecutionBinding,
    workspace_identity: &str,
    source_root_digest: &str,
    source_index_digest: &str,
    runtime_bundle_digest: &Blake3ContentDigest,
    bundle: &RuntimeArtifactBundleBinding,
) -> Result<(), String> {
    binding.validate()?;
    for (field, expected, actual) in [
        (
            "workspaceId",
            workspace_identity,
            binding.workspace_id.as_str(),
        ),
        (
            "sourceSnapshotDigest",
            source_root_digest,
            binding.source_snapshot_digest.as_str(),
        ),
        (
            "sourceIndexDigest",
            source_index_digest,
            binding.source_index_digest.as_str(),
        ),
        (
            "runtimeBundleDigest",
            runtime_bundle_digest.as_str(),
            binding.runtime_bundle_digest.as_str(),
        ),
        (
            "schemaBundleDigest",
            bundle.schema_bundle_digest().as_str(),
            binding.schema_bundle_digest.as_str(),
        ),
    ] {
        if !same_content_digest(expected, actual) {
            return Err(format!(
                "reasonKind=runtime-workspace-execution-binding-mismatch field={field} expected={expected} actual={actual}"
            ));
        }
    }
    Ok(())
}

fn same_content_digest(left: &str, right: &str) -> bool {
    left == right
        || left
            .strip_prefix("blake3-256:")
            .is_some_and(|digest| digest == right)
        || right
            .strip_prefix("blake3-256:")
            .is_some_and(|digest| digest == left)
}

fn complete_generation_authority_stamp(
    identity: &ContentIdentity,
    activation: &RuntimeArtifactActivationEvent,
    runtime_bundle_digest: &Blake3ContentDigest,
    bundle: &RuntimeArtifactBundleBinding,
) -> AuthorityStamp {
    let mut hasher = blake3::Hasher::new();
    hasher.update(b"agent.semantic-protocols.complete-generation-authority.v1\0");
    for field in [
        identity.digest(),
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
