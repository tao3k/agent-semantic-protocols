use std::path::Path;

use crate::engine::turso::connect_turso_client_db;
use crate::engine::turso_statement::run_turso_operation;

pub(in crate::engine) async fn persist_exact_selector_projection_v1(
    db_path: &Path,
    key: &agent_semantic_content_identity::exact_selector_cache::ExactSelectorMerkleLookupKeyV1<'_>,
    record: &agent_semantic_content_identity::exact_selector_cache::ExactSelectorProjectionRecordV1,
) -> Result<(), String> {
    agent_semantic_content_identity::exact_selector_cache::ValidatedExactSelectorProjectionV1::hydrate(
        record.clone(),
        key,
    )
    .map_err(|miss| format!("refusing invalid exact-selector projection write: {miss:?}"))?;
    let record_json = serde_json::to_string(record)
        .map_err(|error| format!("failed to encode Turso exact-selector record: {error}"))?;
    let connection = match connect_turso_client_db(db_path).await {
        Ok(connection) => connection,
        Err(error) if crate::engine::turso_lock_policy::is_turso_lock_error(&error) => {
            return Ok(());
        }
        Err(error) => return Err(error),
    };
    if let Err(error) = super::core::ensure_turso_source_index_schema(&connection).await {
        if crate::engine::turso_lock_policy::is_turso_lock_error(&error) {
            return Ok(());
        }
        return Err(error);
    }
    let projection_mode = projection_mode_name(key.projection_mode);
    if let Err(error) = run_turso_operation(
        || async {
            connection
                .execute(
                    "INSERT OR REPLACE INTO asp_exact_selector_projection_v1 (
                        language_id,
                        workspace_root_digest,
                        owner_path,
                        owner_subtree_digest,
                        source_blob_digest,
                        parser_identity_digest,
                        query_pack_digest,
                        structural_selector,
                        projection_mode,
                        record_json
                     ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
                    (
                        key.language_id,
                        key.workspace_root_digest.as_str(),
                        key.owner_path,
                        key.owner_subtree_digest.as_str(),
                        key.source_blob_digest.as_str(),
                        key.parser_identity_digest.as_str(),
                        key.query_pack_digest.as_str(),
                        key.structural_selector,
                        projection_mode,
                        record_json.as_str(),
                    ),
                )
                .await
                .map_err(|error| error.to_string())
        },
        "failed to persist Turso exact-selector projection",
    )
    .await
    {
        if crate::engine::turso_lock_policy::is_turso_lock_error(&error) {
            return Ok(());
        }
        return Err(error);
    }
    Ok(())
}

pub(in crate::engine) async fn lookup_exact_selector_projection_v1(
    db_path: &Path,
    key: &agent_semantic_content_identity::exact_selector_cache::ExactSelectorMerkleLookupKeyV1<'_>,
) -> Result<
    Option<
        agent_semantic_content_identity::exact_selector_cache::ValidatedExactSelectorProjectionV1,
    >,
    String,
> {
    if !db_path.exists() {
        return Ok(None);
    }
    let connection = match crate::engine::turso::connect_turso_client_db_read_only(db_path).await {
        Ok(connection) => connection,
        Err(error) if crate::engine::turso_lock_policy::is_turso_lock_error(&error) => {
            return Ok(None);
        }
        Err(error) => return Err(error),
    };
    let table_exists = match crate::engine::turso::turso_table_exists(
        &connection,
        "asp_exact_selector_projection_v1",
    )
    .await
    {
        Ok(table_exists) => table_exists,
        Err(error) if crate::engine::turso_lock_policy::is_turso_lock_error(&error) => {
            return Ok(None);
        }
        Err(error) => return Err(error),
    };
    if !table_exists {
        return Ok(None);
    }
    let projection_mode = projection_mode_name(key.projection_mode);
    let mut rows = run_turso_operation(
        || async {
            connection
                .query(
                    "SELECT record_json
                     FROM asp_exact_selector_projection_v1
                     WHERE language_id = ?1
                       AND workspace_root_digest = ?2
                       AND owner_path = ?3
                       AND owner_subtree_digest = ?4
                       AND source_blob_digest = ?5
                       AND parser_identity_digest = ?6
                       AND query_pack_digest = ?7
                       AND structural_selector = ?8
                       AND projection_mode = ?9
                     LIMIT 1",
                    (
                        key.language_id,
                        key.workspace_root_digest.as_str(),
                        key.owner_path,
                        key.owner_subtree_digest.as_str(),
                        key.source_blob_digest.as_str(),
                        key.parser_identity_digest.as_str(),
                        key.query_pack_digest.as_str(),
                        key.structural_selector,
                        projection_mode,
                    ),
                )
                .await
                .map_err(|error| error.to_string())
        },
        "failed to query Turso exact-selector projection",
    )
    .await?;
    let next_row = rows
        .next()
        .await
        .map_err(|error| format!("failed to read Turso exact-selector projection: {error}"));
    let Some(row) = (match next_row {
        Ok(row) => row,
        Err(error) if crate::engine::turso_lock_policy::is_turso_lock_error(&error) => {
            return Ok(None);
        }
        Err(error) => return Err(error),
    }) else {
        return Ok(None);
    };
    let record_json = row
        .get::<String>(0)
        .map_err(|error| format!("failed to read Turso exact-selector record JSON: {error}"))?;
    let record = match serde_json::from_str(&record_json) {
        Ok(record) => record,
        Err(_) => return Ok(None),
    };
    let validated = match agent_semantic_content_identity::exact_selector_cache::ValidatedExactSelectorProjectionV1::hydrate(
        record,
        key,
    ) {
        Ok(validated) => validated,
        Err(_) => return Ok(None),
    };
    Ok(Some(validated))
}

fn projection_mode_name(
    mode: agent_semantic_content_identity::exact_selector_merkle::ExactProjectionModeV1,
) -> &'static str {
    match mode {
        agent_semantic_content_identity::exact_selector_merkle::ExactProjectionModeV1::Code => {
            "code"
        }
        agent_semantic_content_identity::exact_selector_merkle::ExactProjectionModeV1::Skeleton => {
            "skeleton"
        }
        agent_semantic_content_identity::exact_selector_merkle::ExactProjectionModeV1::Names => {
            "names"
        }
        agent_semantic_content_identity::exact_selector_merkle::ExactProjectionModeV1::Verbatim => {
            "verbatim"
        }
    }
}
