// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Public refresh API for the DB Engine source index.

use std::path::{Path, PathBuf};
use std::time::Instant;

use agent_semantic_client_core::{
    ProjectContext, RuntimeProviderProjection, RuntimeProviderProjectionEvidence,
};

use crate::server_source_index::model::SourceIndexScopeFile;
use crate::server_source_index::provider_envelope::{
    ProviderSourceEnvelopeLookupRequestV1,
    current_provider_source_index_snapshot_at_artifact_root_with_registry,
};

pub(super) fn provider_scope_digest(
    registry: &RuntimeProviderProjectionEvidence,
    files: &[SourceIndexScopeFile],
) -> String {
    let mut scope = files
        .iter()
        .map(|file| {
            format!(
                "{}\u{1f}{}\u{1f}{}",
                file.path.display(),
                file.language_id,
                file.provider_id
            )
        })
        .collect::<Vec<_>>();
    scope.sort();
    scope.dedup();
    agent_semantic_artifacts::provider_digest(
        format!(
            "registry={}\u{1e}scope={}",
            registry.fingerprint,
            scope.join("\u{1e}")
        )
        .as_bytes(),
    )
}

/// One content-authoritative view of the live workspace for all source
/// acquisition paths in a request.
pub struct CurrentSourceIndexSnapshot {
    pub workspace_snapshot: agent_semantic_artifacts::WorkspaceSnapshot,
    pub source_snapshot: agent_semantic_content_identity::SourceSnapshotEvidence,
    /// Complete generation identity validated once when the snapshot is
    /// published or loaded.
    pub workspace_generation:
        agent_semantic_content_identity::workspace_generation_evidence::ValidatedWorkspaceGenerationV1,
    /// Owner bytes captured in the same read pass that produced the Merkle root.
    pub source_blobs: crate::ClientDbSourceIndexSourceBlobs,
}

pub fn current_provider_source_index_snapshot_with_registry(
    project_root: &Path,
    language_id: &agent_semantic_client_core::LanguageId,
    provider_id: &agent_semantic_client_core::ProviderId,
    provider_registry: &RuntimeProviderProjection,
) -> Result<CurrentSourceIndexSnapshot, String> {
    let project_context = ProjectContext::resolve(project_root)?;
    current_provider_source_index_snapshot_at_artifact_root_with_registry(
        ProviderSourceEnvelopeLookupRequestV1 {
            project_root,
            artifact_root: project_context.state_layout().artifacts_dir(),
            language_id,
            provider_id,
            provider_registry,
        },
    )
}

/// Capture the current worktree snapshot for exactly one registered provider.
///
/// This is the rootDepth=0 query boundary. It performs no envelope
/// publication, CAS write, database bootstrap, or provider admission mutation.
#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct PublishedProviderSourceEnvelope {
    schema_id: String,
    schema_version: String,
    provider_id: String,
    address_provider_digest: String,
    provider_workspace_root: String,
    provider_workspace_identity_digest: String,
    source_snapshot: agent_semantic_content_identity::SourceSnapshotEvidence,
    materialization_state: String,
    owner_coverage: String,
    cas_root: PathBuf,
    owners: Vec<PublishedProviderSourceOwner>,
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct PublishedProviderSourceOwner {
    path: String,
    snapshot_leaf_digest: String,
    blob_digest: String,
    source_content_digest: String,
    cas_path: String,
}

