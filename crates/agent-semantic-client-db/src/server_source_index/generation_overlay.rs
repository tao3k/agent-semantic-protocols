// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use crate::server_source_index::generation_commit::PreparedSourceIndexGeneration;

fn partition_affected_owner_membership(
    affected_owner_paths: &BTreeSet<String>,
    present_owner_paths: &BTreeSet<String>,
) -> (BTreeSet<String>, BTreeSet<String>) {
    let upsert_owner_paths = affected_owner_paths
        .intersection(present_owner_paths)
        .cloned()
        .collect();
    let removed_owner_paths = affected_owner_paths
        .difference(present_owner_paths)
        .cloned()
        .collect();
    (upsert_owner_paths, removed_owner_paths)
}

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
    let active = match crate::engine::active_turso_source_index_generation_candidate(
        db_path,
        &project_root,
        &schema_id,
        &schema_version,
    )
    .await?
    {
        crate::engine::ClientDbActiveSourceIndexGenerationCandidate::Missing => {
            // The first admitted generation is the baseline. Changed paths describe
            // the targeted cold build; they do not imply that a Merkle base exists.
            if changed_owner_paths.is_empty() {
                return Err(
                    "target-provider cold generation requires current owner membership".to_owned(),
                );
            }
            return Ok(prepared);
        }
        crate::engine::ClientDbActiveSourceIndexGenerationCandidate::Ready(active) => *active,
        crate::engine::ClientDbActiveSourceIndexGenerationCandidate::ReuseRejected(reason) => {
            if replacement_authority.is_none() {
                return Err(format!(
                    "incremental generation active base was rejected: {reason}"
                ));
            }
            eprintln!(
                "[active-generation-reuse] state=rejected recovery=complete-provider-baseline reason={reason}"
            );
            return Ok(prepared);
        }
    };
    let active_materialization = crate::engine::active_turso_workspace_generation_materialization(
        db_path,
        workspace_identity.as_str(),
        &project_root,
    )
    .await?
    .ok_or_else(|| {
        "incremental generation requires the active canonical materialization".to_owned()
    })?;
    let active_source_blobs = complete_active_source_blobs(
        &active.source_blobs,
        &active_materialization.workspace_snapshot,
        &active_materialization.owners,
        &active_materialization.auxiliary_owners,
    )?;
    if let Some(replacement_authority) = replacement_authority {
        changed_owner_paths.extend(
            active
                .snapshot
                .owners
                .iter()
                .filter(|owner| {
                    owner.language_id.as_deref() == Some(replacement_authority.language_id.as_str())
                        && owner.provider_id.as_deref()
                            == Some(replacement_authority.provider_id.as_str())
                })
                .map(|owner| owner.owner_path.clone()),
        );
        project_resolutions = merge_target_project_resolutions(
            active_materialization.project_resolutions.clone(),
            project_resolutions,
            replacement_authority,
        )?;
    } else {
        // An owner-local delta changes neither the admitted project roots nor
        // their provider ownership. The partial projection intentionally has
        // no authority to replace the complete base resolution set.
        project_resolutions = active_materialization.project_resolutions.clone();
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
    let (upsert_owner_paths, removed_owner_paths) =
        partition_affected_owner_membership(&changed_owner_paths, &present_changed_owners);
    let full_import = crate::overlay_active_source_index_import(
        &active.snapshot,
        &active_source_blobs,
        &prepared.refresh_request().import,
        &upsert_owner_paths,
        &removed_owner_paths,
    )?;
    let delta_materialization = prepared.materialization();
    let delta_file_digests = delta_materialization
        .workspace_snapshot
        .file_digests()
        .map(|(path, digest)| (path.to_owned(), digest.to_owned()))
        .collect::<Vec<_>>();
    let mut overlay_changed_paths = delta_file_digests
        .iter()
        .filter(|(path, digest)| {
            active_materialization.workspace_snapshot.file_digest(path) != Some(digest.as_str())
        })
        .map(|(path, _)| path.clone())
        .collect::<BTreeSet<_>>();
    // A same-byte Hook mutation is still an admitted owner-local successor.
    // Retain its explicit cut in provenance even when the canonical root is
    // equal; auxiliary owners are added above only when their digest changed.
    overlay_changed_paths.extend(upsert_owner_paths.iter().cloned());
    let workspace_snapshot = active_materialization
        .workspace_snapshot
        .canonical_with_overlay_delta(delta_file_digests, removed_owner_paths.iter().cloned());
    workspace_snapshot.validate()?;
    let successor_source_snapshot = workspace_snapshot.overlay_evidence(
        partial_source_snapshot.source_kind,
        partial_source_snapshot.provider_digest,
        active.snapshot.source_snapshot.root_digest,
        agent_semantic_content_identity::WorkspaceOverlayPaths::new(
            overlay_changed_paths,
            removed_owner_paths.iter().cloned(),
        ),
    )?;
    let materialization = crate::runtime_server_workspace::WorkspaceCanonicalMaterialization::from_source_index_overlay(
        active_materialization,
        crate::runtime_server_workspace::WorkspaceCanonicalMaterializationOverlay {
            delta: delta_materialization,
            workspace_snapshot,
            source_snapshot: successor_source_snapshot.clone(),
            import: &full_import,
            upsert_owner_paths: &upsert_owner_paths,
            removed_owner_paths: &removed_owner_paths,
            project_resolutions,
        },
    )?;
    let file_count = full_import.owners.len().min(u32::MAX as usize) as u32;
    Ok(prepared.with_complete_successor(
        successor_source_snapshot,
        file_count,
        full_import,
        materialization,
    ))
}

