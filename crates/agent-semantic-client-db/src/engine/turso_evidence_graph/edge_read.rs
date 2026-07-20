use std::path::Path;

use agent_semantic_content_identity::SourceSnapshotEvidence;

use super::TursoClientDbGraphEdge;
use super::artifact::graph_artifact_digest_for_snapshot;
use crate::engine::turso::connect_turso_client_db;
use crate::engine::turso_statement::run_turso_operation_with_lock_retry;

/// List EvidenceGraph edges from the Turso DB Engine file.
pub async fn list_turso_graph_edges(
    db_path: &Path,
    source_snapshot: &SourceSnapshotEvidence,
    kind: Option<&str>,
    limit: u32,
) -> Result<Vec<TursoClientDbGraphEdge>, String> {
    if limit == 0 {
        return Ok(Vec::new());
    }
    let connection = connect_turso_client_db(db_path).await?;
    let graph_artifact_digest = graph_artifact_digest_for_snapshot(source_snapshot);
    let (sql, parameter): (&str, Option<&str>) = if let Some(kind) = kind {
        (
            "SELECT from_id, to_id, kind
             FROM asp_graph_artifact_edge
             WHERE graph_artifact_digest = ?1 AND kind = ?2
             ORDER BY from_id, to_id, kind
             LIMIT ?3",
            Some(kind),
        )
    } else {
        (
            "SELECT from_id, to_id, kind
             FROM asp_graph_artifact_edge
             WHERE graph_artifact_digest = ?1
             ORDER BY from_id, to_id, kind
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
            "failed to query Turso graph edges",
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
            "failed to query Turso graph edges",
        )
        .await?
    };
    let mut edges = Vec::new();
    while let Some(row) = rows
        .next()
        .await
        .map_err(|error| format!("failed to read Turso graph edge row: {error}"))?
    {
        edges.push(TursoClientDbGraphEdge {
            from: row
                .get::<String>(0)
                .map_err(|error| format!("failed to read Turso graph edge from id: {error}"))?,
            to: row
                .get::<String>(1)
                .map_err(|error| format!("failed to read Turso graph edge to id: {error}"))?,
            kind: row
                .get::<String>(2)
                .map_err(|error| format!("failed to read Turso graph edge kind: {error}"))?,
        });
    }
    Ok(edges)
}
