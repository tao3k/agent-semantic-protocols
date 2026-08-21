use super::membership::{
    stage_turso_source_index_import_membership, turso_source_index_import_membership,
    turso_source_index_membership_changes,
};
use super::projection::{
    refresh_turso_source_index_posting_projection, refresh_turso_source_index_selector_projection,
    write_turso_source_index_owner_rows,
};
use super::readiness::turso_source_index_projection_ready;
use super::trace::{
    source_index_db_trace, source_index_db_trace_membership_changes,
    source_index_db_trace_posting_projection, source_index_db_trace_row_counts,
};
use super::transaction::TursoSourceIndexWriteStats;
use crate::ClientDbSourceIndexImport;
use crate::engine::turso_statement::execute_turso_operation;

pub(super) async fn write_turso_source_index_rows(
    connection: &mut turso::Connection,
    import: &ClientDbSourceIndexImport,
    materialization: &crate::runtime_server_workspace::WorkspaceCanonicalMaterialization,
    membership_change_set: &crate::source_index::ClientDbSourceIndexMembershipChangeSet,
    project_root: &str,
    file_hashes_json: &str,
    source_snapshot_json: &str,
) -> Result<(
    TursoSourceIndexWriteStats,
    crate::runtime_server_workspace::WorkspaceCanonicalMaterialization,
), String> {
    let cold_write_started = std::time::Instant::now();
    super::readiness::validate_turso_source_index_selector_projection_records(import)?;
    let imported_membership = turso_source_index_import_membership(import)?;
    let transaction = connection
        .transaction_with_behavior(turso::transaction::TransactionBehavior::Immediate)
        .await
        .map_err(|error| format!("failed to begin Turso source-index transaction: {error}"))?;
    let write_result = async {
        let connection = &*transaction;

        let projection_ready = turso_source_index_projection_ready(
            connection,
            project_root,
            import.schema_id.as_str(),
            import.schema_version.as_str(),
        )
        .await?;
        let overlay_base_generation_id = match membership_change_set {
            crate::source_index::ClientDbSourceIndexMembershipChangeSet::FullSnapshot => None,
            crate::source_index::ClientDbSourceIndexMembershipChangeSet::MerkleOverlay {
                base_generation_id,
                ..
            } => Some(base_generation_id.as_str()),
        };
        let (_membership_changed_owner_paths, removed_owner_paths) = match membership_change_set {
            crate::source_index::ClientDbSourceIndexMembershipChangeSet::FullSnapshot => {
                stage_turso_source_index_import_membership(connection, file_hashes_json).await?;
                turso_source_index_membership_changes(
                    connection,
                    project_root,
                    import.schema_id.as_str(),
                    import.schema_version.as_str(),
                    projection_ready,
                )
                .await?
            }
            crate::source_index::ClientDbSourceIndexMembershipChangeSet::MerkleOverlay {
                base_generation_id: _,
                changed_owner_paths,
                removed_owner_paths,
            } => {
                if !projection_ready {
                    return Err(
                        "source-index Merkle overlay requires a published base projection"
                            .to_string(),
                    );
                }
                let source_snapshot: agent_semantic_content_identity::SourceSnapshotEvidence =
                    serde_json::from_str(source_snapshot_json).map_err(|error| {
                        format!(
                            "failed to decode Turso source-index Merkle overlay evidence: {error}"
                        )
                    })?;
                super::membership::validate_turso_source_index_overlay_base(
                    connection,
                    project_root,
                    import.schema_id.as_str(),
                    import.schema_version.as_str(),
                    source_snapshot
                        .base_root_digest
                        .as_deref()
                        .expect("Merkle overlay evidence validated before write"),
                )
                .await?;
                (
                    changed_owner_paths
                        .iter()
                        .map(|path| path.as_str().to_string())
                        .collect(),
                    removed_owner_paths
                        .iter()
                        .map(|path| path.as_str().to_string())
                        .collect(),
                )
            }
        };
        let (
            effective_materialization,
            effective_file_hashes_json,
            effective_source_snapshot_json,
            effective_selector_fingerprint,
        ) = match membership_change_set {
                crate::source_index::ClientDbSourceIndexMembershipChangeSet::FullSnapshot => (
                    materialization.clone(),
                    file_hashes_json.to_string(),
                    source_snapshot_json.to_string(),
                    None,
                ),
                crate::source_index::ClientDbSourceIndexMembershipChangeSet::MerkleOverlay {
                    changed_owner_paths,
                    removed_owner_paths,
                    ..
                } => {
                    let active = super::generation_snapshot::load_turso_source_index_generation_snapshot(
                        connection,
                        project_root,
                        import.schema_id.as_str(),
                        import.schema_version.as_str(),
                    )
                    .await?
                    .ok_or_else(|| {
                        "source-index Merkle overlay requires an active canonical generation"
                            .to_string()
                    })?;
                    let active_blobs = super::generation_snapshot::load_turso_source_index_generation_blobs_on_connection(
                        connection,
                        project_root,
                        import.schema_id.as_str(),
                        import.schema_version.as_str(),
                        &active,
                    )
                    .await?;
                    let changed_owner_paths = changed_owner_paths
                        .iter()
                        .map(|path| path.as_str().to_string())
                        .collect::<std::collections::BTreeSet<_>>();
                    let removed_owner_paths = removed_owner_paths
                        .iter()
                        .map(|path| path.as_str().to_string())
                        .collect::<std::collections::BTreeSet<_>>();
                    let full_import = crate::overlay_active_source_index_import(
                        &active,
                        &active_blobs,
                        import,
                        &changed_owner_paths,
                        &removed_owner_paths,
                    )?;
                    let file_hashes_by_path = full_import
                        .file_hashes
                        .iter()
                        .map(|record| (record.path.as_str(), record.sha256.as_str()))
                        .collect::<std::collections::BTreeMap<_, _>>();
                    for owner in &full_import.owners {
                        let owner_path = owner.owner_path.as_str();
                        if !file_hashes_by_path.contains_key(owner_path) {
                            return Err(format!(
                                "complete source-index successor is missing owner digest: ownerPath={owner_path}"
                            ));
                        }
                    }
                    let workspace_snapshot =
                        agent_semantic_content_identity::WorkspaceSnapshot::from_file_bytes(
                            full_import.source_blobs.iter(),
                        );
                    workspace_snapshot.validate()?;
                    let partial_source_snapshot: agent_semantic_content_identity::SourceSnapshotEvidence =
                        serde_json::from_str(source_snapshot_json).map_err(|error| {
                            format!(
                                "failed to decode partial source-index overlay evidence: {error}"
                            )
                        })?;
                    let mut successor_source_snapshot = workspace_snapshot.evidence(
                        partial_source_snapshot.source_kind,
                        partial_source_snapshot.provider_digest,
                    );
                    successor_source_snapshot.base_root_digest =
                        Some(active.source_snapshot.root_digest.clone());
                    successor_source_snapshot.dirty_paths_digest =
                        partial_source_snapshot.dirty_paths_digest;
                    let full_materialization =
                crate::runtime_server_workspace::WorkspaceCanonicalMaterialization::from_source_index(
            materialization.workspace_identity.clone(),
                            &successor_source_snapshot,
                            &full_import,
                            &full_import.source_blobs,
                            materialization.project_resolutions.clone(),
                        )?;
                    let selector_fingerprint =
                        super::core::turso_source_index_selector_fingerprint(&full_import)?;
                    (
                        full_materialization,
                        serde_json::to_string(&full_import.file_hashes).map_err(|error| {
                            format!(
                                "failed to encode complete source-index successor hashes: {error}"
                            )
                        })?,
                        serde_json::to_string(&successor_source_snapshot).map_err(|error| {
                            format!(
                                "failed to encode complete source-index successor evidence: {error}"
                            )
                        })?,
                        Some(selector_fingerprint),
                    )
                }
            };
        let materialization = &effective_materialization;
        let file_hashes_json = effective_file_hashes_json.as_str();
        let source_snapshot_json = effective_source_snapshot_json.as_str();
        let materialization_digest = materialization.generation_identity_digest()?;
        let physical_generation_id = format!(
            "source-index-{}",
            materialization_digest
                .strip_prefix("blake3-256:")
                .unwrap_or(materialization_digest.as_str())
        );
        let prepared = super::prepare::prepare_turso_source_index_rows(
            import,
            &imported_membership,
            &physical_generation_id,
        )
        .await?;
        let physical_generation_id = prepared.physical_generation_id.as_str();
        if let Some(base_generation_id) = overlay_base_generation_id {
            super::generation_clone::clone_active_generation_rows(
                connection,
                project_root,
                import.schema_id.as_str(),
                import.schema_version.as_str(),
                base_generation_id,
                physical_generation_id,
            )
            .await?;
        }
        let selector_fingerprint =
            effective_selector_fingerprint.unwrap_or(prepared.selector_fingerprint);
        let changed_owner_paths = prepared.changed_owner_paths;
        let changed_owner_rows = prepared.changed_owner_rows;
        let changed_selector_rows = prepared.changed_selector_rows;
        let semantic_term_count = prepared.semantic_term_count;
        let membership_trace_stage = match membership_change_set {
            crate::source_index::ClientDbSourceIndexMembershipChangeSet::FullSnapshot => {
                "snapshot-membership-joined"
            }
            crate::source_index::ClientDbSourceIndexMembershipChangeSet::MerkleOverlay {
                ..
            } => "merkle-frontier-prepared",
        };
        source_index_db_trace_membership_changes(
            cold_write_started,
            membership_trace_stage,
            changed_owner_paths.len(),
            removed_owner_paths.len(),
        );
        source_index_db_trace_row_counts(
            "snapshot-rows-built",
            cold_write_started,
            changed_owner_rows.len(),
            semantic_term_count,
        );
        super::source_blob::write_source_index_blobs(connection, import).await?;
        write_turso_source_index_owner_rows(
            connection,
            &changed_owner_rows,
            project_root,
            import.schema_id.as_str(),
            import.schema_version.as_str(),
            physical_generation_id,
        )
        .await?;
        source_index_db_trace("snapshot-owner-rows-written", cold_write_started);
        let removed_owner_paths_json =
            serde_json::to_string(&removed_owner_paths).map_err(|error| {
                format!("failed to encode Turso source-index removed owners: {error}")
            })?;
        execute_turso_operation(
            || async {
                connection
                    .execute(
                        "DELETE FROM asp_source_index_owner_v1
                         WHERE project_root = ?1
                           AND schema_id = ?2
                           AND schema_version = ?3
                           AND generation_id = ?4
                           AND owner_path IN (SELECT value FROM json_each(?5))",
                        (
                            project_root,
                            import.schema_id.as_str(),
                            import.schema_version.as_str(),
                            physical_generation_id,
                            removed_owner_paths_json.as_str(),
                        ),
                    )
                    .await
                    .map_err(|error| error.to_string())
            },
            "failed to delete Turso source-index snapshot owners",
        )
        .await?;
        source_index_db_trace("snapshot-owners-pruned", cold_write_started);
        let mut projection_owner_paths = changed_owner_paths.iter().cloned().collect::<Vec<_>>();
        projection_owner_paths.extend(removed_owner_paths.iter().cloned());
        projection_owner_paths.sort();
        projection_owner_paths.dedup();
        refresh_turso_source_index_selector_projection(
            connection,
            project_root,
            import.schema_id.as_str(),
            import.schema_version.as_str(),
            physical_generation_id,
            &projection_owner_paths,
            &changed_selector_rows,
        )
        .await?;
        source_index_db_trace("snapshot-selector-rows-written", cold_write_started);
        super::relation::refresh_turso_source_index_relation_projection(
            connection,
            import,
            project_root,
            import.schema_id.as_str(),
            import.schema_version.as_str(),
            physical_generation_id,
            &projection_owner_paths,
        )
        .await?;
        source_index_db_trace("snapshot-relation-rows-written", cold_write_started);
        let posting_count = refresh_turso_source_index_posting_projection(
            connection,
            project_root,
            import.schema_id.as_str(),
            import.schema_version.as_str(),
            physical_generation_id,
            &projection_owner_paths,
            &changed_owner_rows,
        )
        .await?;
        source_index_db_trace_posting_projection(cold_write_started, posting_count);
        super::materialization::persist_workspace_generation_materialization(
            connection,
            project_root,
            import.schema_id.as_str(),
            import.schema_version.as_str(),
            physical_generation_id,
            materialization,
        )
        .await?;
        source_index_db_trace("snapshot-materialization-written", cold_write_started);
        super::publish::publish_turso_source_index_scope(
            super::publish::PublishTursoSourceIndexScopeRequest {
                transaction: &transaction,
                project_root,
                schema_id: import.schema_id.as_str(),
                schema_version: import.schema_version.as_str(),
                generation_id: physical_generation_id,
                file_hashes_json,
                source_snapshot_json,
                selector_fingerprint: selector_fingerprint.as_str(),
            },
        )
        .await?;
        source_index_db_trace("snapshot-scope-published", cold_write_started);
        let stats = TursoSourceIndexWriteStats {
            physical_generation_id: physical_generation_id.to_string(),
            changed_owner_count: changed_owner_paths.len().min(u32::MAX as usize) as u32,
            removed_owner_count: removed_owner_paths.len().min(u32::MAX as usize) as u32,
            posting_write_count: posting_count.min(u32::MAX as usize) as u32,
        };
        Ok((stats, effective_materialization))
    }
    .await;

    match write_result {
        Ok(stats) => {
            transaction.commit().await.map_err(|error| {
                format!("failed to commit Turso source-index transaction: {error}")
            })?;
            source_index_db_trace("transaction-committed", cold_write_started);
            Ok(stats)
        }
        Err(write_error) => match transaction.rollback().await {
            Ok(()) => Err(write_error),
            Err(rollback_error) => Err(format!("{write_error}; rollbackError={rollback_error}")),
        },
    }
}
