use std::path::Path;

use agent_semantic_client_core::{ClientCacheFileHash, ProviderRegistryEvidence};
use agent_semantic_client_db::{
    ClientDbSourceIndexPath, ClientDbSourceIndexScopeFile, ClientDbSourceIndexSourceBlobs,
    source_index_file_hashes,
};
use sha2::{Digest as _, Sha256};

pub(crate) async fn source_index_snapshot_from_files_async(
    index_root: &Path,
    files: &[ClientDbSourceIndexScopeFile],
    registry: &ProviderRegistryEvidence,
) -> Result<
    (
        Vec<ClientCacheFileHash>,
        agent_semantic_artifacts::WorkspaceSnapshot,
        agent_semantic_content_identity::SourceSnapshotEvidence,
        ClientDbSourceIndexSourceBlobs,
    ),
    String,
> {
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
            format!("{:x}", Sha256::digest(&bytes)),
        ));
        source_blobs.push((ClientDbSourceIndexPath::new(snapshot_path), bytes));
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
    ))
}

#[cfg(test)]
#[path = "../../tests/unit/source_index_async_snapshot.rs"]
mod tests;
