//! Turso DB Engine adapter for `client.turso` state.

use std::path::{Path, PathBuf};
use std::time::Duration;

use serde::Serialize;

use super::contract::{
    ClientDbBackend, ClientDbEngineBackend, ClientDbEngineDurability, ClientDbEngineFeatures,
};
use super::turso_lock_policy::{
    TURSO_CLIENT_DB_BUSY_TIMEOUT_MS, TURSO_CLIENT_DB_LOCK_RETRY_ATTEMPTS,
    TURSO_CLIENT_DB_LOCK_RETRY_BASE_MS, TURSO_CLIENT_DB_LOCK_RETRY_MAX_MS,
    TURSO_CLIENT_DB_OPERATION_LOCK_RETRY_ATTEMPTS, TURSO_CLIENT_DB_OPERATION_LOCK_RETRY_MS,
    TURSO_CLIENT_DB_STATEMENT_LOCK_RETRY_ATTEMPTS, is_turso_lock_error, turso_lock_retry_delay,
};
use super::turso_statement::{
    execute_turso_operation_with_lock_retry, execute_turso_statement_with_lock_retry,
    run_turso_operation_with_lock_retry,
};

const TURSO_CLIENT_DB_FILE: &str = "client.turso";
const TURSO_CLIENT_DB_SCHEMA_VERSION: i64 = 1;
const TURSO_CLIENT_DB_SCHEMA_BOOTSTRAP_PENDING: &str = "pending-cutover";
const TURSO_CLIENT_DB_SCHEMA_BOOTSTRAP_READY: &str = "ready";
const TURSO_CLIENT_DB_INDEX_METHOD: bool = true;
const TURSO_CLIENT_DB_MULTIPROCESS_WAL: bool = true;
const TURSO_CLIENT_DB_OPERATION_LOCK_ENABLED: bool = true;
const TURSO_CLIENT_DB_MVCC_ENABLED: bool = false;
const TURSO_CLIENT_DB_BEGIN_CONCURRENT_ENABLED: bool = false;

/// Bootstrap metadata table used to record the Turso DB Engine schema version.
pub const TURSO_BOOTSTRAP_TABLE: &str = "asp_db_engine_bootstrap";
/// Stable search-document table for generated selector/search projections.
pub const TURSO_SEARCH_DOCUMENT_TABLE: &str = "asp_search_document";
/// Session-scoped dirty overlay document table for dynamic search.
pub const TURSO_OVERLAY_DOCUMENT_TABLE: &str = "asp_overlay_document";
/// Bounded search route receipt table for replay and ranking feedback.
pub const TURSO_ROUTE_RECEIPT_TABLE: &str = "asp_route_receipt";

/// Diagnostic report for the Turso DB Engine backend.
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
            concurrent_writes: false,
            fts: true,
            fts_index_method: TURSO_CLIENT_DB_INDEX_METHOD,
            vector: false,
            overlay_search: true,
            sync: false,
            encryption: false,
            multi_process_wal: TURSO_CLIENT_DB_MULTIPROCESS_WAL,
            serialized_writer_slot: true,
            busy_timeout_ms: TURSO_CLIENT_DB_BUSY_TIMEOUT_MS,
            open_lock_retry_attempts: TURSO_CLIENT_DB_LOCK_RETRY_ATTEMPTS,
            open_lock_retry_base_ms: TURSO_CLIENT_DB_LOCK_RETRY_BASE_MS,
            open_lock_retry_max_ms: TURSO_CLIENT_DB_LOCK_RETRY_MAX_MS,
            statement_lock_retry_attempts: TURSO_CLIENT_DB_STATEMENT_LOCK_RETRY_ATTEMPTS,
            operation_lock: TURSO_CLIENT_DB_OPERATION_LOCK_ENABLED,
            operation_lock_retry_attempts: TURSO_CLIENT_DB_OPERATION_LOCK_RETRY_ATTEMPTS,
            operation_lock_retry_ms: TURSO_CLIENT_DB_OPERATION_LOCK_RETRY_MS,
            mvcc: TURSO_CLIENT_DB_MVCC_ENABLED,
            begin_concurrent: TURSO_CLIENT_DB_BEGIN_CONCURRENT_ENABLED,
        }
    }

    fn inspect(&self, db_path: &Path) -> TursoClientDbEngineReport {
        let active_db_path = db_path.with_file_name(self.db_file_name());
        let (status, schema_bootstrap, reason) = if active_db_path.exists() {
            ("ready", TURSO_CLIENT_DB_SCHEMA_BOOTSTRAP_READY, None)
        } else {
            ("missing", TURSO_CLIENT_DB_SCHEMA_BOOTSTRAP_PENDING, None)
        };
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

pub(super) fn prepare_turso_client_db_path(db_path: &Path) -> Result<PathBuf, String> {
    if let Some(parent) = db_path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|error| format!("failed to create Turso client DB dir: {error}"))?;
    }
    Ok(db_path.with_file_name(TURSO_CLIENT_DB_FILE))
}

