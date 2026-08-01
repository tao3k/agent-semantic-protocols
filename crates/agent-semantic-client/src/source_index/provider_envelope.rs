//! Publishes provider-scoped source snapshots into State Core artifacts.

use std::path::{Path, PathBuf};

use super::CurrentSourceIndexSnapshot;

/// Typed inputs for publishing one provider-scoped source snapshot envelope.
pub struct ProviderSourceSnapshotEnvelopePublicationV1<'a> {
    pub snapshot: &'a CurrentSourceIndexSnapshot,
    pub provider_id: &'a str,
    pub address_provider_digest: &'a str,
    pub source_extensions: &'a [String],
    pub artifact_root: &'a Path,
    pub provider_workspace_root: &'a Path,
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct ProviderSourceSnapshotEnvelopeV1<'a> {
    schema_id: &'static str,
    schema_version: &'static str,
    provider_id: &'a str,
    address_provider_digest: &'a str,
    provider_workspace_root: &'a str,
    provider_workspace_identity_digest: &'a str,
    source_snapshot: &'a agent_semantic_content_identity::SourceSnapshotEvidence,
    root_depth: usize,
    materialization_state: &'static str,
    owner_coverage: &'static str,
    cas_root: &'a Path,
    owners: Vec<ProviderSourceSnapshotOwnerV1>,
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct ProviderSourceSnapshotOwnerV1 {
    path: String,
    snapshot_leaf_digest: String,
    blob_digest: String,
    source_content_digest: String,
    cas_path: String,
}

fn provider_source_snapshot_merkle_root_depth(leaf_count: usize) -> usize {
    if leaf_count <= 1 {
        0
    } else {
        usize::BITS as usize - (leaf_count - 1).leading_zeros() as usize
    }
}

fn collect_provider_source_snapshot_owners(
    snapshot: &CurrentSourceIndexSnapshot,
    source_extensions: &[String],
    artifact_root: &Path,
) -> Result<(PathBuf, Vec<ProviderSourceSnapshotOwnerV1>), String> {
    let cas_root = artifact_root.join("source-blob-cas").join("v1");
    let cas = agent_semantic_content_identity::ContentAddressedStore::new(&cas_root);
    let normalized_extensions = source_extensions
        .iter()
        .map(|extension| extension.trim_start_matches('.').to_ascii_lowercase())
        .collect::<std::collections::BTreeSet<_>>();
    let mut owners = Vec::new();
    for (path, bytes) in snapshot.source_blobs.iter() {
        let extension = Path::new(path)
            .extension()
            .map(|extension| extension.to_string_lossy().to_ascii_lowercase());
        if !normalized_extensions.is_empty()
            && extension
                .as_ref()
                .is_none_or(|extension| !normalized_extensions.contains(extension))
        {
            continue;
        }
        let snapshot_leaf_digest =
            snapshot
                .workspace_snapshot
                .file_digest(path)
                .ok_or_else(|| {
                    format!(
                "provider source blob is not committed by snapshot root: path={path} rootDigest={}",
                snapshot.source_snapshot.root_digest
            )
                })?;
        let blob_digest = agent_semantic_content_identity::hash_blob(bytes).value;
        let source_content_digest =
            agent_semantic_content_identity::exact_selector_merkle::blake3_content_digest_v1(bytes)
                .as_str()
                .to_owned();
        let blob_path = cas.write(&blob_digest, bytes).map_err(|error| {
            format!(
                "failed to publish provider source blob to ASP content store: path={path} blobDigest={blob_digest} error={error}"
            )
        })?;
        let cas_path = blob_path
            .strip_prefix(&cas_root)
            .map_err(|error| {
                format!(
                    "provider source blob escaped ASP content store: path={} casRoot={} error={error}",
                    blob_path.display(),
                    cas_root.display()
                )
            })?
            .to_string_lossy()
            .replace('\\', "/");
        owners.push(ProviderSourceSnapshotOwnerV1 {
            path: path.to_string(),
            snapshot_leaf_digest: snapshot_leaf_digest.to_string(),
            blob_digest,
            source_content_digest,
            cas_path,
        });
    }
    owners.sort_by(|left, right| left.path.cmp(&right.path));
    Ok((cas_root, owners))
}

fn provider_source_snapshot(
    snapshot: &CurrentSourceIndexSnapshot,
    owners: &[ProviderSourceSnapshotOwnerV1],
) -> Result<agent_semantic_content_identity::SourceSnapshotEvidence, String> {
    let workspace_snapshot = agent_semantic_artifacts::WorkspaceSnapshot::from_file_hashes(
        owners
            .iter()
            .map(|owner| (owner.path.as_str(), owner.snapshot_leaf_digest.as_str())),
    );
    Ok(workspace_snapshot.evidence(
        agent_semantic_content_identity::SourceSnapshotKind::Filesystem,
        snapshot.source_snapshot.provider_digest.clone(),
    ))
}

