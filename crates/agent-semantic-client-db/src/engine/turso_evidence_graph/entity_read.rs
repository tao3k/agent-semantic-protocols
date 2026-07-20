use std::path::Path;

use agent_semantic_content_identity::SourceSnapshotEvidence;

use super::TursoClientDbGraphEntity;
use super::artifact::graph_artifact_digest_for_snapshot;
use crate::engine::turso::connect_turso_client_db;
use crate::engine::turso_statement::run_turso_operation_with_lock_retry;

pub(super) fn decode_turso_graph_entity_row(
    row: &turso::Row,
) -> Result<TursoClientDbGraphEntity, String> {
    let query_keys_json = row
        .get::<String>(8)
        .map_err(|error| format!("failed to read Turso graph query keys: {error}"))?;
    let query_keys = serde_json::from_str::<Vec<String>>(&query_keys_json)
        .map_err(|error| format!("failed to decode Turso graph query keys: {error}"))?;
    Ok(TursoClientDbGraphEntity {
        id: row
            .get::<String>(0)
            .map_err(|error| format!("failed to read Turso graph entity id: {error}"))?,
        kind: row
            .get::<String>(1)
            .map_err(|error| format!("failed to read Turso graph entity kind: {error}"))?,
        semantic_kind: row
            .get::<Option<String>>(2)
            .map_err(|error| format!("failed to read Turso graph entity semantic kind: {error}"))?,
        label: row
            .get::<String>(3)
            .map_err(|error| format!("failed to read Turso graph entity label: {error}"))?,
        selector: row
            .get::<Option<String>>(4)
            .map_err(|error| format!("failed to read Turso graph entity selector: {error}"))?,
        path: row
            .get::<Option<String>>(5)
            .map_err(|error| format!("failed to read Turso graph entity path: {error}"))?,
        language_id: row
            .get::<Option<String>>(6)
            .map_err(|error| format!("failed to read Turso graph entity language id: {error}"))?,
        provider_id: row
            .get::<Option<String>>(7)
            .map_err(|error| format!("failed to read Turso graph entity provider id: {error}"))?,
        query_keys,
    })
}

/// List EvidenceGraph entities from the Turso DB Engine file.
pub async fn list_turso_graph_entities(
    db_path: &Path,
    source_snapshot: &SourceSnapshotEvidence,
    kind: Option<&str>,
    limit: u32,
) -> Result<Vec<TursoClientDbGraphEntity>, String> {
    if limit == 0 {
        return Ok(Vec::new());
    }
    let connection = connect_turso_client_db(db_path).await?;
    let graph_artifact_digest = graph_artifact_digest_for_snapshot(source_snapshot);
    let (sql, parameter): (&str, Option<&str>) = if let Some(kind) = kind {
        (
            "SELECT id, kind, semantic_kind, label, selector, path, language_id, provider_id, query_keys_json
             FROM asp_graph_artifact_entity
             WHERE graph_artifact_digest = ?1 AND kind = ?2
             ORDER BY id
             LIMIT ?3",
            Some(kind),
        )
    } else {
        (
            "SELECT id, kind, semantic_kind, label, selector, path, language_id, provider_id, query_keys_json
             FROM asp_graph_artifact_entity
             WHERE graph_artifact_digest = ?1
             ORDER BY id
             LIMIT ?2",
            None,
        )
    };
    let mut rows = if let Some(kind) = parameter {
        run_turso_operation_with_lock_retry(
            || async {
                connection
                    .query(sql, (graph_artifact_digest.as_str(), kind, limit))
                    .await
                    .map_err(|error| error.to_string())
            },
            "failed to query Turso graph entities",
        )
        .await?
    } else {
        run_turso_operation_with_lock_retry(
            || async {
                connection
                    .query(sql, (graph_artifact_digest.as_str(), limit))
                    .await
                    .map_err(|error| error.to_string())
            },
            "failed to query Turso graph entities",
        )
        .await?
    };
    let mut entities = Vec::new();
    while let Some(row) = rows
        .next()
        .await
        .map_err(|error| format!("failed to read Turso graph entity row: {error}"))?
    {
        entities.push(decode_turso_graph_entity_row(&row)?);
    }
    Ok(entities)
}
