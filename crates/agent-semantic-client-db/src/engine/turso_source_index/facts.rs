use super::membership::{
    stage_turso_source_index_import_membership, turso_source_index_import_membership,
    turso_source_index_membership_changes,
};
use super::projection::{
    refresh_turso_source_index_posting_projection, write_turso_source_index_owner_rows,
};
use super::readiness::{
    turso_source_index_projection_ready, validate_turso_source_index_selector_payload_proofs,
};
use super::trace::{
    source_index_db_trace, source_index_db_trace_membership_changes,
    source_index_db_trace_posting_projection, source_index_db_trace_row_counts,
};
use super::transaction::{TURSO_SOURCE_INDEX_COLD_WRITE_BUDGET, TursoSourceIndexWriteStats};
use crate::ClientDbSourceIndexImport;
use crate::engine::turso_statement::execute_turso_operation;

pub(super) async fn write_turso_source_index_rows(
    connection: &turso::Connection,
    import: &ClientDbSourceIndexImport,
    membership_change_set: &crate::source_index::ClientDbSourceIndexMembershipChangeSet,
    project_root: &str,
    file_hashes_json: &str,
    source_snapshot_json: &str,
) -> Result<TursoSourceIndexWriteStats, String> {
    let cold_write_started = std::time::Instant::now();
    validate_turso_source_index_selector_payload_proofs(import)?;
    let imported_membership = turso_source_index_import_membership(import)?;
    let transaction = turso::transaction::Transaction::new_unchecked(
        connection,
        turso::transaction::TransactionBehavior::Immediate,
    )
    .await
    .map_err(|error| format!("failed to begin Turso source-index transaction: {error}"))?;
    let write_result = tokio::time::timeout(TURSO_SOURCE_INDEX_COLD_WRITE_BUDGET, async {
        let connection = &*transaction;

        let projection_ready = turso_source_index_projection_ready(
            connection,
            project_root,
            import.schema_id.as_str(),
            import.schema_version.as_str(),
        )
        .await?;
        let (membership_changed_owner_paths, removed_owner_paths) = match membership_change_set {
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
        let prepared = super::prepare::prepare_turso_source_index_rows(
            connection,
            import,
            project_root,
            &imported_membership,
            membership_changed_owner_paths,
            projection_ready,
            matches!(
                membership_change_set,
                crate::source_index::ClientDbSourceIndexMembershipChangeSet::MerkleOverlay { .. }
            ),
        )
        .await?;
        let physical_generation_id = prepared.physical_generation_id.as_str();
        let selector_fingerprint = prepared.selector_fingerprint;
        let changed_owner_paths = prepared.changed_owner_paths;
        let changed_owner_rows = prepared.changed_owner_rows;
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
        super::publish::publish_turso_source_index_scope(
            connection,
            project_root,
            import.schema_id.as_str(),
            import.schema_version.as_str(),
            physical_generation_id,
            file_hashes_json,
            source_snapshot_json,
            selector_fingerprint.as_str(),
        )
        .await?;
        source_index_db_trace("snapshot-scope-published", cold_write_started);
        let stats = TursoSourceIndexWriteStats {
            physical_generation_id: physical_generation_id.to_string(),
            changed_owner_count: changed_owner_paths.len().min(u32::MAX as usize) as u32,
            removed_owner_count: removed_owner_paths.len().min(u32::MAX as usize) as u32,
            posting_write_count: posting_count.min(u32::MAX as usize) as u32,
        };
        Ok(stats)
    })
    .await;

    match write_result {
        Ok(Ok(stats)) => {
            transaction.commit().await.map_err(|error| {
                format!("failed to commit Turso source-index transaction: {error}")
            })?;
            source_index_db_trace("transaction-committed", cold_write_started);
            Ok(stats)
        }
        Ok(Err(write_error)) => match transaction.rollback().await {
            Ok(()) => Err(write_error),
            Err(rollback_error) => Err(format!("{write_error}; rollbackError={rollback_error}")),
        },
        Err(_) => {
            let write_error = format!(
                "source-index cold-write budget exhausted: budgetMs={} elapsedMs={} owners={} selectors={}",
                TURSO_SOURCE_INDEX_COLD_WRITE_BUDGET.as_millis(),
                cold_write_started.elapsed().as_millis(),
                import.owners.len(),
                import.selectors.len(),
            );
            match transaction.rollback().await {
                Ok(()) => Err(write_error),
                Err(rollback_error) => {
                    Err(format!("{write_error}; rollbackError={rollback_error}"))
                }
            }
        }
    }
}