fn source_snapshot_envelope_component(value: &str) -> String {
    value
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || matches!(character, '-' | '_') {
                character
            } else {
                '_'
            }
        })
        .collect()
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProviderWorkspaceIdentityV1 {
    pub root: String,
    pub digest: String,
}

pub fn provider_workspace_identity_v1(
    provider_workspace_root: &Path,
) -> Result<ProviderWorkspaceIdentityV1, String> {
    let resolved =
        agent_semantic_client_core::state_core::ResolvedState::resolve(provider_workspace_root)?;
    let canonical_workspace_root = provider_workspace_root
        .canonicalize()
        .unwrap_or_else(|_| provider_workspace_root.to_path_buf());
    let identity_root = resolved
        .repo
        .git_toplevel
        .as_deref()
        .and_then(|git_toplevel| canonical_workspace_root.strip_prefix(git_toplevel).ok())
        .filter(|relative| !relative.as_os_str().is_empty())
        .map(|relative| relative.to_string_lossy().replace('\\', "/"))
        .unwrap_or_else(|| ".".to_owned());
    let digest = agent_semantic_content_identity::exact_selector_merkle::blake3_content_digest_v1(
        format!("asp.provider-workspace.v1\0{identity_root}").as_bytes(),
    )
    .as_str()
    .to_owned();
    Ok(ProviderWorkspaceIdentityV1 {
        root: identity_root,
        digest,
    })
}

pub(super) fn source_snapshot_envelope_file_name(
    provider_id: &str,
    provider_digest: &str,
    provider_workspace_identity_digest: &str,
) -> String {
    format!(
        "{}--{}--{}.json",
        source_snapshot_envelope_component(provider_id),
        source_snapshot_envelope_component(provider_digest),
        source_snapshot_envelope_component(provider_workspace_identity_digest)
    )
}

fn publish_provider_source_snapshot_envelope_file(
    artifact_root: &Path,
    envelope: &ProviderSourceSnapshotEnvelopeV1<'_>,
    provider_digest: &str,
) -> Result<PathBuf, String> {
    let envelope_dir = artifact_root
        .join("source-snapshot-envelopes")
        .join("v1")
        .join(&envelope.source_snapshot.root_digest);
    std::fs::create_dir_all(&envelope_dir).map_err(|error| {
        format!(
            "failed to create provider source envelope directory {}: {error}",
            envelope_dir.display()
        )
    })?;
    let file_name = source_snapshot_envelope_file_name(
        envelope.provider_id,
        provider_digest,
        envelope.provider_workspace_identity_digest,
    );
    let envelope_path = envelope_dir.join(&file_name);
    let bytes = serde_json::to_vec_pretty(envelope)
        .map_err(|error| format!("failed to encode provider source snapshot envelope: {error}"))?;
    let temporary = envelope_dir.join(format!(".{file_name}.tmp-{}", std::process::id()));
    std::fs::write(&temporary, bytes).map_err(|error| {
        format!(
            "failed to write provider source snapshot envelope {}: {error}",
            temporary.display()
        )
    })?;
    std::fs::rename(&temporary, &envelope_path).map_err(|error| {
        let _ = std::fs::remove_file(&temporary);
        format!(
            "failed to publish provider source snapshot envelope {}: {error}",
            envelope_path.display()
        )
    })?;
    Ok(envelope_path)
}

/// Publishes one provider-scoped, root-bound source envelope.
pub fn publish_provider_source_snapshot_envelope(
    request: ProviderSourceSnapshotEnvelopePublicationV1<'_>,
) -> Result<PathBuf, String> {
    let provider_id = agent_semantic_client_core::ProviderId::from(request.provider_id);
    let provider_workspace_identity =
        provider_workspace_identity_v1(request.provider_workspace_root)?;
    let (cas_root, owners) = collect_provider_source_snapshot_owners(
        request.snapshot,
        request.source_extensions,
        request.artifact_root,
    )?;
    let source_snapshot = provider_source_snapshot(request.snapshot, &owners)?;
    let envelope = ProviderSourceSnapshotEnvelopeV1 {
        schema_id: "asp.exact-source-snapshot-envelope.v1",
        schema_version: "1",
        provider_id: provider_id.as_str(),
        address_provider_digest: request.address_provider_digest,
        provider_workspace_root: &provider_workspace_identity.root,
        provider_workspace_identity_digest: &provider_workspace_identity.digest,
        source_snapshot: &source_snapshot,
        root_depth: provider_source_snapshot_merkle_root_depth(source_snapshot.leaf_count),
        materialization_state: "artifact-complete",
        owner_coverage: "complete",
        cas_root: &cas_root,
        owners,
    };
    publish_provider_source_snapshot_envelope_file(
        request.artifact_root,
        &envelope,
        request.address_provider_digest,
    )
}

