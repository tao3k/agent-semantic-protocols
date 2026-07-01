//! Turso DB Engine adapter for `client.turso` state, search, and overlay data.

use std::path::{Path, PathBuf};

use serde::Serialize;

#[cfg(feature = "turso-backend")]
use crate::evidence_graph::{
    ClientDbEvidenceGraph, ClientDbEvidenceGraphEdge, ClientDbEvidenceGraphNode,
};

use super::contract::{
    ClientDbBackend, ClientDbEngineBackend, ClientDbEngineDurability, ClientDbEngineFeatures,
};

const TURSO_CLIENT_DB_FILE: &str = "client.turso";
const TURSO_CLIENT_DB_SCHEMA_VERSION: i64 = 1;
const TURSO_CLIENT_DB_SCHEMA_BOOTSTRAP_PENDING: &str = "pending-cutover";
#[cfg(feature = "turso-backend")]
const TURSO_CLIENT_DB_SCHEMA_BOOTSTRAP_READY: &str = "ready";
#[cfg(not(feature = "turso-backend"))]
const TURSO_CLIENT_DB_CUTOVER_REASON: &str =
    "Turso DB Engine backend requires the turso-backend feature";
#[cfg(feature = "turso-backend")]
/// Bootstrap metadata table used to record the Turso DB Engine schema version.
pub const TURSO_BOOTSTRAP_TABLE: &str = "asp_db_engine_bootstrap";
#[cfg(feature = "turso-backend")]
/// Stable entity table for the Turso-backed EvidenceGraph substrate.
pub const TURSO_ENTITY_TABLE: &str = "asp_graph_entity";
#[cfg(feature = "turso-backend")]
/// Stable edge table for the Turso-backed EvidenceGraph substrate.
pub const TURSO_EDGE_TABLE: &str = "asp_graph_edge";
#[cfg(feature = "turso-backend")]
/// Stable search-document table for generated selector/search projections.
pub const TURSO_SEARCH_DOCUMENT_TABLE: &str = "asp_search_document";
#[cfg(feature = "turso-backend")]
/// Session-scoped dirty overlay document table for dynamic search.
pub const TURSO_OVERLAY_DOCUMENT_TABLE: &str = "asp_overlay_document";
#[cfg(feature = "turso-backend")]
/// Bounded search route receipt table for replay and ranking feedback.
pub const TURSO_ROUTE_RECEIPT_TABLE: &str = "asp_route_receipt";

/// Diagnostic report for the planned Turso DB Engine backend.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TursoClientDbEngineReport {
    pub backend: &'static str,
    pub status: &'static str,
    pub db_file_name: &'static str,
    pub schema_version: i64,
    pub schema_bootstrap: &'static str,
    pub durability: &'static str,
    pub features: ClientDbEngineFeatures,
    pub db_path: PathBuf,
    pub reason: Option<&'static str>,
}

#[cfg(feature = "turso-backend")]
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

#[cfg(feature = "turso-backend")]
/// Feature-gated EvidenceGraph edge row written through the Turso adapter.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TursoClientDbGraphEdge {
    pub from: String,
    pub to: String,
    pub kind: String,
}

#[cfg(feature = "turso-backend")]
/// Persistence receipt for writing an EvidenceGraph projection into Turso.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TursoClientDbEvidenceGraphPersistReport {
    pub entity_count: usize,
    pub edge_count: usize,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(super) struct TursoClientDbEngineBackend;

impl ClientDbEngineBackend for TursoClientDbEngineBackend {
    type Connection = ();
    type Report = TursoClientDbEngineReport;

    fn backend(&self) -> ClientDbBackend {
        ClientDbBackend::Turso
    }