fn complete_active_source_blobs(
    active: &crate::ClientDbActiveGenerationSourceBlobs,
    workspace_snapshot: &agent_semantic_content_identity::WorkspaceSnapshot,
    owners: &[crate::runtime_server_workspace::WorkspaceOwnerSnapshot],
    auxiliary_owners: &[crate::runtime_server_workspace::WorkspaceAuxiliaryOwnerSnapshot],
) -> Result<crate::ClientDbActiveGenerationSourceBlobs, String> {
    let mut blobs = active
        .owners
        .iter()
        .cloned()
        .map(|blob| (blob.owner_path.clone(), blob))
        .collect::<BTreeMap<_, _>>();
    for owner in owners {
        validate_materialized_blob_identity(
            workspace_snapshot,
            &owner.owner_path,
            &owner.content_digest,
        )?;
        if !blobs.contains_key(&owner.owner_path) {
            return Err(format!(
                "active canonical materialization is missing searchable owner bytes: {}",
                owner.owner_path
            ));
        }
    }
    for owner in auxiliary_owners {
        validate_materialized_blob_identity(
            workspace_snapshot,
            &owner.owner_path,
            &owner.content_digest,
        )?;
        blobs.entry(owner.owner_path.clone()).or_insert_with(|| {
            crate::ClientDbActiveGenerationSourceBlob {
                owner_path: owner.owner_path.clone(),
                content_digest: owner.content_digest.clone(),
                source_bytes: std::sync::Arc::from(owner.bytes.clone()),
            }
        });
    }
    let expected_paths = workspace_snapshot
        .file_digests()
        .map(|(path, _)| path)
        .collect::<BTreeSet<_>>();
    let actual_paths = blobs.keys().map(String::as_str).collect::<BTreeSet<_>>();
    if actual_paths != expected_paths {
        return Err(format!(
            "active canonical materialization source membership drift: expected={} actual={}",
            expected_paths.len(),
            actual_paths.len()
        ));
    }
    Ok(crate::ClientDbActiveGenerationSourceBlobs {
        generation_id: active.generation_id.clone(),
        owners: blobs.into_values().collect(),
    })
}

fn validate_materialized_blob_identity(
    workspace_snapshot: &agent_semantic_content_identity::WorkspaceSnapshot,
    owner_path: &str,
    content_digest: &str,
) -> Result<(), String> {
    let digest = content_digest
        .strip_prefix("blake3-256:")
        .ok_or_else(|| format!("materialized owner digest is untyped: {owner_path}"))?;
    if workspace_snapshot.file_digest(owner_path) != Some(digest) {
        return Err(format!(
            "materialized owner digest is outside the canonical workspace snapshot: {owner_path}"
        ));
    }
    Ok(())
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

#[cfg(test)]
#[path = "../../tests/unit/server_source_index_generation_overlay.rs"]
mod tests;