/// Named inputs for resolving one pre-published provider workspace envelope.
pub struct ProviderSourceEnvelopeLookupRequestV1<'a> {
    pub project_root: &'a Path,
    pub artifact_root: &'a Path,
    pub language_id: &'a agent_semantic_client_core::LanguageId,
    pub provider_id: &'a agent_semantic_client_core::ProviderId,
    pub provider_registry: &'a agent_semantic_client_core::ProviderRegistrySnapshot,
}

struct ProviderSourceEnvelopeLookupV1 {
    provider_digest: String,
    provider_workspace_identity: ProviderWorkspaceIdentityV1,
    envelope_path: PathBuf,
}

fn provider_source_envelope_lookup(
    request: &ProviderSourceEnvelopeLookupRequestV1<'_>,
) -> Result<ProviderSourceEnvelopeLookupV1, String> {
    let provider = request
        .provider_registry
        .providers
        .iter()
        .find(|provider| {
            &provider.language_id == request.language_id
                && &provider.provider_id == request.provider_id
        })
        .ok_or_else(|| {
            format!(
                "requested target provider is not registered: languageId={} providerId={}",
                request.language_id, request.provider_id
            )
        })?;
    let provider_digest =
        provider_registry_address_digest(request.provider_registry, request.project_root);
    let provider_workspace_identity = provider_workspace_identity_v1(request.project_root)?;
    let file_name = source_snapshot_envelope_file_name(
        provider.provider_id.as_str(),
        &provider_digest,
        &provider_workspace_identity.digest,
    );
    let envelope_path = request
        .artifact_root
        .join("source-snapshot-envelopes")
        .join("v1")
        .join(file_name);
    Ok(ProviderSourceEnvelopeLookupV1 {
        provider_digest,
        provider_workspace_identity,
        envelope_path,
    })
}

pub(super) fn provider_registry_address_digest(
    provider_registry: &agent_semantic_client_core::ProviderRegistrySnapshot,
    project_root: &Path,
) -> String {
    let registry = provider_registry.evidence(project_root);
    agent_semantic_artifacts::provider_digest(registry.fingerprint.as_bytes())
}

/// Load one already-published provider workspace envelope without materializing it.
pub fn current_provider_source_index_snapshot_at_artifact_root_with_registry(
    request: ProviderSourceEnvelopeLookupRequestV1<'_>,
) -> Result<CurrentSourceIndexSnapshot, String> {
    let lookup = provider_source_envelope_lookup(&request)?;
    if !lookup.envelope_path.is_file() {
        return Err(format!(
            "requested provider workspace source envelope is not published: languageId={} providerId={} providerWorkspaceRoot={} envelope={}",
            request.language_id,
            request.provider_id,
            lookup.provider_workspace_identity.root,
            lookup.envelope_path.display()
        ));
    }
    super::api::load_provider_source_index_snapshot_envelope(
        request.artifact_root,
        request.provider_id.as_str(),
        &lookup.provider_digest,
        &lookup.provider_workspace_identity,
        &lookup.envelope_path,
    )
}

/// Resolve the canonical pre-published envelope path for one provider workspace.
pub fn provider_source_snapshot_envelope_path_at_artifact_root_with_registry(
    request: ProviderSourceEnvelopeLookupRequestV1<'_>,
) -> Result<PathBuf, String> {
    Ok(provider_source_envelope_lookup(&request)?.envelope_path)
}

/// Refresh and publish one provider workspace envelope, then load it strictly.
pub fn ensure_provider_source_index_snapshot_at_artifact_root_with_registry(
    request: ProviderSourceEnvelopeLookupRequestV1<'_>,
) -> Result<CurrentSourceIndexSnapshot, String> {
    super::generation::publish_target_provider_source_envelope_v1(
        super::generation::TargetProviderSourceEnvelopePublicationRequestV1 {
            collection_scope: super::collect::SourceIndexCollectionScope::TargetProvider {
                language_id: request.language_id.clone(),
                provider_id: request.provider_id.clone(),
            },
            provider_registry: request.provider_registry,
            artifact_root: request.artifact_root,
            project_root: request.project_root,
        },
    )?;
    current_provider_source_index_snapshot_at_artifact_root_with_registry(request)
}

#[cfg(test)]
#[path = "../../tests/unit/provider_source_snapshot_envelope_generation.rs"]
mod provider_source_snapshot_envelope_generation_tests;
