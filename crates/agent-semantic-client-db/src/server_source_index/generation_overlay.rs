use std::collections::BTreeSet;
use std::path::Path;

use agent_semantic_content_identity::WorkspaceSnapshot;

use crate::server_source_index::generation_commit::PreparedSourceIndexGeneration;

pub(super) async fn complete_generation_from_optional_active_base(
    db_path: &Path,
    prepared: PreparedSourceIndexGeneration,
    mut changed_owner_paths: BTreeSet<String>,
    replacement_authority: Option<&agent_semantic_search::ResidentSearchAuthority>,
) -> Result<PreparedSourceIndexGeneration, String> {
    if changed_owner_paths.is_empty() && replacement_authority.is_none() {
        return Err("incremental generation requires changed owner membership".to_string());
    }
    let (
        project_root,
        schema_id,
        schema_version,
        partial_source_snapshot,
        workspace_identity,
        mut project_resolutions,
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
        if changed_owner_paths.is_empty() {
            return Err(
                "target-provider cold generation requires current owner membership".to_owned(),
            );
        }
        return Ok(prepared);
    };
    if let Some(replacement_authority) = replacement_authority {
        changed_owner_paths.extend(active.snapshot.owners.iter().filter_map(|owner| {
            (owner.language_id.as_deref() == Some(replacement_authority.language_id.as_str())
                && owner.provider_id.as_deref() == Some(replacement_authority.provider_id.as_str()))
            .then(|| owner.owner_path.clone())
        }));
        let active_materialization =
            crate::engine::active_turso_workspace_generation_materialization(
                db_path,
                workspace_identity.as_str(),
                &project_root,
            )
            .await?
            .ok_or_else(|| {
                "target-provider replacement requires the active canonical materialization"
                    .to_owned()
            })?;
        project_resolutions = merge_target_project_resolutions(
            active_materialization.project_resolutions,
            project_resolutions,
            replacement_authority,
        )?;
    }
    if changed_owner_paths.is_empty() {
        return Err("incremental generation requires changed owner membership".to_owned());
    }
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
    let successor_source_snapshot = workspace_snapshot.overlay_evidence(
        partial_source_snapshot.source_kind,
        partial_source_snapshot.provider_digest,
        active.snapshot.source_snapshot.root_digest,
        agent_semantic_content_identity::WorkspaceOverlayPaths::new(
            present_changed_owners.iter().cloned(),
            removed_owner_paths.iter().cloned(),
        ),
    )?;
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
    Ok(prepared.with_complete_successor(
        successor_source_snapshot,
        file_count,
        full_import,
        materialization,
    ))
}

fn merge_target_project_resolutions(
    mut active: Vec<agent_semantic_content_identity::AdmittedProjectResolution>,
    incoming: Vec<agent_semantic_content_identity::AdmittedProjectResolution>,
    replacement_authority: &agent_semantic_search::ResidentSearchAuthority,
) -> Result<Vec<agent_semantic_content_identity::AdmittedProjectResolution>, String> {
    if incoming.iter().any(|resolution| {
        resolution.resolution.language_id != replacement_authority.language_id.as_str()
            || resolution.resolution.provider_id != replacement_authority.provider_id.as_str()
    }) {
        return Err(
            "target-provider replacement received a cross-authority project resolution".to_owned(),
        );
    }
    active.retain(|resolution| {
        resolution.resolution.language_id != replacement_authority.language_id.as_str()
            || resolution.resolution.provider_id != replacement_authority.provider_id.as_str()
    });
    active.extend(incoming);
    active.sort_by(|left, right| {
        (
            &left.resolution.language_id,
            &left.resolution.provider_id,
            &left.resolution.project_entry,
            &left.candidate_base,
        )
            .cmp(&(
                &right.resolution.language_id,
                &right.resolution.provider_id,
                &right.resolution.project_entry,
                &right.candidate_base,
            ))
    });
    for pair in active.windows(2) {
        if pair[0].resolution.language_id == pair[1].resolution.language_id
            && pair[0].resolution.provider_id == pair[1].resolution.provider_id
            && pair[0].resolution.project_entry == pair[1].resolution.project_entry
        {
            return Err(
                "duplicate project resolution authority in successor generation".to_owned(),
            );
        }
    }
    Ok(active)
}
