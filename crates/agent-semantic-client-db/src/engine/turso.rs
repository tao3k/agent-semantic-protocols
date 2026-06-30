use std::path::{Path, PathBuf};

use serde::Serialize;

use super::facade::ClientDbEngineBackend;
use super::{ClientDbBackend, ClientDbEngineDurability, ClientDbEngineFeatures};

const TURSO_CLIENT_DB_FILE: &str = "client.turso";
const TURSO_CLIENT_DB_SCHEMA_VERSION: i64 = 1;
const TURSO_CLIENT_DB_SCHEMA_BOOTSTRAP_PENDING: &str = "pending-cutover";
#[cfg(feature = "turso-backend")]
const TURSO_CLIENT_DB_SCHEMA_BOOTSTRAP_READY: &str = "ready";
const TURSO_CLIENT_DB_CUTOVER_REASON: &str =
    "active backend remains sqlite-v1 until Turso cutover gates pass";
#[cfg(feature = "turso-backend")]
pub const TURSO_BOOTSTRAP_TABLE: &str = "asp_db_engine_bootstrap";
#[cfg(feature = "turso-backend")]
pub const TURSO_ENTITY_TABLE: &str = "asp_graph_entity";
#[cfg(feature = "turso-backend")]
pub const TURSO_SEARCH_DOCUMENT_TABLE: &str = "asp_search_document";
#[cfg(feature = "turso-backend")]
pub const TURSO_OVERLAY_DOCUMENT_TABLE: &str = "asp_overlay_document";

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
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TursoClientDbSearchDocument {
    pub namespace: String,
    pub document_id: String,
    pub entity_id: String,
    pub selector: Option<String>,
    pub document: String,
}

#[cfg(feature = "turso-backend")]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TursoClientDbOverlayDocument {
    pub repo_id: String,
    pub workspace_id: String,
    pub session_id: String,
    pub base_generation: String,
    pub document_id: String,
    pub selector: Option<String>,
    pub document: String,
}

#[cfg(feature = "turso-backend")]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TursoClientDbSearchHit {
    pub source: &'static str,
    pub document_id: String,
    pub selector: Option<String>,
    pub document: String,
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
            "Turso DB Engine backend is not active yet: {}; dbPath={}",
            TURSO_CLIENT_DB_CUTOVER_REASON,
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
        TursoClientDbEngineReport {
            backend: self.backend().as_str(),
            status: "planned",
            db_file_name: self.db_file_name(),
            schema_version: self.schema_version(),
            schema_bootstrap: TURSO_CLIENT_DB_SCHEMA_BOOTSTRAP_PENDING,
            durability: self.durability().as_str(),
            features: self.features(),
            db_path: db_path.with_file_name(self.db_file_name()),
            reason: Some(TURSO_CLIENT_DB_CUTOVER_REASON),
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
    if let Some(parent) = db_path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|error| format!("failed to create Turso client DB dir: {error}"))?;
    }
    let turso_path = db_path.with_file_name(TURSO_CLIENT_DB_FILE);
    let database = turso::Builder::new_local(turso_path.to_string_lossy().as_ref())
        .build()
        .await
        .map_err(|error| format!("failed to open Turso client DB: {error}"))?;
    let connection = database
        .connect()
        .map_err(|error| format!("failed to connect Turso client DB: {error}"))?;
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
    bootstrap_turso_client_search_schema(&connection).await?;
    let backend = TursoClientDbEngineBackend;
    let mut report = backend.inspect(db_path);
    report.status = "bootstrap-smoke";
    report.schema_bootstrap = TURSO_CLIENT_DB_SCHEMA_BOOTSTRAP_READY;
    report.reason = None;
    Ok(report)
}

#[cfg(feature = "turso-backend")]
pub async fn upsert_turso_search_document(
    db_path: &Path,
    document: &TursoClientDbSearchDocument,
) -> Result<(), String> {
    let connection = connect_turso_client_db(db_path).await?;
    connection
        .execute(
            "INSERT INTO asp_search_document (namespace, document_id, entity_id, selector, document)
             VALUES (?1, ?2, ?3, ?4, ?5)
             ON CONFLICT(namespace, document_id) DO UPDATE SET
                entity_id = excluded.entity_id,
                selector = excluded.selector,
                document = excluded.document",
            (
                document.namespace.as_str(),
                document.document_id.as_str(),
                document.entity_id.as_str(),
                document.selector.as_deref(),
                document.document.as_str(),
            ),
        )
        .await
        .map_err(|error| format!("failed to upsert Turso search document: {error}"))?;
    Ok(())
}

