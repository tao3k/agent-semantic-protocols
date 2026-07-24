use super::core::TURSO_SOURCE_INDEX_TERM_PROJECTION_VERSION;
use crate::ClientDbSourceIndexImport;
use crate::engine::turso_statement::run_turso_operation;

pub(super) async fn turso_source_index_projection_ready(
    connection: &turso::Connection,
    project_root: &str,
    schema_id: &str,
    schema_version: &str,
) -> Result<bool, String> {
    let mut rows = run_turso_operation(
        || async {
            connection
                .query(
                    "SELECT layout.term_projection_version,
                            layout.token_projection_generation_id,
                            scope.generation_id
                     FROM asp_source_index_layout_v1 AS layout
                     JOIN asp_source_index_scope_v1 AS scope
                       ON scope.project_root = layout.project_root
                      AND scope.schema_id = layout.schema_id
                      AND scope.schema_version = layout.schema_version
                     WHERE layout.project_root = ?1
                       AND layout.schema_id = ?2
                       AND layout.schema_version = ?3
                     LIMIT 1",
                    (project_root, schema_id, schema_version),
                )
                .await
                .map_err(|error| error.to_string())
        },
        "failed to inspect Turso source-index term projection scope",
    )
    .await?;
    let Some(row) = rows.next().await.map_err(|error| {
        format!("failed to read Turso source-index term projection scope: {error}")
    })?
    else {
        return Ok(false);
    };
    let projection_version = row.get::<i64>(0).map_err(|error| {
        format!("failed to decode Turso source-index term projection scope: {error}")
    })?;
    let projection_generation_id = row.get::<String>(1).map_err(|error| {
        format!("failed to decode Turso source-index token projection generation: {error}")
    })?;
    let scope_generation_id = row.get::<String>(2).map_err(|error| {
        format!("failed to decode Turso source-index scope generation: {error}")
    })?;
    if projection_version != TURSO_SOURCE_INDEX_TERM_PROJECTION_VERSION
        || projection_generation_id != scope_generation_id
    {
        return Ok(false);
    }
    let mut token_rows = run_turso_operation(
        || async {
            connection
                .query(
                    "SELECT 1
                     FROM asp_source_index_token_owner_v1
                     WHERE project_root = ?1
                       AND schema_id = ?2
                       AND schema_version = ?3
                       AND generation_id = ?4
                     LIMIT 1",
                    (
                        project_root,
                        schema_id,
                        schema_version,
                        scope_generation_id.as_str(),
                    ),
                )
                .await
                .map_err(|error| error.to_string())
        },
        "failed to inspect Turso source-index token projection",
    )
    .await?;
    token_rows
        .next()
        .await
        .map(|row| row.is_some())
        .map_err(|error| format!("failed to read Turso source-index token projection: {error}"))
}

pub(super) fn validate_turso_source_index_selector_payload_proofs(
    import: &ClientDbSourceIndexImport,
) -> Result<(), String> {
    for selector in &import.selectors {
        if let Some(proof) = &selector.payload_proof
            && proof.structural_selector != selector.selector_id
        {
            return Err(format!(
                "source-index selector payload proof selector mismatch: selector_id={} proof={}",
                selector.selector_id, proof.structural_selector
            ));
        }
    }
    Ok(())
}
