// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-only

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use crate::{
    ClientDbSourceIndexPath, ClientDbSourceIndexScopeFile, ClientDbSourceIndexSourceBlobs,
    source_index_file_hashes,
};
use agent_semantic_client_core::{
    ClientCacheFileHash, RuntimeProviderProjection, RuntimeProviderProjectionEvidence,
};

pub(crate) async fn source_index_snapshot_from_files_async(
    index_root: &Path,
    files: &[ClientDbSourceIndexScopeFile],
    registry: &RuntimeProviderProjectionEvidence,
    provider_registry: &RuntimeProviderProjection,
) -> Result<
    (
        Vec<ClientCacheFileHash>,
        agent_semantic_artifacts::WorkspaceSnapshot,
        agent_semantic_content_identity::SourceSnapshotEvidence,
        ClientDbSourceIndexSourceBlobs,
        crate::server_source_index::projection::ProviderProjectionAuxiliaryOwners,
    ),
    String,
> {
    let auxiliary_owners =
        collect_projection_auxiliary_owners(index_root, files, provider_registry).await?;
    // One Tokio filesystem future per owner floods the blocking scheduler,
    // while one sequential batch underuses large machines. Partition the
    // immutable inventory into an adaptive O(CPU) set of deterministic shards;
    // each shard performs sequential blocking reads and returns indexed facts.
    let source_paths = files
        .iter()
        .map(|file| {
            if file.path.is_absolute() {
                file.path.clone()
            } else {
                index_root.join(&file.path)
            }
        })
        .collect::<Vec<_>>();
    let worker_count = std::thread::available_parallelism()
        .map(std::num::NonZeroUsize::get)
        .unwrap_or(1)
        .min(source_paths.len().max(1));
    let source_count = source_paths.len();
    // Paths are canonically ordered, so adjacent owners commonly belong to the
    // same crate and have correlated sizes. Contiguous chunks therefore put a
    // cluster of large owners on one worker and make cold latency equal to the
    // slowest directory. Round-robin assignment balances those clusters while
    // the indexed merge below preserves the canonical input order.
    let mut shards = (0..worker_count)
        .map(|_| Vec::<(usize, PathBuf)>::new())
        .collect::<Vec<_>>();
    for (index, source_path) in source_paths.into_iter().enumerate() {
        shards[index % worker_count].push((index, source_path));
    }
    let mut reads = tokio::task::JoinSet::new();
    for shard in shards {
        let index_root_owned = index_root.to_path_buf();
        reads.spawn_blocking(move || {
            shard
                .into_iter()
                .map(|(index, source_path)| {
                    let bytes = std::fs::read(&source_path).map_err(|error| {
                        format!(
                            "failed to hash workspace source {} with BLAKE3: {error}",
                            source_path.display()
                        )
                    })?;
                    let snapshot_path = source_path
                        .strip_prefix(&index_root_owned)
                        .unwrap_or(source_path.as_path())
                        .to_string_lossy()
                        .replace('\\', "/");
                    Ok::<_, String>((index, snapshot_path, bytes))
                })
                .collect::<Result<Vec<_>, String>>()
        });
    }
    let mut indexed_results = Vec::with_capacity(source_count);
    while let Some(joined) = reads.join_next().await {
        indexed_results.extend(
            joined.map_err(|error| format!("workspace source read shard failed: {error}"))??,
        );
    }
    indexed_results.sort_unstable_by_key(|(index, _, _)| *index);

    let mut workspace_file_hashes = Vec::with_capacity(files.len());
    let mut source_blobs = Vec::with_capacity(files.len());
    for (_, snapshot_path, bytes) in indexed_results {
        workspace_file_hashes.push((
            snapshot_path.clone(),
            blake3::hash(&bytes).to_hex().to_string(),
        ));
        source_blobs.push((ClientDbSourceIndexPath::new(snapshot_path), bytes));
    }
    for owner in auxiliary_owners.values().flatten() {
        let digest = blake3::hash(&owner.source_bytes).to_hex().to_string();
        if let Some((_, existing)) = workspace_file_hashes
            .iter()
            .find(|(path, _)| path == &owner.owner_path)
        {
            if existing != &digest {
                return Err(format!(
                    "workspace snapshot path has conflicting immutable bytes: {}",
                    owner.owner_path
                ));
            }
        } else {
            workspace_file_hashes.push((owner.owner_path.clone(), digest));
            source_blobs.push((
                ClientDbSourceIndexPath::new(owner.owner_path.clone()),
                owner.source_bytes.clone(),
            ));
        }
    }
    let typed_source_blobs = ClientDbSourceIndexSourceBlobs::from_normalized(source_blobs);
    let file_hashes = source_index_file_hashes(
        index_root,
        files,
        &typed_source_blobs,
        &registry.fingerprint,
        registry.scope_dirs.iter().map(String::as_str),
    )?;
    let workspace_snapshot =
        agent_semantic_artifacts::WorkspaceSnapshot::from_file_hashes(workspace_file_hashes);
    let source_snapshot = workspace_snapshot.evidence(
        agent_semantic_artifacts::SourceSnapshotKind::Filesystem,
        crate::server_source_index::api::provider_scope_digest(registry, files),
    );
    Ok((
        file_hashes,
        workspace_snapshot,
        source_snapshot,
        typed_source_blobs,
        auxiliary_owners,
    ))
}

