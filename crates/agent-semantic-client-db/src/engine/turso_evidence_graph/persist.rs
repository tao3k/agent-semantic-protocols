use std::path::Path;

use agent_semantic_content_identity::SourceSnapshotEvidence;

use super::artifact::{GRAPH_ARTIFACT_SCHEMA_ID, graph_artifact_digest_for_snapshot};
use super::{
    TursoClientDbEvidenceGraphPersistReport, TursoClientDbGraphEdge, TursoClientDbGraphEntity,
};
use crate::engine::turso::connect_turso_client_db;
use crate::engine::turso_operation_lock::acquire_turso_operation_lock;
use crate::engine::turso_statement::{
    execute_turso_prepared_statement_with_lock_retry, execute_turso_statement_with_lock_retry,
    run_turso_operation_with_lock_retry,
};
use crate::evidence_graph::ClientDbEvidenceGraph;

/// Persist a DB-owned EvidenceGraph projection into the Turso DB Engine file.
pub async fn persist_turso_evidence_graph(
    db_path: &Path,
    graph: &ClientDbEvidenceGraph,
    source_snapshot: &SourceSnapshotEvidence,
) -> Result<TursoClientDbEvidenceGraphPersistReport, String> {
    let graph_artifact_digest = graph_artifact_digest_for_snapshot(source_snapshot);
    let source_snapshot_json = serde_json::to_string(source_snapshot)
        .map_err(|error| format!("failed to encode graph source snapshot evidence: {error}"))?;
    let _operation_lock = acquire_turso_operation_lock(db_path, "evidence-graph-persist")?;
    let connection = connect_turso_client_db(db_path).await?;
    execute_turso_statement_with_lock_retry(
        &connection,
        "BEGIN TRANSACTION",
        "failed to begin Turso evidence graph transaction",
    )
    .await?;
    let mut artifact_statement = match run_turso_operation_with_lock_retry(
        || async {
            connection
                .prepare_cached(
                    "INSERT INTO asp_graph_artifact (
                        graph_artifact_digest, snapshot_root, provider_digest,
                        source_snapshot_json, schema_id
                     ) VALUES (?1, ?2, ?3, ?4, ?5)
                     ON CONFLICT(graph_artifact_digest) DO NOTHING",
                )
                .await
                .map_err(|error| error.to_string())
        },
        "failed to prepare Turso graph artifact upsert",
    )
    .await
    {
        Ok(statement) => statement,
        Err(error) => {
            let _ = execute_turso_statement_with_lock_retry(
                &connection,
                "ROLLBACK",
                "failed to rollback Turso evidence graph transaction after artifact prepare",
            )
            .await;
            return Err(format!(
                "failed to prepare Turso graph artifact upsert: {error}"
            ));
        }
    };
    if let Err(error) = execute_turso_prepared_statement_with_lock_retry!(
        artifact_statement,
        (
            graph_artifact_digest.as_str(),
            source_snapshot.root_digest.as_str(),
            source_snapshot.provider_digest.as_str(),
            source_snapshot_json.as_str(),
            GRAPH_ARTIFACT_SCHEMA_ID,
        ),
        "failed to upsert Turso graph artifact",
    ) {
        let _ = execute_turso_statement_with_lock_retry(
            &connection,
            "ROLLBACK",
            "failed to rollback Turso evidence graph transaction after artifact upsert",
        )
        .await;
        return Err(error);
    }
    let mut entity_statement = match run_turso_operation_with_lock_retry(
        || async {
            connection
                .prepare_cached(
                    "INSERT INTO asp_graph_artifact_entity (
                        graph_artifact_digest, id, kind, semantic_kind, label,
                        selector, path, language_id, provider_id, query_keys_json
                     ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)
                     ON CONFLICT(graph_artifact_digest, id) DO UPDATE SET
                        kind = excluded.kind,
                        semantic_kind = excluded.semantic_kind,
                        label = excluded.label,
                        selector = excluded.selector,
                        path = excluded.path,
                        language_id = excluded.language_id,
                        provider_id = excluded.provider_id,
                        query_keys_json = excluded.query_keys_json",
                )
                .await
                .map_err(|error| error.to_string())
        },
        "failed to prepare Turso graph entity upsert",
    )
    .await
    {
        Ok(statement) => statement,
        Err(error) => {
            let _ = execute_turso_statement_with_lock_retry(
                &connection,
                "ROLLBACK",
                "failed to rollback Turso evidence graph transaction after entity prepare",
            )
            .await;
            return Err(format!(
                "failed to prepare Turso graph entity upsert: {error}"
            ));
        }
    };
    for node in &graph.nodes {
        let entity = TursoClientDbGraphEntity::from(node);
        let query_keys_json = match serde_json::to_string(&entity.query_keys) {
            Ok(value) => value,
            Err(error) => {
                let _ = execute_turso_statement_with_lock_retry(
                    &connection,
                    "ROLLBACK",
                    "failed to rollback Turso evidence graph transaction after entity encode",
                )
                .await;
                return Err(format!(
                    "failed to encode Turso graph entity query keys: {error}"
                ));
            }
        };
        if let Err(error) = execute_turso_prepared_statement_with_lock_retry!(
            entity_statement,
            (
                graph_artifact_digest.as_str(),
                entity.id.as_str(),
                entity.kind.as_str(),
                entity.semantic_kind.as_deref(),
                entity.label.as_str(),
                entity.selector.as_deref(),
                entity.path.as_deref(),
                entity.language_id.as_deref(),
                entity.provider_id.as_deref(),
                query_keys_json.as_str(),
            ),
            "failed to upsert Turso graph entity",
        ) {
            let _ = execute_turso_statement_with_lock_retry(
                &connection,
                "ROLLBACK",
                "failed to rollback Turso evidence graph transaction after entity upsert",
            )
            .await;
            return Err(error);
        }
    }
    let mut edge_statement = match run_turso_operation_with_lock_retry(
        || async {
            connection
                .prepare_cached(
                    "INSERT INTO asp_graph_artifact_edge (
                        graph_artifact_digest, from_id, to_id, kind
                     ) VALUES (?1, ?2, ?3, ?4)
                     ON CONFLICT(graph_artifact_digest, from_id, to_id, kind) DO UPDATE SET
                        kind = excluded.kind",
                )
                .await
                .map_err(|error| error.to_string())
        },
        "failed to prepare Turso graph edge upsert",
    )
    .await
    {
        Ok(statement) => statement,
        Err(error) => {
            let _ = execute_turso_statement_with_lock_retry(
                &connection,
                "ROLLBACK",
                "failed to rollback Turso evidence graph transaction after edge prepare",
            )
            .await;
            return Err(format!(
                "failed to prepare Turso graph edge upsert: {error}"
            ));
        }
    };
    for edge in &graph.edges {
        let edge = TursoClientDbGraphEdge::from(edge);
        if let Err(error) = execute_turso_prepared_statement_with_lock_retry!(
            edge_statement,
            (
                graph_artifact_digest.as_str(),
                edge.from.as_str(),
                edge.to.as_str(),
                edge.kind.as_str(),
            ),
            "failed to upsert Turso graph edge",
        ) {
            let _ = execute_turso_statement_with_lock_retry(
                &connection,
                "ROLLBACK",
                "failed to rollback Turso evidence graph transaction after edge upsert",
            )
            .await;
            return Err(error);
        }
    }
    drop(artifact_statement);
    drop(entity_statement);
    drop(edge_statement);
    execute_turso_statement_with_lock_retry(
        &connection,
        "COMMIT",
        "failed to commit Turso evidence graph transaction",
    )
    .await?;
    Ok(TursoClientDbEvidenceGraphPersistReport {
        entity_count: graph.nodes.len(),
        edge_count: graph.edges.len(),
        graph_artifact_digest,
    })
}
