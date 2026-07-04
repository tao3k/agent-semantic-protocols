//! Turso EvidenceGraph adapter.

use std::path::Path;

use crate::evidence_graph::{
    ClientDbEvidenceGraph, ClientDbEvidenceGraphEdge, ClientDbEvidenceGraphNode,
};

use super::turso::connect_turso_client_db;
use super::turso_operation_lock::acquire_turso_operation_lock;
use super::turso_statement::{
    execute_turso_operation_with_lock_retry, execute_turso_prepared_statement_with_lock_retry,
    execute_turso_statement_with_lock_retry, run_turso_operation_with_lock_retry,
};

/// Stable entity table for the Turso-backed EvidenceGraph substrate.
pub const TURSO_ENTITY_TABLE: &str = "asp_graph_entity";
/// Stable edge table for the Turso-backed EvidenceGraph substrate.
pub const TURSO_EDGE_TABLE: &str = "asp_graph_edge";

/// Feature-gated EvidenceGraph entity row written through the Turso adapter.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TursoClientDbGraphEntity {
    pub id: String,
    pub kind: String,
    pub label: String,
    pub selector: Option<String>,
    pub path: Option<String>,
    pub language_id: Option<String>,
    pub provider_id: Option<String>,
    pub query_keys: Vec<String>,
}

/// Feature-gated EvidenceGraph edge row written through the Turso adapter.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TursoClientDbGraphEdge {
    pub from: String,
    pub to: String,
    pub kind: String,
}

/// Persistence receipt for writing an EvidenceGraph projection into Turso.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TursoClientDbEvidenceGraphPersistReport {
    pub entity_count: usize,
    pub edge_count: usize,
}

/// Insert or update one EvidenceGraph entity in the Turso DB Engine file.
pub async fn upsert_turso_graph_entity(
    db_path: &Path,
    entity: &TursoClientDbGraphEntity,
) -> Result<(), String> {
    let _operation_lock = acquire_turso_operation_lock(db_path, "graph-entity-upsert")?;
    let connection = connect_turso_client_db(db_path).await?;
    upsert_turso_graph_entity_with_connection(&connection, entity).await
}

/// Insert or update one EvidenceGraph edge in the Turso DB Engine file.
pub async fn upsert_turso_graph_edge(
    db_path: &Path,
    edge: &TursoClientDbGraphEdge,
) -> Result<(), String> {
    let _operation_lock = acquire_turso_operation_lock(db_path, "graph-edge-upsert")?;
    let connection = connect_turso_client_db(db_path).await?;
    upsert_turso_graph_edge_with_connection(&connection, edge).await
}