pub(super) fn load_provider_source_index_snapshot_envelope(
    artifact_root: &Path,
    requested_provider_id: &str,
    expected_address_provider_digest: &str,
    expected_provider_workspace_identity: &crate::server_source_index::provider_envelope::ProviderWorkspaceIdentityV1,
    envelope_path: &Path,
) -> Result<CurrentSourceIndexSnapshot, String> {
    let bytes = std::fs::read(envelope_path).map_err(|error| {
        format!(
            "failed to read requested provider source envelope {}: {error}",
            envelope_path.display()
        )
    })?;
    let envelope: PublishedProviderSourceEnvelope =
        serde_json::from_slice(&bytes).map_err(|error| {
            format!(
                "invalid requested provider source envelope {}: {error}",
                envelope_path.display()
            )
        })?;
    let expected_file_name =
        crate::server_source_index::provider_envelope::source_snapshot_envelope_file_name(
            requested_provider_id,
            expected_address_provider_digest,
            &expected_provider_workspace_identity.digest,
        );
    let invalid_reason = if envelope.schema_id != "asp.exact-source-snapshot-envelope.v1" {
        Some("schema-id")
    } else if envelope.schema_version != "1" {
        Some("schema-version")
    } else if envelope.provider_id != requested_provider_id {
        Some("provider-id")
    } else if envelope.address_provider_digest != expected_address_provider_digest {
        Some("address-provider-digest")
    } else if envelope.provider_workspace_root != expected_provider_workspace_identity.root {
        Some("provider-workspace-root")
    } else if envelope.provider_workspace_identity_digest
        != expected_provider_workspace_identity.digest
    {
        Some("provider-workspace-identity-digest")
    } else if envelope_path.file_name().and_then(|name| name.to_str())
        != Some(expected_file_name.as_str())
    {
        Some("address-provider-digest-filename")
    } else if envelope.source_snapshot.provider_digest.len() != 64
        || !envelope
            .source_snapshot
            .provider_digest
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit())
    {
        Some("source-provider-digest")
    } else if envelope.materialization_state != "artifact-complete" {
        Some("materialization-state")
    } else if envelope.owner_coverage != "complete" {
        Some("owner-coverage")
    } else if envelope.owners.is_empty() {
        Some("owners-empty")
    } else {
        None
    };
    if let Some(reason) = invalid_reason {
        return Err(provider_envelope_contract_error(
            requested_provider_id,
            reason,
        ));
    }
    let cas_root = artifact_root.join("source-blob-cas").join("v1");
    if envelope.cas_root != cas_root {
        return Err(provider_envelope_contract_error(
            requested_provider_id,
            "cas-root",
        ));
    }
    let mut workspace_hashes = Vec::with_capacity(envelope.owners.len());
    let mut source_blobs = Vec::with_capacity(envelope.owners.len());
    for owner in &envelope.owners {
        let relative_cas_path = normalized_envelope_relative_path(&owner.cas_path)?;
        let blob_path = cas_root.join(&relative_cas_path);
        let owner_bytes = std::fs::read(&blob_path).map_err(|error| {
            format!(
                "requested provider source envelope blob is missing: providerId={requested_provider_id} path={} error={error}",
                owner.path
            )
        })?;
        if agent_semantic_content_identity::hash_blob(&owner_bytes).value != owner.blob_digest
            || agent_semantic_content_identity::exact_selector_merkle::blake3_content_digest_v1(
                &owner_bytes,
            )
            .as_str()
                != owner.source_content_digest
        {
            return Err(format!(
                "requested provider source envelope blob digest is invalid: providerId={requested_provider_id} path={}",
                owner.path
            ));
        }
        workspace_hashes.push((owner.path.as_str(), owner.snapshot_leaf_digest.as_str()));
        source_blobs.push((
            crate::ClientDbSourceIndexPath::new(owner.path.clone()),
            owner_bytes,
        ));
    }
    let workspace_snapshot =
        agent_semantic_artifacts::WorkspaceSnapshot::from_file_hashes(workspace_hashes);
    let source_snapshot = workspace_snapshot.evidence(
        agent_semantic_artifacts::SourceSnapshotKind::Filesystem,
        envelope.source_snapshot.provider_digest.clone(),
    );
    if source_snapshot.root_digest != envelope.source_snapshot.root_digest
        || source_snapshot.leaf_count != envelope.source_snapshot.leaf_count
    {
        return Err(format!(
            "requested provider source envelope snapshot is invalid: providerId={requested_provider_id}"
        ));
    }
    Ok(CurrentSourceIndexSnapshot {
        workspace_generation: materialized_workspace_generation(
            &source_snapshot,
            source_blobs.len(),
        )?,
        workspace_snapshot,
        source_snapshot,
        source_blobs: crate::ClientDbSourceIndexSourceBlobs::from_normalized(source_blobs),
    })
}

fn provider_envelope_contract_error(provider_id: &str, reason: &str) -> String {
    format!(
        "requested provider source envelope contract is invalid: providerId={provider_id} reason={reason}"
    )
}

fn normalized_envelope_relative_path(path: &str) -> Result<PathBuf, String> {
    let mut normalized = PathBuf::new();
    for component in Path::new(path).components() {
        match component {
            std::path::Component::Normal(component) => normalized.push(component),
            std::path::Component::CurDir => {}
            std::path::Component::ParentDir
            | std::path::Component::RootDir
            | std::path::Component::Prefix(_) => {
                return Err(format!(
                    "provider source envelope CAS path escaped artifact root: path={path}"
                ));
            }
        }
    }
    if normalized.as_os_str().is_empty() {
        return Err("provider source envelope CAS path is empty".to_owned());
    }
    Ok(normalized)
}

pub(super) fn source_index_trace(stage: &str, started: Instant) {
    if std::env::var_os("ASP_SOURCE_INDEX_TRACE").is_some() {
        eprintln!(
            "[source-index-trace] stage={} elapsedMs={}",
            stage,
            started.elapsed().as_millis()
        );
    }
}

#[cfg(test)]
#[path = "../../tests/unit/source_index_api.rs"]
mod tests;
#[cfg(test)]
pub(crate) fn materialized_current_source_index_snapshot(
    workspace_snapshot: agent_semantic_content_identity::WorkspaceSnapshot,
    source_snapshot: agent_semantic_content_identity::SourceSnapshotEvidence,
    source_blobs: crate::ClientDbSourceIndexSourceBlobs,
) -> Result<CurrentSourceIndexSnapshot, String> {
    let workspace_generation =
        materialized_workspace_generation(&source_snapshot, source_blobs.len())?;
    Ok(CurrentSourceIndexSnapshot {
        workspace_snapshot,
        source_snapshot,
        workspace_generation,
        source_blobs,
    })
}

fn materialized_workspace_generation(
    source_snapshot: &agent_semantic_content_identity::SourceSnapshotEvidence,
    owner_count: usize,
) -> Result<
    agent_semantic_content_identity::workspace_generation_evidence::ValidatedWorkspaceGenerationV1,
    String,
> {
    agent_semantic_content_identity::workspace_generation_evidence::ValidatedWorkspaceGenerationV1::new(
        agent_semantic_content_identity::workspace_generation_evidence::WorkspaceGenerationEvidenceV1 {
            root_digest: source_snapshot.root_digest.clone(),
            root_depth: 1,
            leaf_count: source_snapshot.leaf_count as u64,
            owner_count: owner_count as u64,
        },
    )
    .map_err(|error| error.to_string())
}