pub(super) async fn bootstrap_turso_schema_version(
    connection: &turso::Connection,
) -> Result<(), String> {
    execute_turso_statement_with_lock_retry(
        connection,
        "CREATE TABLE IF NOT EXISTS asp_db_engine_bootstrap (schema_version INTEGER NOT NULL)",
        "failed to bootstrap Turso client DB schema",
    )
    .await?;
    execute_turso_statement_with_lock_retry(
        connection,
        "DELETE FROM asp_db_engine_bootstrap",
        "failed to reset Turso bootstrap schema row",
    )
    .await?;
    execute_turso_operation_with_lock_retry(
        || async {
            connection
                .execute(
                    "INSERT INTO asp_db_engine_bootstrap (schema_version) VALUES (?1)",
                    [TURSO_CLIENT_DB_SCHEMA_VERSION],
                )
                .await
                .map_err(|error| error.to_string())
        },
        "failed to write Turso bootstrap schema row",
    )
    .await?;
    Ok(())
}

pub(super) fn turso_bootstrap_report(db_path: &Path) -> TursoClientDbEngineReport {
    let backend = TursoClientDbEngineBackend;
    let mut report = backend.inspect(db_path);
    report.status = "bootstrap-smoke";
    report.schema_bootstrap = TURSO_CLIENT_DB_SCHEMA_BOOTSTRAP_READY;
    report.reason = None;
    report
}

async fn open_turso_client_db_read_only(turso_path: PathBuf) -> Result<turso::Connection, String> {
    shared_turso_read_only_connection(&turso_path).await
}

fn turso_builder(turso_path: &Path) -> turso::Builder {
    turso::Builder::new_local(turso_path.to_string_lossy().as_ref())
        .experimental_index_method(TURSO_CLIENT_DB_INDEX_METHOD)
        .experimental_multiprocess_wal(TURSO_CLIENT_DB_MULTIPROCESS_WAL)
}

/// A connection paired with the shared database authority that created it.
pub(super) struct TursoConnectionLease {
    _database: std::sync::Arc<turso::Database>,
    connection: turso::Connection,
}

impl std::ops::Deref for TursoConnectionLease {
    type Target = turso::Connection;

    fn deref(&self) -> &Self::Target {
        &self.connection
    }
}

impl std::ops::DerefMut for TursoConnectionLease {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.connection
    }
}

struct TursoDatabasePoolEntry {
    database: std::sync::Arc<turso::Database>,
}

type TursoDatabasePool = std::collections::BTreeMap<std::path::PathBuf, TursoDatabasePoolEntry>;

fn turso_database_pool() -> &'static tokio::sync::Mutex<TursoDatabasePool> {
    static POOL: std::sync::OnceLock<tokio::sync::Mutex<TursoDatabasePool>> =
        std::sync::OnceLock::new();
    POOL.get_or_init(|| tokio::sync::Mutex::new(TursoDatabasePool::new()))
}

async fn shared_turso_database(
    turso_path: &Path,
) -> Result<std::sync::Arc<turso::Database>, String> {
    let mut pool = turso_database_pool().lock().await;
    if let Some(entry) = pool.get(turso_path) {
        return Ok(std::sync::Arc::clone(&entry.database));
    }
    let database = std::sync::Arc::new(build_turso_database_with_lock_retry(turso_path).await?);
    pool.insert(
        turso_path.to_path_buf(),
        TursoDatabasePoolEntry {
            database: std::sync::Arc::clone(&database),
        },
    );
    Ok(database)
}

async fn shared_turso_read_only_connection(turso_path: &Path) -> Result<turso::Connection, String> {
    let database = shared_turso_database(turso_path).await?;
    let connection = database
        .connect()
        .map_err(|error| format!("failed to connect Turso client DB read-only: {error}"))?;
    connection
        .execute("PRAGMA query_only = 1", ())
        .await
        .map_err(|error| format!("failed to enforce Turso client DB read-only mode: {error}"))?;
    Ok(connection)
}