/// Persist a DB-owned EvidenceGraph projection into the Turso DB Engine file.
pub async fn persist_turso_evidence_graph(
    db_path: &Path,
    graph: &ClientDbEvidenceGraph,
) -> Result<TursoClientDbEvidenceGraphPersistReport, String> {
    let _operation_lock = acquire_turso_operation_lock(db_path, "evidence-graph-persist")?;
    let connection = connect_turso_client_db(db_path).await?;
    execute_turso_statement_with_lock_retry(
        &connection,
        "BEGIN TRANSACTION",
        "failed to begin Turso evidence graph transaction",
    )
    .await?;
    let mut entity_statement = match run_turso_operation_with_lock_retry(
        || async {
            connection
                .prepare_cached(
                    "INSERT INTO asp_graph_entity (id, kind, label, selector, path, language_id, provider_id, query_keys_json)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
                     ON CONFLICT(id) DO UPDATE SET
                        kind = excluded.kind,
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
            return Err(format!("failed to prepare Turso graph entity upsert: {error}"));
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
                entity.id.as_str(),
                entity.kind.as_str(),
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
                    "INSERT INTO asp_graph_edge (from_id, to_id, kind)
                     VALUES (?1, ?2, ?3)
                     ON CONFLICT(from_id, to_id, kind) DO UPDATE SET
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
            (edge.from.as_str(), edge.to.as_str(), edge.kind.as_str()),
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
    })
}

async fn upsert_turso_graph_entity_with_connection(
    connection: &turso::Connection,
    entity: &TursoClientDbGraphEntity,
) -> Result<(), String> {
    let query_keys_json = serde_json::to_string(&entity.query_keys)
        .map_err(|error| format!("failed to encode Turso graph entity query keys: {error}"))?;
    execute_turso_operation_with_lock_retry(
        || async {
            connection
                .execute(
                    "INSERT INTO asp_graph_entity (id, kind, label, selector, path, language_id, provider_id, query_keys_json)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
                     ON CONFLICT(id) DO UPDATE SET
                        kind = excluded.kind,
                        label = excluded.label,
                        selector = excluded.selector,
                        path = excluded.path,
                        language_id = excluded.language_id,
                        provider_id = excluded.provider_id,
                        query_keys_json = excluded.query_keys_json",
                    (
                        entity.id.as_str(),
                        entity.kind.as_str(),
                        entity.label.as_str(),
                        entity.selector.as_deref(),
                        entity.path.as_deref(),
                        entity.language_id.as_deref(),
                        entity.provider_id.as_deref(),
                        query_keys_json.as_str(),
                    ),
                )
                .await
                .map_err(|error| error.to_string())
        },
        "failed to upsert Turso graph entity",
    )
    .await?;
    Ok(())
}

async fn upsert_turso_graph_edge_with_connection(
    connection: &turso::Connection,
    edge: &TursoClientDbGraphEdge,
) -> Result<(), String> {
    execute_turso_operation_with_lock_retry(
        || async {
            connection
                .execute(
                    "INSERT INTO asp_graph_edge (from_id, to_id, kind)
                     VALUES (?1, ?2, ?3)
                     ON CONFLICT(from_id, to_id, kind) DO UPDATE SET
                        kind = excluded.kind",
                    (edge.from.as_str(), edge.to.as_str(), edge.kind.as_str()),
                )
                .await
                .map_err(|error| error.to_string())
        },
        "failed to upsert Turso graph edge",
    )
    .await?;
    Ok(())
}

/// List EvidenceGraph entities from the Turso DB Engine file.
pub async fn list_turso_graph_entities(
    db_path: &Path,
    kind: Option<&str>,
    limit: u32,
) -> Result<Vec<TursoClientDbGraphEntity>, String> {
    if limit == 0 {
        return Ok(Vec::new());
    }
    let connection = connect_turso_client_db(db_path).await?;
    let (sql, parameter): (&str, Option<&str>) = if let Some(kind) = kind {
        (
            "SELECT id, kind, label, selector, path, language_id, provider_id, query_keys_json
             FROM asp_graph_entity
             WHERE kind = ?1
             ORDER BY id
             LIMIT ?2",
            Some(kind),
        )
    } else {
        (
            "SELECT id, kind, label, selector, path, language_id, provider_id, query_keys_json
             FROM asp_graph_entity
             ORDER BY id
             LIMIT ?1",
            None,
        )
    };
    let mut rows = if let Some(kind) = parameter {
        run_turso_operation_with_lock_retry(
            || async {
                connection
                    .query(sql, (kind, limit))
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
                    .query(sql, [limit])
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
        let query_keys_json = row
            .get::<String>(7)
            .map_err(|error| format!("failed to read Turso graph query keys: {error}"))?;
        let query_keys = serde_json::from_str::<Vec<String>>(&query_keys_json)
            .map_err(|error| format!("failed to decode Turso graph query keys: {error}"))?;
        entities.push(TursoClientDbGraphEntity {
            id: row
                .get::<String>(0)
                .map_err(|error| format!("failed to read Turso graph entity id: {error}"))?,
            kind: row
                .get::<String>(1)
                .map_err(|error| format!("failed to read Turso graph entity kind: {error}"))?,
            label: row
                .get::<String>(2)
                .map_err(|error| format!("failed to read Turso graph entity label: {error}"))?,
            selector: row
                .get::<Option<String>>(3)
                .map_err(|error| format!("failed to read Turso graph entity selector: {error}"))?,
            path: row
                .get::<Option<String>>(4)
                .map_err(|error| format!("failed to read Turso graph entity path: {error}"))?,
            language_id: row.get::<Option<String>>(5).map_err(|error| {
                format!("failed to read Turso graph entity language id: {error}")
            })?,
            provider_id: row.get::<Option<String>>(6).map_err(|error| {
                format!("failed to read Turso graph entity provider id: {error}")
            })?,
            query_keys,
        });
    }
    Ok(entities)
}

/// List EvidenceGraph edges from the Turso DB Engine file.
pub async fn list_turso_graph_edges(
    db_path: &Path,
    kind: Option<&str>,
    limit: u32,
) -> Result<Vec<TursoClientDbGraphEdge>, String> {
    if limit == 0 {
        return Ok(Vec::new());
    }
    let connection = connect_turso_client_db(db_path).await?;
    let (sql, parameter): (&str, Option<&str>) = if let Some(kind) = kind {
        (
            "SELECT from_id, to_id, kind
             FROM asp_graph_edge
             WHERE kind = ?1
             ORDER BY from_id, to_id, kind
             LIMIT ?2",
            Some(kind),
        )
    } else {
        (
            "SELECT from_id, to_id, kind
             FROM asp_graph_edge
             ORDER BY from_id, to_id, kind
             LIMIT ?1",
            None,
        )
    };
    let mut rows = if let Some(kind) = parameter {
        run_turso_operation_with_lock_retry(
            || async {
                connection
                    .query(sql, (kind, limit))
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
                    .query(sql, [limit])
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

impl From<&ClientDbEvidenceGraphNode> for TursoClientDbGraphEntity {
    fn from(node: &ClientDbEvidenceGraphNode) -> Self {
        Self {
            id: node.id.clone(),
            kind: node.kind.to_string(),
            label: node.label.clone(),
            selector: node.selector.clone(),
            path: node.path.clone(),
            language_id: node.language_id.clone(),
            provider_id: node.provider_id.clone(),
            query_keys: node.query_keys.clone(),
        }
    }
}

impl From<&ClientDbEvidenceGraphEdge> for TursoClientDbGraphEdge {
    fn from(edge: &ClientDbEvidenceGraphEdge) -> Self {
        Self {
            from: edge.from.clone(),
            to: edge.to.clone(),
            kind: edge.kind.to_string(),
        }
    }
}
