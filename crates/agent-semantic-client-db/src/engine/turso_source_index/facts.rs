use super::membership::{
    retire_turso_source_index_precanonical_tables, stage_turso_source_index_import_membership,
    turso_source_index_import_membership, turso_source_index_membership_changes,
};
use super::projection::{
    refresh_turso_source_index_posting_projection, write_turso_source_index_owner_rows,
};
use super::readiness::{
    turso_source_index_precanonical_storage_exists, turso_source_index_projection_ready,
    validate_turso_source_index_selector_payload_proofs,
};
use super::trace::{
    source_index_db_trace, source_index_db_trace_membership_changes,
    source_index_db_trace_posting_projection, source_index_db_trace_row_counts,
};
use super::transaction::{
    TURSO_SOURCE_INDEX_COLD_WRITE_BUDGET, TursoSourceIndexWriteStats,
    return_turso_source_index_write_failure, rollback_turso_source_index_transaction,
};
use crate::ClientDbSourceIndexImport;
use crate::engine::turso_statement::{
    execute_turso_operation_with_lock_retry, execute_turso_statement_with_lock_retry,
    run_turso_operation_with_lock_retry,
};

pub(super) async fn write_turso_source_index_rows(
    connection: &turso::Connection,
    import: &ClientDbSourceIndexImport,
    project_root: &str,
    file_hashes_json: &str,
) -> Result<TursoSourceIndexWriteStats, String> {
    let cold_write_started = std::time::Instant::now();
    let transaction_started = std::sync::atomic::AtomicBool::new(false);
    validate_turso_source_index_selector_payload_proofs(import)?;
    let imported_membership = turso_source_index_import_membership(import)?;
    let retire_precanonical_storage =
        turso_source_index_precanonical_storage_exists(connection).await?;
    let write_result = tokio::time::timeout(TURSO_SOURCE_INDEX_COLD_WRITE_BUDGET, async {
        execute_turso_statement_with_lock_retry(
            connection,
            "BEGIN IMMEDIATE",
            "failed to begin Turso source-index transaction",
        )
        .await?;
        transaction_started.store(true, std::sync::atomic::Ordering::Release);

        let projection_ready = turso_source_index_projection_ready(
            connection,
            project_root,
            import.schema_id.as_str(),
            import.schema_version.as_str(),
        )
        .await?;
        stage_turso_source_index_import_membership(connection, file_hashes_json).await?;
        let (membership_changed_owner_paths, removed_owner_paths) =
            turso_source_index_membership_changes(
                connection,
                project_root,
                import.schema_id.as_str(),
                import.schema_version.as_str(),
                projection_ready,
            )
            .await?;
        let prepared = super::prepare::prepare_turso_source_index_rows(
            connection,
            import,
            project_root,
            &imported_membership,
            membership_changed_owner_paths,
            projection_ready,
        )
        .await?;
        let physical_generation_id = prepared.physical_generation_id.as_str();
        let selector_fingerprint = prepared.selector_fingerprint;
        let changed_owner_paths = prepared.changed_owner_paths;
        let changed_owner_rows = prepared.changed_owner_rows;
        let semantic_term_count = prepared.semantic_term_count;
        source_index_db_trace_membership_changes(
            cold_write_started,
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
        execute_turso_operation_with_lock_retry(
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
        execute_turso_statement_with_lock_retry(
            connection,
            "DROP TABLE IF EXISTS asp_source_index_term_v1",
            "failed to retire Turso source-index term projection",
        )
        .await?;
        execute_turso_statement_with_lock_retry(
            connection,
            "DROP TABLE IF EXISTS asp_source_index_token_v1",
            "failed to retire Turso source-index JSON token dictionary",
        )
        .await?;
        source_index_db_trace("legacy-term-projection-retired", cold_write_started);

        super::publish::publish_turso_source_index_scope(
            connection,
            project_root,
            import.schema_id.as_str(),
            import.schema_version.as_str(),
            physical_generation_id,
            file_hashes_json,
            selector_fingerprint.as_str(),
        )
        .await?;
        if retire_precanonical_storage {
            retire_turso_source_index_precanonical_tables(connection).await?;
        }
        source_index_db_trace("snapshot-scope-published", cold_write_started);
        Ok(TursoSourceIndexWriteStats {
            physical_generation_id: physical_generation_id.to_string(),
            changed_owner_count: changed_owner_paths.len().min(u32::MAX as usize) as u32,
            removed_owner_count: removed_owner_paths.len().min(u32::MAX as usize) as u32,
            posting_write_count: posting_count.min(u32::MAX as usize) as u32,
        })
    })
    .await;

    match write_result {
        Ok(Ok(stats)) => {
            let commit_result = execute_turso_statement_with_lock_retry(
                connection,
                "COMMIT",
                "failed to commit Turso source-index transaction",
            )
            .await;
            if let Err(error) = commit_result {
                return rollback_turso_source_index_transaction(connection, error).await;
            }
            source_index_db_trace("transaction-committed", cold_write_started);
            let mut checkpoint_rows = run_turso_operation_with_lock_retry(
                || async {
                    connection
                        .query("PRAGMA wal_checkpoint(TRUNCATE)", ())
                        .await
                        .map_err(|error| error.to_string())
                },
                "failed to checkpoint committed Turso source-index snapshot",
            )
            .await?;
        while checkpoint_rows
            .next()
            .await
            .map_err(|error| format!("failed to read Turso source-index checkpoint: {error}"))?
            .is_some()
        {}
        source_index_db_trace("wal-checkpoint-completed", cold_write_started);
        Ok(stats)
        }
        Ok(Err(error)) => {
            return_turso_source_index_write_failure(
                connection,
                transaction_started.load(std::sync::atomic::Ordering::Acquire),
                error,
            )
            .await
        }
        Err(_) => {
            return_turso_source_index_write_failure(
                connection,
                transaction_started.load(std::sync::atomic::Ordering::Acquire),
                format!(
                    "source-index cold-write budget exhausted: budgetMs={} elapsedMs={} owners={} selectors={}",
                    TURSO_SOURCE_INDEX_COLD_WRITE_BUDGET.as_millis(),
                    cold_write_started.elapsed().as_millis(),
                    import.owners.len(),
                    import.selectors.len(),
                ),
            )
            .await
        }
    }
}