async fn collect_projection_auxiliary_owners(
    index_root: &Path,
    files: &[ClientDbSourceIndexScopeFile],
    provider_registry: &RuntimeProviderProjection,
) -> Result<crate::server_source_index::projection::ProviderProjectionAuxiliaryOwners, String> {
    let providers = provider_registry
        .providers
        .iter()
        .map(|provider| (provider.provider_id.as_str(), provider))
        .collect::<BTreeMap<_, _>>();
    let mut candidates = BTreeMap::<PathBuf, BTreeSet<String>>::new();
    for file in files {
        let Some(provider) = providers.get(file.provider_id.as_str()) else {
            continue;
        };
        let source_path = if file.path.is_absolute() {
            file.path.clone()
        } else {
            index_root.join(&file.path)
        };
        let mut directory = source_path.parent();
        while let Some(current) = directory {
            if !current.starts_with(index_root) {
                break;
            }
            for config_file in &provider.config_files {
                candidates
                    .entry(current.join(config_file))
                    .or_default()
                    .insert(provider.provider_id.as_str().to_owned());
            }
            if current == index_root {
                break;
            }
            directory = current.parent();
        }
    }

    let mut reads = tokio::task::JoinSet::new();
    for (path, provider_ids) in candidates {
        let index_root = index_root.to_path_buf();
        reads.spawn(async move {
            match tokio::fs::read(&path).await {
                Ok(source_bytes) => {
                    let owner_path = path
                        .strip_prefix(&index_root)
                        .map_err(|error| {
                            format!(
                                "projection auxiliary path escaped workspace: path={} error={error}",
                                path.display()
                            )
                        })?
                        .to_string_lossy()
                        .replace('\\', "/");
                    Ok::<_, String>(Some((provider_ids, owner_path, source_bytes)))
                }
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
                Err(error) => Err(format!(
                    "read projection auxiliary owner: path={} error={error}",
                    path.display()
                )),
            }
        });
    }

    let mut auxiliary_owners = BTreeMap::<
        String,
        Vec<agent_semantic_provider_transport::projection_batch::ProviderProjectionOwner>,
    >::new();
    while let Some(result) = reads.join_next().await {
        let Some((provider_ids, owner_path, source_bytes)) =
            result.map_err(|error| format!("projection auxiliary read task failed: {error}"))??
        else {
            continue;
        };
        let source_leaf_digest =
            agent_semantic_content_identity::exact_selector_merkle::blake3_content_digest_v1(
                &source_bytes,
            )
            .as_str()
            .to_owned();
        for provider_id in provider_ids {
            auxiliary_owners.entry(provider_id).or_default().push(
                agent_semantic_provider_transport::projection_batch::ProviderProjectionOwner {
                    owner_path: owner_path.clone(),
                    source_leaf_digest: source_leaf_digest.clone(),
                    source_bytes: source_bytes.clone(),
                },
            );
        }
    }
    for owners in auxiliary_owners.values_mut() {
        owners.sort_by(|left, right| left.owner_path.cmp(&right.owner_path));
    }
    Ok(auxiliary_owners)
}

#[cfg(test)]
#[path = "../../tests/unit/source_index_async_snapshot.rs"]
mod tests;