    fn db_file_name(&self) -> &'static str {
        TURSO_CLIENT_DB_FILE
    }

    fn schema_version(&self) -> i64 {
        TURSO_CLIENT_DB_SCHEMA_VERSION
    }

    fn durability(&self) -> ClientDbEngineDurability {
        ClientDbEngineDurability::TursoLocalFile
    }

    fn features(&self) -> ClientDbEngineFeatures {
        ClientDbEngineFeatures {
            async_io: true,
            concurrent_writes: true,
            fts: true,
            vector: false,
            overlay_search: true,
            sync: true,
            encryption: false,
        }
    }

    fn open_or_create(&self, db_path: &Path) -> Result<Self::Connection, String> {
        Err(format!(
            "Turso DB Engine synchronous open is not supported; use the async bootstrap facade; dbPath={}",
            db_path.display()
        ))
    }

    fn open_read_only_existing(&self, db_path: &Path) -> Result<Option<Self::Connection>, String> {
        if db_path.exists() {
            self.open_or_create(db_path).map(Some)
        } else {
            Ok(None)
        }
    }

    fn inspect(&self, db_path: &Path) -> TursoClientDbEngineReport {
        let active_db_path = db_path.with_file_name(self.db_file_name());
        #[cfg(feature = "turso-backend")]
        let (status, schema_bootstrap, reason) = if active_db_path.exists() {
            ("ready", TURSO_CLIENT_DB_SCHEMA_BOOTSTRAP_READY, None)
        } else {
            ("missing", TURSO_CLIENT_DB_SCHEMA_BOOTSTRAP_PENDING, None)
        };
        #[cfg(not(feature = "turso-backend"))]
        let (status, schema_bootstrap, reason) = (
            "feature-disabled",
            TURSO_CLIENT_DB_SCHEMA_BOOTSTRAP_PENDING,
            Some(TURSO_CLIENT_DB_CUTOVER_REASON),
        );
        TursoClientDbEngineReport {
            backend: self.backend().as_str(),
            status,
            db_file_name: self.db_file_name(),
            schema_version: self.schema_version(),
            schema_bootstrap,
            durability: self.durability().as_str(),
            features: self.features(),
            db_path: active_db_path,
            reason,
        }
    }
}

/// Bootstrap the planned Turso backend file without switching active DB traffic.
///
/// This is the first executable Turso cutover gate: the DB crate must be able
/// to create the local Turso file and apply a minimal schema through the Turso
/// Rust API before any active backend migration is allowed.
#[cfg(feature = "turso-backend")]
pub async fn bootstrap_turso_client_db(
    db_path: &Path,
) -> Result<TursoClientDbEngineReport, String> {
    let turso_path = prepare_turso_client_db_path(db_path)?;
    let connection = open_turso_client_connection(&turso_path).await?;
    bootstrap_turso_schema_version(&connection).await?;
    bootstrap_turso_client_search_schema(&connection).await?;
    Ok(turso_bootstrap_report(db_path))
}

#[cfg(feature = "turso-backend")]
/// Insert or update one EvidenceGraph entity in the Turso DB Engine file.
pub async fn upsert_turso_graph_entity(
    db_path: &Path,
    entity: &TursoClientDbGraphEntity,
) -> Result<(), String> {
    let connection = connect_turso_client_db(db_path).await?;
    upsert_turso_graph_entity_with_connection(&connection, entity).await
}

#[cfg(feature = "turso-backend")]
/// Insert or update one EvidenceGraph edge in the Turso DB Engine file.
pub async fn upsert_turso_graph_edge(
    db_path: &Path,
    edge: &TursoClientDbGraphEdge,
) -> Result<(), String> {
    let connection = connect_turso_client_db(db_path).await?;
    upsert_turso_graph_edge_with_connection(&connection, edge).await
}

#[cfg(feature = "turso-backend")]
/// Persist a DB-owned EvidenceGraph projection into the Turso DB Engine file.
pub async fn persist_turso_evidence_graph(
    db_path: &Path,
    graph: &ClientDbEvidenceGraph,
) -> Result<TursoClientDbEvidenceGraphPersistReport, String> {
    let connection = connect_turso_client_db(db_path).await?;
    connection
        .execute("BEGIN TRANSACTION", ())
        .await
        .map_err(|error| format!("failed to begin Turso evidence graph transaction: {error}"))?;
    for node in &graph.nodes {
        if let Err(error) = upsert_turso_graph_entity_with_connection(
            &connection,
            &TursoClientDbGraphEntity::from(node),
        )
        .await
        {
            let _ = connection.execute("ROLLBACK", ()).await;
            return Err(error);
        }
    }
    for edge in &graph.edges {
        if let Err(error) = upsert_turso_graph_edge_with_connection(
            &connection,
            &TursoClientDbGraphEdge::from(edge),
        )
        .await
        {
            let _ = connection.execute("ROLLBACK", ()).await;
            return Err(error);
        }
    }
    connection
        .execute("COMMIT", ())
        .await
        .map_err(|error| format!("failed to commit Turso evidence graph transaction: {error}"))?;
    Ok(TursoClientDbEvidenceGraphPersistReport {
        entity_count: graph.nodes.len(),
        edge_count: graph.edges.len(),
    })
}

#[cfg(feature = "turso-backend")]
async fn upsert_turso_graph_entity_with_connection(
    connection: &turso::Connection,
    entity: &TursoClientDbGraphEntity,
) -> Result<(), String> {
    let query_keys_json = serde_json::to_string(&entity.query_keys)
        .map_err(|error| format!("failed to encode Turso graph entity query keys: {error}"))?;
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
        .map_err(|error| format!("failed to upsert Turso graph entity: {error}"))?;
    Ok(())
}