#[cfg(feature = "turso-backend")]
pub async fn upsert_turso_overlay_document(
    db_path: &Path,
    document: &TursoClientDbOverlayDocument,
) -> Result<(), String> {
    let connection = connect_turso_client_db(db_path).await?;
    connection
        .execute(
            "INSERT INTO asp_overlay_document
             (repo_id, workspace_id, session_id, base_generation, document_id, selector, document)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
             ON CONFLICT(repo_id, workspace_id, session_id, base_generation, document_id)
             DO UPDATE SET
                selector = excluded.selector,
                document = excluded.document",
            (
                document.repo_id.as_str(),
                document.workspace_id.as_str(),
                document.session_id.as_str(),
                document.base_generation.as_str(),
                document.document_id.as_str(),
                document.selector.as_deref(),
                document.document.as_str(),
            ),
        )
        .await
        .map_err(|error| format!("failed to upsert Turso overlay document: {error}"))?;
    Ok(())
}

#[cfg(feature = "turso-backend")]
pub async fn search_turso_documents(
    db_path: &Path,
    query: &str,
    limit: u32,
) -> Result<Vec<TursoClientDbSearchHit>, String> {
    if limit == 0 || query.trim().is_empty() {
        return Ok(Vec::new());
    }
    let connection = connect_turso_client_db(db_path).await?;
    let like_query = format!("%{}%", query.trim());
    let mut hits = Vec::new();
    collect_turso_search_hits(
        &connection,
        "stable",
        "SELECT document_id, selector, document
         FROM asp_search_document
         WHERE document LIKE ?1 OR selector LIKE ?1
         ORDER BY document_id
         LIMIT ?2",
        &like_query,
        limit,
        &mut hits,
    )
    .await?;
    if hits.len() < limit as usize {
        collect_turso_search_hits(
            &connection,
            "overlay",
            "SELECT document_id, selector, document
             FROM asp_overlay_document
             WHERE document LIKE ?1 OR selector LIKE ?1
             ORDER BY document_id
             LIMIT ?2",
            &like_query,
            limit.saturating_sub(hits.len() as u32),
            &mut hits,
        )
        .await?;
    }
    Ok(hits)
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
            query_keys_json TEXT NOT NULL DEFAULT '[]'
        )",
        "CREATE INDEX IF NOT EXISTS asp_graph_entity_kind_idx ON asp_graph_entity(kind)",
        "CREATE TABLE IF NOT EXISTS asp_search_document (
            namespace TEXT NOT NULL,
            document_id TEXT NOT NULL,
            entity_id TEXT NOT NULL,
            selector TEXT,
            document TEXT NOT NULL,
            PRIMARY KEY(namespace, document_id)
        )",
        "CREATE INDEX IF NOT EXISTS asp_search_document_entity_idx ON asp_search_document(entity_id)",
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
    ] {
        connection
            .execute(statement, ())
            .await
            .map_err(|error| format!("failed to bootstrap Turso search schema: {error}"))?;
    }
    Ok(())
}

#[cfg(feature = "turso-backend")]
async fn connect_turso_client_db(db_path: &Path) -> Result<turso::Connection, String> {
    let turso_path = db_path.with_file_name(TURSO_CLIENT_DB_FILE);
    let database = turso::Builder::new_local(turso_path.to_string_lossy().as_ref())
        .build()
        .await
        .map_err(|error| format!("failed to open Turso client DB: {error}"))?;
    database
        .connect()
        .map_err(|error| format!("failed to connect Turso client DB: {error}"))
}

#[cfg(feature = "turso-backend")]
async fn collect_turso_search_hits(
    connection: &turso::Connection,
    source: &'static str,
    sql: &str,
    like_query: &str,
    limit: u32,
    hits: &mut Vec<TursoClientDbSearchHit>,
) -> Result<(), String> {
    let mut rows = connection
        .query(sql, (like_query, limit))
        .await
        .map_err(|error| format!("failed to query Turso search documents: {error}"))?;
    while let Some(row) = rows
        .next()
        .await
        .map_err(|error| format!("failed to read Turso search row: {error}"))?
    {
        let document_id = row
            .get::<String>(0)
            .map_err(|error| format!("failed to read Turso document id: {error}"))?;
        let selector = row
            .get::<Option<String>>(1)
            .map_err(|error| format!("failed to read Turso selector: {error}"))?;
        let document = row
            .get::<String>(2)
            .map_err(|error| format!("failed to read Turso document body: {error}"))?;
        hits.push(TursoClientDbSearchHit {
            source,
            document_id,
            selector,
            document,
        });
    }
    Ok(())
}