async fn build_turso_database_with_lock_retry(
    turso_path: &Path,
) -> Result<turso::Database, String> {
    let mut last_lock_error = None;
    for attempt in 0..TURSO_CLIENT_DB_LOCK_RETRY_ATTEMPTS {
        match turso_builder(turso_path).build().await {
            Ok(database) => return Ok(database),
            Err(error) => {
                let message = format!("failed to open Turso client DB: {error}");
                if !is_turso_lock_error(&message) {
                    return Err(message);
                }
                last_lock_error = Some(message);
            }
        }
        tokio::time::sleep(turso_lock_retry_delay(attempt)).await;
    }
    Err(format!(
        "{} after {} retry attempts",
        last_lock_error.unwrap_or_else(|| format!(
            "failed to open Turso client DB: lock persisted for {}",
            turso_path.display()
        )),
        TURSO_CLIENT_DB_LOCK_RETRY_ATTEMPTS
    ))
}

pub(super) fn turso_client_db_exists(db_path: &Path) -> bool {
    db_path.with_file_name(TURSO_CLIENT_DB_FILE).exists()
}

pub(super) async fn connect_turso_client_db(
    db_path: &Path,
) -> Result<TursoConnectionLease, String> {
    let turso_path = db_path.with_file_name(TURSO_CLIENT_DB_FILE);
    let database = shared_turso_database(&turso_path).await?;
    let connection = database
        .connect()
        .map_err(|error| format!("failed to connect Turso client DB: {error}"))?;
    connection
        .busy_timeout(Duration::from_millis(TURSO_CLIENT_DB_BUSY_TIMEOUT_MS))
        .map_err(|error| format!("failed to configure Turso client DB busy timeout: {error}"))?;
    Ok(TursoConnectionLease {
        _database: database,
        connection,
    })
}

pub(super) async fn connect_turso_client_db_read_only(
    db_path: &Path,
) -> Result<turso::Connection, String> {
    let turso_path = db_path.with_file_name(TURSO_CLIENT_DB_FILE);
    connect_turso_client_db_read_only_file(
        &turso_path,
        TURSO_CLIENT_DB_LOCK_RETRY_ATTEMPTS,
        TURSO_CLIENT_DB_BUSY_TIMEOUT_MS,
    )
    .await
}

async fn connect_turso_client_db_read_only_file(
    turso_path: &Path,
    retry_attempts: usize,
    busy_timeout_ms: u64,
) -> Result<turso::Connection, String> {
    connect_turso_client_db_file_with_opener(
        turso_path,
        retry_attempts,
        busy_timeout_ms,
        open_turso_client_db_read_only,
    )
    .await
}

async fn connect_turso_client_db_file_with_opener<F, Fut>(
    turso_path: &Path,
    retry_attempts: usize,
    busy_timeout_ms: u64,
    open: F,
) -> Result<turso::Connection, String>
where
    F: Fn(PathBuf) -> Fut,
    Fut: std::future::Future<Output = Result<turso::Connection, String>>,
{
    let mut last_lock_error = None;
    for attempt in 0..retry_attempts {
        match open(turso_path.to_path_buf()).await {
            Ok(connection) => {
                connection
                    .busy_timeout(Duration::from_millis(busy_timeout_ms))
                    .map_err(|error| {
                        format!("failed to configure Turso client DB busy timeout: {error}")
                    })?;
                return Ok(connection);
            }
            Err(message) => {
                if !is_turso_lock_error(&message) {
                    return Err(message);
                }
                last_lock_error = Some(message);
            }
        }
        tokio::time::sleep(turso_lock_retry_delay(attempt)).await;
    }
    Err(format!(
        "{} after {} retry attempts",
        last_lock_error.unwrap_or_else(|| format!(
            "failed to open Turso client DB: lock persisted for {}",
            turso_path.display()
        )),
        retry_attempts
    ))
}

pub(super) async fn turso_table_exists(
    connection: &turso::Connection,
    table_name: &str,
) -> Result<bool, String> {
    let mut rows = run_turso_operation_with_lock_retry(
        || async {
            connection
                .query("PRAGMA table_list", ())
                .await
                .map_err(|error| error.to_string())
        },
        &format!("failed to inspect Turso table {table_name}"),
    )
    .await?;
    while let Some(row) = rows
        .next()
        .await
        .map_err(|error| format!("failed to read Turso table {table_name} existence: {error}"))?
    {
        let name = row
            .get::<String>(1)
            .map_err(|error| format!("failed to read Turso table list name: {error}"))?;
        if name == table_name {
            return Ok(true);
        }
    }
    Ok(false)
}