#[cfg(feature = "turso-backend")]
async fn upsert_turso_graph_edge_with_connection(
    connection: &turso::Connection,
    edge: &TursoClientDbGraphEdge,
) -> Result<(), String> {
    connection
        .execute(
            "INSERT INTO asp_graph_edge (from_id, to_id, kind)
             VALUES (?1, ?2, ?3)
             ON CONFLICT(from_id, to_id, kind) DO UPDATE SET
                kind = excluded.kind",
            (edge.from.as_str(), edge.to.as_str(), edge.kind.as_str()),
        )
        .await
        .map_err(|error| format!("failed to upsert Turso graph edge: {error}"))?;
    Ok(())
}

#[cfg(feature = "turso-backend")]
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
        connection
            .query(sql, (kind, limit))
            .await
            .map_err(|error| format!("failed to query Turso graph entities: {error}"))?
    } else {
        connection
            .query(sql, [limit])
            .await
            .map_err(|error| format!("failed to query Turso graph entities: {error}"))?
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

#[cfg(feature = "turso-backend")]
/// Return EvidenceGraph entities by primary key using an existing Turso connection.
pub(super) async fn list_turso_graph_entities_by_ids_with_connection(
    connection: &turso::Connection,
    ids: &[String],
) -> Result<Vec<TursoClientDbGraphEntity>, String> {
    let mut entities = Vec::new();
    for id in ids {
        let mut rows = connection
            .query(
                "SELECT id, kind, label, selector, path, language_id, provider_id, query_keys_json
                 FROM asp_graph_entity
                 WHERE id = ?1
                 LIMIT 1",
                [id.as_str()],
            )
            .await
            .map_err(|error| format!("failed to query Turso graph entity by id: {error}"))?;
        if let Some(row) = rows
            .next()
            .await
            .map_err(|error| format!("failed to read Turso graph entity by id row: {error}"))?
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
                selector: row.get::<Option<String>>(3).map_err(|error| {
                    format!("failed to read Turso graph entity selector: {error}")
                })?,
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
    }
    Ok(entities)
}

#[cfg(feature = "turso-backend")]
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
        connection
            .query(sql, (kind, limit))
            .await
            .map_err(|error| format!("failed to query Turso graph edges: {error}"))?
    } else {
        connection
            .query(sql, [limit])
            .await
            .map_err(|error| format!("failed to query Turso graph edges: {error}"))?
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

#[cfg(feature = "turso-backend")]
fn prepare_turso_client_db_path(db_path: &Path) -> Result<PathBuf, String> {
    if let Some(parent) = db_path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|error| format!("failed to create Turso client DB dir: {error}"))?;
    }
    Ok(db_path.with_file_name(TURSO_CLIENT_DB_FILE))
}

#[cfg(feature = "turso-backend")]
async fn open_turso_client_connection(turso_path: &Path) -> Result<turso::Connection, String> {
    let database = turso_builder(turso_path)
        .build()
        .await
        .map_err(|error| format!("failed to open Turso client DB: {error}"))?;
    database
        .connect()
        .map_err(|error| format!("failed to connect Turso client DB: {error}"))
}

#[cfg(feature = "turso-backend")]
async fn bootstrap_turso_schema_version(connection: &turso::Connection) -> Result<(), String> {
    connection
        .execute(
            "CREATE TABLE IF NOT EXISTS asp_db_engine_bootstrap (schema_version INTEGER NOT NULL)",
            (),
        )
        .await
        .map_err(|error| format!("failed to bootstrap Turso client DB schema: {error}"))?;
    connection
        .execute("DELETE FROM asp_db_engine_bootstrap", ())
        .await
        .map_err(|error| format!("failed to reset Turso bootstrap schema row: {error}"))?;
    connection
        .execute(
            "INSERT INTO asp_db_engine_bootstrap (schema_version) VALUES (?1)",
            [TURSO_CLIENT_DB_SCHEMA_VERSION],
        )
        .await
        .map_err(|error| format!("failed to write Turso bootstrap schema row: {error}"))?;
    Ok(())
}

#[cfg(feature = "turso-backend")]
fn turso_bootstrap_report(db_path: &Path) -> TursoClientDbEngineReport {
    let backend = TursoClientDbEngineBackend;
    let mut report = backend.inspect(db_path);
    report.status = "bootstrap-smoke";
    report.schema_bootstrap = TURSO_CLIENT_DB_SCHEMA_BOOTSTRAP_READY;
    report.reason = None;
    report
}

