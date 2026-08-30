use std::collections::BTreeSet;
use std::path::Path;

use agent_semantic_content_identity::WorkspaceSnapshot;

use crate::server_source_index::generation_commit::PreparedSourceIndexGeneration;

pub(super) async fn complete_generation_from_optional_active_base(
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
    let Some(active) = crate::active_turso_source_index_generation(
        db_path,
        &project_root,
        &schema_id,
        &schema_version,
    )
    .await?
    else {
        // The first admitted generation is the baseline. Changed paths describe
        // the targeted cold build; they do not imply that a Merkle base exists.
        return Ok(prepared);
    };
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
    let full_import = crate::overlay_active_source_index_import(
        &active.snapshot,
        &active.source_blobs,
        &prepared.refresh_request().import,
        &changed_owner_paths,
        &removed_owner_paths,
    )?;
    let workspace_snapshot = WorkspaceSnapshot::from_file_bytes(full_import.source_blobs.iter());
    workspace_snapshot.validate()?;
    let mut successor_source_snapshot = workspace_snapshot.evidence(
        partial_source_snapshot.source_kind,
        partial_source_snapshot.provider_digest,
    );
    successor_source_snapshot.base_root_digest = Some(active.snapshot.source_snapshot.root_digest);
    successor_source_snapshot.dirty_paths_digest = partial_source_snapshot.dirty_paths_digest;
    let materialization =
        crate::runtime_server_workspace::WorkspaceCanonicalMaterialization::from_source_index(
            workspace_identity,
            &workspace_snapshot,
            &successor_source_snapshot,
            &full_import,
            &full_import.source_blobs,
            project_resolutions,
        )?;
    let file_count = full_import.owners.len().min(u32::MAX as usize) as u32;
    Ok(prepared.with_complete_successor(successor_source_snapshot, file_count, materialization))
}
