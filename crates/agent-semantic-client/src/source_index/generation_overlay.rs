use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use agent_semantic_content_identity::WorkspaceSnapshot;

use super::generation_commit::PreparedSourceIndexGeneration;

pub(super) async fn complete_incremental_generation(
    db_path: &Path,
    prepared: PreparedSourceIndexGeneration,
    changed_owner_paths: BTreeSet<String>,
) -> Result<PreparedSourceIndexGeneration, String> {
    if changed_owner_paths.is_empty() {
        return Err("incremental generation requires changed owner membership".to_string());
    }
    let (
        project_root,
        schema_id,
        schema_version,
        partial_source_snapshot,
        workspace_identity,
        project_resolutions,
    ) = {
        let refresh = prepared.refresh_request();
        let materialization = prepared.materialization();
        (
            refresh.import.project_root.clone(),
            refresh.import.schema_id.clone(),
            refresh.import.schema_version.clone(),
            refresh.source_snapshot.clone(),
            materialization.workspace_identity.clone(),
            materialization.project_resolutions.clone(),
        )
    };
    let active = agent_semantic_client_db::active_turso_source_index_generation(
        db_path,
        &project_root,
        &schema_id,
        &schema_version,
    )
    .await?
    .ok_or_else(|| {
        "incremental generation requires an admitted active source-index generation".to_string()
    })?;
    let present_changed_owners = prepared
        .refresh_request()
        .import
        .owners
        .iter()
        .map(|owner| owner.owner_path.as_str().to_string())
        .collect::<BTreeSet<_>>();
    let removed_owner_paths = changed_owner_paths
        .difference(&present_changed_owners)
        .cloned()
        .collect::<BTreeSet<_>>();
    let full_import = agent_semantic_client_db::overlay_active_source_index_import(
        &active.snapshot,
        &active.source_blobs,
        &prepared.refresh_request().import,
        &changed_owner_paths,
        &removed_owner_paths,
    )?;
    let file_hashes = full_import
        .file_hashes
        .iter()
        .map(|record| (record.path.as_str(), record.sha256.as_str()))
        .collect::<BTreeMap<_, _>>();
    let owner_hashes = full_import
        .owners
        .iter()
        .map(|owner| {
            let owner_path = owner.owner_path.as_str();
            file_hashes
                .get(owner_path)
                .map(|digest| (owner_path, *digest))
                .ok_or_else(|| {
                    format!(
                        "complete incremental generation is missing owner digest: ownerPath={owner_path}"
                    )
                })
        })
        .collect::<Result<Vec<_>, String>>()?;
    let workspace_snapshot = WorkspaceSnapshot::from_file_hashes(owner_hashes);
    workspace_snapshot.validate()?;
    let mut successor_source_snapshot = workspace_snapshot.evidence(
        partial_source_snapshot.source_kind,
        partial_source_snapshot.provider_digest,
    );
    successor_source_snapshot.base_root_digest = Some(active.snapshot.source_snapshot.root_digest);
    successor_source_snapshot.dirty_paths_digest = partial_source_snapshot.dirty_paths_digest;
    let materialization =
        agent_semantic_client_db::runtime_server_workspace::WorkspaceCanonicalMaterialization::from_source_index(
            workspace_identity,
            &successor_source_snapshot,
            &full_import,
            &full_import.source_blobs,
            project_resolutions,
        )?;
    let file_count = full_import.owners.len().min(u32::MAX as usize) as u32;
    Ok(prepared.with_complete_successor(successor_source_snapshot, file_count, materialization))
}