#[cfg(feature = "turso-backend")]
async fn bootstrap_turso_client_search_schema(
    connection: &turso::Connection,
) -> Result<(), String> {
    for statement in [
        "CREATE TABLE IF NOT EXISTS asp_graph_entity (
            id TEXT PRIMARY KEY,
            kind TEXT NOT NULL,
            label TEXT NOT NULL,
            selector TEXT,
            path TEXT,
            language_id TEXT,
            provider_id TEXT,
            query_keys_json TEXT NOT NULL DEFAULT '[]'
        )",
        "CREATE INDEX IF NOT EXISTS asp_graph_entity_kind_idx ON asp_graph_entity(kind)",
        "CREATE INDEX IF NOT EXISTS asp_graph_entity_language_idx ON asp_graph_entity(kind, language_id)",
        "CREATE TABLE IF NOT EXISTS asp_graph_edge (
            from_id TEXT NOT NULL,
            to_id TEXT NOT NULL,
            kind TEXT NOT NULL,
            PRIMARY KEY(from_id, to_id, kind)
        )",
        "CREATE INDEX IF NOT EXISTS asp_graph_edge_kind_idx ON asp_graph_edge(kind)",
        "CREATE INDEX IF NOT EXISTS asp_graph_edge_to_idx ON asp_graph_edge(to_id)",
        "CREATE TABLE IF NOT EXISTS asp_search_document (
            namespace TEXT NOT NULL,
            document_id TEXT NOT NULL,
            entity_id TEXT NOT NULL,
            selector TEXT,
            document TEXT NOT NULL,
            PRIMARY KEY(namespace, document_id)
        )",
        "CREATE INDEX IF NOT EXISTS asp_search_document_entity_idx ON asp_search_document(entity_id)",
        "CREATE INDEX IF NOT EXISTS asp_search_document_fts_idx ON asp_search_document USING fts (document, selector)",
        "CREATE TABLE IF NOT EXISTS asp_overlay_document (
            repo_id TEXT NOT NULL,
            workspace_id TEXT NOT NULL,
            session_id TEXT NOT NULL,
            base_generation TEXT NOT NULL,
            document_id TEXT NOT NULL,
            selector TEXT,
            document TEXT NOT NULL,
            PRIMARY KEY(repo_id, workspace_id, session_id, base_generation, document_id)
        )",
        "CREATE INDEX IF NOT EXISTS asp_overlay_document_session_idx ON asp_overlay_document(repo_id, workspace_id, session_id)",
        "CREATE INDEX IF NOT EXISTS asp_overlay_document_fts_idx ON asp_overlay_document USING fts (document, selector)",
        "CREATE TABLE IF NOT EXISTS asp_route_receipt (
            receipt_id TEXT PRIMARY KEY,
            repo_id TEXT NOT NULL,
            workspace_id TEXT NOT NULL,
            scope_id TEXT NOT NULL,
            session_id TEXT,
            query TEXT NOT NULL,
            route_source TEXT NOT NULL,
            selected_selector TEXT,
            next_command TEXT,
            hit_count INTEGER NOT NULL,
            evidence_ids_json TEXT NOT NULL DEFAULT '[]',
            created_at_ms INTEGER NOT NULL
        )",
        "CREATE INDEX IF NOT EXISTS asp_route_receipt_workspace_idx ON asp_route_receipt(repo_id, workspace_id, created_at_ms)",
        "CREATE INDEX IF NOT EXISTS asp_route_receipt_session_idx ON asp_route_receipt(repo_id, workspace_id, session_id, created_at_ms)",
    ] {
        connection
            .execute(statement, ())
            .await
            .map_err(|error| format!("failed to bootstrap Turso search schema: {error}"))?;
    }
    Ok(())
}

#[cfg(feature = "turso-backend")]
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

#[cfg(feature = "turso-backend")]
impl From<&ClientDbEvidenceGraphEdge> for TursoClientDbGraphEdge {
    fn from(edge: &ClientDbEvidenceGraphEdge) -> Self {
        Self {
            from: edge.from.clone(),
            to: edge.to.clone(),
            kind: edge.kind.to_string(),
        }
    }
}

#[cfg(feature = "turso-backend")]
fn turso_builder(turso_path: &Path) -> turso::Builder {
    turso::Builder::new_local(turso_path.to_string_lossy().as_ref()).experimental_index_method(true)
}

#[cfg(feature = "turso-backend")]
pub(super) async fn connect_turso_client_db(db_path: &Path) -> Result<turso::Connection, String> {
    let turso_path = db_path.with_file_name(TURSO_CLIENT_DB_FILE);
    let database = turso_builder(&turso_path)
        .build()
        .await
        .map_err(|error| format!("failed to open Turso client DB: {error}"))?;
    database
        .connect()
        .map_err(|error| format!("failed to connect Turso client DB: {error}"))
}
