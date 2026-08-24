use super::contract::TURSO_SOURCE_INDEX_TERM_PROJECTION_VERSION;
use super::core::turso_source_index_scope_row_counts;
use crate::engine::turso_statement::execute_turso_operation;

fn unix_time_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
        .min(i64::MAX as u128) as i64
}

pub(super) struct PublishTursoSourceIndexScopeRequest<'transaction, 'connection, 'value> {
    pub transaction: &'transaction turso::transaction::Transaction<'connection>,
    pub project_root: &'value str,
    pub schema_id: &'value str,
    pub schema_version: &'value str,
    pub generation_id: &'value str,
    pub file_hashes_json: &'value str,
    pub source_snapshot_json: &'value str,
    pub selector_fingerprint: &'value str,
}

pub(super) async fn publish_turso_source_index_scope(
    request: PublishTursoSourceIndexScopeRequest<'_, '_, '_>,
) -> Result<(u32, u32), String> {
    let PublishTursoSourceIndexScopeRequest {
        transaction,
        project_root,
        schema_id,
        schema_version,
        generation_id,
        file_hashes_json,
        source_snapshot_json,
        selector_fingerprint,
    } = request;
    let (effective_owner_count, effective_selector_count) = turso_source_index_scope_row_counts(
        &*transaction,
        project_root,
        schema_id,
        schema_version,
        generation_id,
    )
    .await?;
    execute_turso_operation(
        || async {
            let mut statement = transaction
                .prepare(
                    "INSERT INTO asp_source_index_scope_v1 (
                        project_root,
                        schema_id,
                        schema_version,
                        generation_id,
                        file_hashes_json,
                        source_snapshot_json,
                        selector_fingerprint,
                        owner_count,
                        selector_count,
                        updated_at_ms
                    ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)
                    ON CONFLICT(project_root, schema_id, schema_version) DO UPDATE SET
                        generation_id = excluded.generation_id,
                        file_hashes_json = excluded.file_hashes_json,
                        source_snapshot_json = excluded.source_snapshot_json,
                        selector_fingerprint = excluded.selector_fingerprint,
                        owner_count = excluded.owner_count,
                        selector_count = excluded.selector_count,
                        updated_at_ms = excluded.updated_at_ms",
                )
                .await
                .map_err(|error| error.to_string())?;
            statement
                .execute((
                    project_root,
                    schema_id,
                    schema_version,
                    generation_id,
                    file_hashes_json,
                    source_snapshot_json,
                    selector_fingerprint,
                    i64::from(effective_owner_count),
                    i64::from(effective_selector_count),
                    unix_time_ms(),
                ))
                .await
                .map_err(|error| error.to_string())
        },
        "failed to publish Turso source-index snapshot scope",
    )
    .await?;
    execute_turso_operation(
        || async {
            let mut statement = transaction
                .prepare(
                    "INSERT INTO asp_source_index_layout_v1 (
                        project_root,
                        schema_id,
                        schema_version,
                        term_projection_version,
                        token_projection_generation_id
                    ) VALUES (?1, ?2, ?3, ?4, ?5)
                    ON CONFLICT(project_root, schema_id, schema_version) DO UPDATE SET
                        term_projection_version = excluded.term_projection_version,
                        token_projection_generation_id = excluded.token_projection_generation_id",
                )
                .await
                .map_err(|error| error.to_string())?;
            statement
                .execute((
                    project_root,
                    schema_id,
                    schema_version,
                    TURSO_SOURCE_INDEX_TERM_PROJECTION_VERSION,
                    generation_id,
                ))
                .await
                .map_err(|error| error.to_string())
        },
        "failed to publish Turso source-index term projection layout",
    )
    .await?;
    Ok((effective_owner_count, effective_selector_count))
}
