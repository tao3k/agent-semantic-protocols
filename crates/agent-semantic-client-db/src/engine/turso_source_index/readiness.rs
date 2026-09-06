// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-only

use super::contract::TURSO_SOURCE_INDEX_TERM_PROJECTION_VERSION;
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

pub(super) fn validate_turso_source_index_selector_projection_records(
    import: &ClientDbSourceIndexImport,
) -> Result<(), String> {
    for selector in &import.selectors {
        let record = &selector.projection_record;
        let proof = &record.proof;
        proof.validate_shape().map_err(|error| error.to_string())?;
        let canonical = proof.canonical_item_selector();
        if proof.structural_selector() != selector.selector_id.as_str()
            || proof.owner_path() != selector.owner_path.as_str()
            || proof.structural_selector() != canonical.structural_selector()
        {
            return Err(format!(
                "source-index selector projection identity mismatch: selector_id={} proof={}",
                selector.selector_id.as_str(),
                proof.structural_selector()
            ));
        }
        let range = &record.source_byte_range;
        if range.start >= range.end || record.projection_payload.is_empty() {
            return Err(format!(
                "source-index selector projection is empty or unbounded: selector_id={}",
                selector.selector_id.as_str()
            ));
        }
    }
    Ok(())
}
