use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use agent_semantic_client_core::{
    ClientCacheFileHash, RuntimeProviderProjection, RuntimeProviderProjectionEvidence,
};
use agent_semantic_client_db::{
    ClientDbSourceIndexPath, ClientDbSourceIndexScopeFile, ClientDbSourceIndexSourceBlobs,
    source_index_file_hashes,
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
        super::projection::ProviderProjectionAuxiliaryOwners,
    ),
    String,
> {
    let auxiliary_owners =
        collect_projection_auxiliary_owners(index_root, files, provider_registry).await?;
    let io_concurrency = tokio::runtime::Handle::current()
        .metrics()
        .num_workers()
        .max(1);
    let mut reads = tokio::task::JoinSet::new();
    let mut next_file = 0_usize;
    let mut results = vec![None; files.len()];
    while next_file < files.len() || !reads.is_empty() {
        while next_file < files.len() && reads.len() < io_concurrency {
            let index = next_file;
            let source_path = if files[index].path.is_absolute() {
                files[index].path.clone()
            } else {
                index_root.join(&files[index].path)
            };
            let index_root = index_root.to_path_buf();
            reads.spawn(async move {
                let bytes = tokio::fs::read(&source_path).await.map_err(|error| {
                    format!(
                        "failed to hash workspace source {} with BLAKE3: {error}",
                        source_path.display()
                    )
                })?;
                let snapshot_path = source_path
                    .strip_prefix(&index_root)
                    .unwrap_or(source_path.as_path())
                    .to_string_lossy()
                    .replace('\\', "/");
                Ok::<_, String>((index, snapshot_path, bytes))
            });
            next_file += 1;
        }
        if let Some(result) = reads.join_next().await {
            let (index, snapshot_path, bytes) =
                result.map_err(|error| format!("workspace source read task failed: {error}"))??;
            results[index] = Some((snapshot_path, bytes));
        }
    }

    let mut workspace_file_hashes = Vec::with_capacity(files.len());
    let mut source_blobs = Vec::with_capacity(files.len());
    for result in results {
        let (snapshot_path, bytes) =
            result.ok_or_else(|| "workspace source read task omitted a file".to_owned())?;
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
        super::api::provider_scope_digest(registry, files),
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
) -> Result<super::projection::ProviderProjectionAuxiliaryOwners, String> {
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
