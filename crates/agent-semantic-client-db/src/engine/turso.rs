//! Turso DB Engine adapter for `client.turso` state.

use std::path::{Path, PathBuf};

use serde::Serialize;

use super::contract::{
    ClientDbBackend, ClientDbEngineBackend, ClientDbEngineDurability, ClientDbEngineFeatures,
};
use super::turso_statement::{execute_turso_statement, run_turso_operation};

const TURSO_CLIENT_DB_FILE: &str = "facts.turso";
const TURSO_SEARCH_PROJECTION_DB_FILE: &str = "search-projection.turso";
const TURSO_CLIENT_DB_SCHEMA_VERSION: i64 = 1;
const TURSO_CLIENT_DB_PHYSICAL_FORMAT_ID: &str = "turso-0.7-native";
const TURSO_CLIENT_DB_FORMAT_RECEIPT_FILE: &str = "facts.turso.format.v1.json";
const TURSO_SEARCH_PROJECTION_DB_FORMAT_RECEIPT_FILE: &str =
    "search-projection.turso.format.v1.json";
const TURSO_CLIENT_DB_SCHEMA_BOOTSTRAP_PENDING: &str = "pending-cutover";
const TURSO_CLIENT_DB_SCHEMA_BOOTSTRAP_READY: &str = "ready";
const TURSO_CLIENT_DB_INDEX_METHOD: bool = true;
const TURSO_CLIENT_DB_MVCC_ENABLED: bool = true;
const TURSO_CLIENT_DB_BEGIN_CONCURRENT_ENABLED: bool = false;

/// Bootstrap metadata table used to record the Turso DB Engine schema version.
pub const TURSO_BOOTSTRAP_TABLE: &str = "asp_db_engine_bootstrap";

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
            concurrent_writes: true,
            fts: true,
            fts_index_method: TURSO_CLIENT_DB_INDEX_METHOD,
            vector: false,
            overlay_search: true,
            sync: false,
            encryption: false,
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

async fn turso_physical_format_is_current(connection: &turso::Connection) -> Result<bool, String> {
    let mut rows = connection
        .query(
            "SELECT 1
             FROM asp_db_engine_format
             WHERE format_id = ?1
               AND turso_version = ?2
               AND logical_schema_version = ?3
             LIMIT 1",
            (
                TURSO_CLIENT_DB_PHYSICAL_FORMAT_ID,
                "0.7",
                TURSO_CLIENT_DB_SCHEMA_VERSION,
            ),
        )
        .await
        .map_err(|error| format!("failed to inspect Turso physical-format authority: {error}"))?;
    rows.next()
        .await
        .map(|row| row.is_some())
        .map_err(|error| format!("failed to read Turso physical-format authority: {error}"))
}

pub(super) fn prepare_turso_client_db_path(db_path: &Path) -> Result<PathBuf, String> {
    if let Some(parent) = db_path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|error| format!("failed to create Turso client DB dir: {error}"))?;
    }
    let turso_path = db_path.with_file_name(TURSO_CLIENT_DB_FILE);
    ensure_no_active_turso_migration(&turso_path)?;
    ensure_turso_0_7_format_receipt(&turso_path)?;
    Ok(turso_path)
}

pub(super) fn turso_0_7_format_receipt_path(turso_path: &Path) -> PathBuf {
    let receipt_file = if turso_path.file_name().and_then(|name| name.to_str())
        == Some(TURSO_SEARCH_PROJECTION_DB_FILE)
    {
        TURSO_SEARCH_PROJECTION_DB_FORMAT_RECEIPT_FILE
    } else {
        TURSO_CLIENT_DB_FORMAT_RECEIPT_FILE
    };
    turso_path.with_file_name(receipt_file)
}

fn ensure_turso_0_7_format_receipt(turso_path: &Path) -> Result<(), String> {
    if !turso_path.exists() {
        return Ok(());
    }
    let receipt_path = turso_0_7_format_receipt_path(turso_path);
    if !receipt_path.is_file() {
        return Err(format!(
            "existing client DB `{}` has no Turso 0.7 format receipt `{}`; full staging migration is required and in-place compatibility bootstrap is forbidden",
            turso_path.display(),
            receipt_path.display()
        ));
    }
    Ok(())
}

fn ensure_no_active_turso_migration(turso_path: &Path) -> Result<(), String> {
    let Some(client_dir) = turso_path.parent() else {
        return Ok(());
    };
    let marker_path =
        client_dir.join(super::turso_migration::TURSO_0_7_ACTIVE_MIGRATION_MARKER_FILE);
    if marker_path.is_file() {
        return Err(format!(
            "active Turso 0.7 cutover is in progress for `{}`; retry after migration completes",
            client_dir.display()
        ));
    }
    Ok(())
}

pub(super) fn write_turso_0_7_format_receipt(turso_path: &Path) -> Result<(), String> {
    let receipt_path = turso_0_7_format_receipt_path(turso_path);
    let temporary_path = receipt_path.with_extension(format!(
        "json.tmp-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(|error| format!("failed to timestamp Turso 0.7 format receipt: {error}"))?
            .as_nanos()
    ));
    let receipt = serde_json::to_vec_pretty(&serde_json::json!({
        "schemaId": "agent.semantic-protocols.turso-client-db-format-receipt",
        "schemaVersion": "1",
        "physicalFormat": TURSO_CLIENT_DB_PHYSICAL_FORMAT_ID,
        "tursoVersion": "0.7",
        "logicalSchemaVersion": TURSO_CLIENT_DB_SCHEMA_VERSION,
        "migrationId": "client-db-v1-project-partition-cutover",
    }))
    .map_err(|error| format!("failed to encode Turso 0.7 format receipt: {error}"))?;
    std::fs::write(&temporary_path, receipt).map_err(|error| {
        format!(
            "failed to write Turso 0.7 format receipt `{}`: {error}",
            temporary_path.display()
        )
    })?;
    std::fs::rename(&temporary_path, &receipt_path).map_err(|error| {
        format!(
            "failed to promote Turso 0.7 format receipt `{}`: {error}",
            receipt_path.display()
        )
    })
}

pub(super) async fn bootstrap_turso_schema_version(
    connection: &mut turso::Connection,
) -> Result<(), String> {
    execute_turso_statement(
        connection,
        "CREATE TABLE IF NOT EXISTS asp_db_engine_bootstrap (schema_version INTEGER NOT NULL)",
        "failed to bootstrap Turso client DB schema",
    )
    .await?;

    let current_version = {
        let mut rows = connection
            .query(
                "SELECT MAX(schema_version) FROM asp_db_engine_bootstrap",
                (),
            )
            .await
            .map_err(|error| format!("failed to read Turso bootstrap schema row: {error}"))?;
        rows.next()
            .await
            .map_err(|error| format!("failed to advance Turso bootstrap schema row: {error}"))?
            .map(|row| row.get::<Option<i64>>(0))
            .transpose()
            .map_err(|error| format!("failed to decode Turso bootstrap schema row: {error}"))?
            .flatten()
    };

    if let Some(version) = current_version {
        if version > TURSO_CLIENT_DB_SCHEMA_VERSION {
            return Err(format!(
                "unsupported newer Turso client DB schema version {version}; maximum supported version is {TURSO_CLIENT_DB_SCHEMA_VERSION}"
            ));
        }
        if version == TURSO_CLIENT_DB_SCHEMA_VERSION {
            let mut complete = true;
            for table in [
                "asp_db_engine_migration",
                "asp_db_engine_format",
                "asp_artifact_pointer",
                "asp_failed_artifact_attempt",
            ] {
                if !turso_table_exists(connection, table).await? {
                    complete = false;
                }
            }
            if complete && turso_physical_format_is_current(connection).await? {
                return Ok(());
            }
        }
        if version != 1 {
            return Err(format!(
                "unsupported older Turso client DB schema version {version}; expected version 1 for migration"
            ));
        }
    }

    let transaction = connection
        .transaction_with_behavior(turso::transaction::TransactionBehavior::Immediate)
        .await
        .map_err(|error| {
            format!("failed to begin Turso client DB schema stabilization: {error}")
        })?;
    let stabilization = async {
        transaction
            .execute_batch(
                "CREATE TABLE IF NOT EXISTS asp_db_engine_migration (\
                    schema_version INTEGER PRIMARY KEY,\
                    migration_id TEXT NOT NULL UNIQUE,\
                    applied_at_ms INTEGER NOT NULL\
                 )",
            )
            .await
            .map_err(|error| format!("failed to create Turso schema history: {error}"))?;
        transaction
            .execute_batch(
                "CREATE TABLE IF NOT EXISTS asp_db_engine_format (\
                    format_id TEXT PRIMARY KEY,\
                    turso_version TEXT NOT NULL,\
                    logical_schema_version INTEGER NOT NULL,\
                    migration_id TEXT NOT NULL,\
                    promoted_at_ms INTEGER NOT NULL\
                 )",
            )
            .await
            .map_err(|error| {
                format!("failed to create Turso physical-format authority: {error}")
            })?;
        transaction
            .execute(
                "INSERT OR REPLACE INTO asp_db_engine_format (\
                    format_id, turso_version, logical_schema_version, migration_id, promoted_at_ms\
                 ) VALUES (?1, ?2, ?3, ?4, ?5)",
                (
                    TURSO_CLIENT_DB_PHYSICAL_FORMAT_ID,
                    "0.7",
                    TURSO_CLIENT_DB_SCHEMA_VERSION,
                    "client-db-v1-project-partition-cutover",
                    0_i64,
                ),
            )
            .await
            .map_err(|error| {
                format!("failed to record Turso 0.7 physical-format authority: {error}")
            })?;
        transaction
            .execute_batch(crate::artifact_pointer_store::CREATE_SCHEMA_SQL)
            .await
            .map_err(|error| format!("failed to stabilize artifact authority schema: {error}"))?;
        transaction
            .execute(
                "INSERT OR IGNORE INTO asp_db_engine_migration (\
                    schema_version, migration_id, applied_at_ms\
                 ) VALUES (?1, ?2, ?3)",
                (
                    TURSO_CLIENT_DB_SCHEMA_VERSION,
                    "client-db-v1-stable-artifact-authority",
                    0_i64,
                ),
            )
            .await
            .map_err(|error| format!("failed to record Turso client DB stabilization: {error}"))?;
        transaction
            .execute("DELETE FROM asp_db_engine_bootstrap", ())
            .await
            .map_err(|error| format!("failed to replace Turso bootstrap schema row: {error}"))?;
        transaction
            .execute(
                "INSERT INTO asp_db_engine_bootstrap (schema_version) VALUES (?1)",
                [TURSO_CLIENT_DB_SCHEMA_VERSION],
            )
            .await
            .map_err(|error| format!("failed to write Turso bootstrap schema row: {error}"))?;
        Ok::<(), String>(())
    }
    .await;

    match stabilization {
        Ok(()) => transaction.commit().await.map_err(|error| {
            format!("failed to commit Turso client DB schema stabilization: {error}")
        }),
        Err(error) => {
            let rollback = transaction.rollback().await;
            match rollback {
                Ok(()) => Err(error),
                Err(rollback_error) => Err(format!(
                    "{error}; additionally failed to roll back Turso client DB schema stabilization: {rollback_error}"
                )),
            }
        }
    }
}

pub(super) fn turso_bootstrap_report(db_path: &Path) -> TursoClientDbEngineReport {
    let backend = TursoClientDbEngineBackend;
    let mut report = backend.inspect(db_path);
    report.status = "bootstrap-smoke";
    report.schema_bootstrap = TURSO_CLIENT_DB_SCHEMA_BOOTSTRAP_READY;
    report.reason = None;
    report
}

pub(super) async fn open_turso_client_db_read_only(
    turso_path: PathBuf,
) -> Result<turso::Connection, String> {
    ensure_no_active_turso_migration(&turso_path)?;
    ensure_turso_0_7_format_receipt(&turso_path)?;
    shared_turso_read_only_connection(&turso_path).await
}

pub(super) async fn validate_turso_0_7_migration_target(turso_path: PathBuf) -> Result<(), String> {
    ensure_turso_0_7_format_receipt(&turso_path)?;
    let connection = shared_turso_read_only_connection(&turso_path).await?;
    drop(connection);
    Ok(())
}

fn turso_builder(turso_path: &Path) -> turso::Builder {
    turso::Builder::new_local(turso_path.to_string_lossy().as_ref())
        .experimental_index_method(TURSO_CLIENT_DB_INDEX_METHOD)
}

type TursoSchemaState = std::sync::Arc<
    tokio::sync::Mutex<
        std::collections::HashMap<&'static str, std::sync::Arc<tokio::sync::OnceCell<()>>>,
    >,
>;

/// A connection paired with the shared database authority that created it.
pub(super) struct TursoConnectionLease {
    _database: std::sync::Arc<turso::Database>,
    connection: tokio::sync::OwnedMutexGuard<turso::Connection>,
    schema_state: TursoSchemaState,
}

impl TursoConnectionLease {
    pub(super) async fn schema_bootstrap_state(
        &self,
        schema_id: &'static str,
    ) -> std::sync::Arc<tokio::sync::OnceCell<()>> {
        let mut states = self.schema_state.lock().await;
        std::sync::Arc::clone(
            states
                .entry(schema_id)
                .or_insert_with(|| std::sync::Arc::new(tokio::sync::OnceCell::new())),
        )
    }
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
    database: tokio::sync::OnceCell<std::sync::Arc<turso::Database>>,
    write_lanes: tokio::sync::RwLock<Vec<std::sync::Arc<tokio::sync::Mutex<turso::Connection>>>>,
    next_write_lane: std::sync::atomic::AtomicUsize,
    schema_state: TursoSchemaState,
}

type TursoDatabasePool =
    std::collections::BTreeMap<std::path::PathBuf, std::sync::Arc<TursoDatabasePoolEntry>>;

fn turso_database_pool() -> &'static tokio::sync::Mutex<TursoDatabasePool> {
    static POOL: std::sync::OnceLock<tokio::sync::Mutex<TursoDatabasePool>> =
        std::sync::OnceLock::new();
    POOL.get_or_init(|| tokio::sync::Mutex::new(TursoDatabasePool::new()))
}

pub(crate) async fn shared_turso_database(
    turso_path: &Path,
) -> Result<std::sync::Arc<turso::Database>, String> {
    let entry = shared_turso_pool_entry(turso_path).await;
    let database = entry
        .database
        .get_or_try_init(|| async {
            build_turso_database(turso_path)
                .await
                .map(std::sync::Arc::new)
        })
        .await?;
    Ok(std::sync::Arc::clone(database))
}

async fn shared_turso_pool_entry(turso_path: &Path) -> std::sync::Arc<TursoDatabasePoolEntry> {
    let mut pool = turso_database_pool().lock().await;
    std::sync::Arc::clone(pool.entry(turso_path.to_path_buf()).or_insert_with(|| {
        std::sync::Arc::new(TursoDatabasePoolEntry {
            database: tokio::sync::OnceCell::new(),
            write_lanes: tokio::sync::RwLock::new(Vec::new()),
            next_write_lane: std::sync::atomic::AtomicUsize::new(0),
            schema_state: std::sync::Arc::new(tokio::sync::Mutex::new(
                std::collections::HashMap::new(),
            )),
        })
    }))
}

pub(super) async fn evict_turso_client_dir(client_dir: &Path) {
    let mut pool = turso_database_pool().lock().await;
    pool.remove(&client_dir.join(TURSO_CLIENT_DB_FILE));
    pool.remove(&client_dir.join(TURSO_SEARCH_PROJECTION_DB_FILE));
}

async fn configure_turso_write_connection(
    connection: &turso::Connection,
    mvcc_enabled: bool,
) -> Result<(), String> {
    if !mvcc_enabled {
        return Ok(());
    }

    let mut rows = connection
        .query("PRAGMA journal_mode = 'mvcc'", ())
        .await
        .map_err(|error| format!("failed to enable Turso client DB MVCC: {error}"))?;
    let row = rows
        .next()
        .await
        .map_err(|error| format!("failed to read Turso client DB journal mode: {error}"))?
        .ok_or_else(|| "Turso client DB journal mode returned no row".to_string())?;
    let journal_mode = row
        .get::<String>(0)
        .map_err(|error| format!("failed to decode Turso client DB journal mode: {error}"))?;
    if journal_mode != "mvcc" {
        return Err(format!(
            "Turso client DB requires journal_mode=mvcc, observed {journal_mode}"
        ));
    }
    Ok(())
}

fn adaptive_turso_write_lane_ceiling() -> usize {
    std::thread::available_parallelism()
        .map(std::num::NonZeroUsize::get)
        .unwrap_or(1)
}

async fn new_turso_write_lane(
    database: &turso::Database,
    turso_path: &Path,
) -> Result<std::sync::Arc<tokio::sync::Mutex<turso::Connection>>, String> {
    let connection = database
        .connect()
        .map_err(|error| format!("failed to connect Turso client DB write lane: {error}"))?;
    configure_turso_write_connection(
        &connection,
        TURSO_CLIENT_DB_MVCC_ENABLED
            && turso_path.file_name().and_then(|name| name.to_str())
                != Some(TURSO_SEARCH_PROJECTION_DB_FILE),
    )
    .await?;
    Ok(std::sync::Arc::new(tokio::sync::Mutex::new(connection)))
}

async fn shared_turso_write_connection(turso_path: &Path) -> Result<TursoConnectionLease, String> {
    let entry = shared_turso_pool_entry(turso_path).await;
    let database = shared_turso_database(turso_path).await?;
    let mut lanes = {
        let lanes = entry.write_lanes.read().await;
        lanes.iter().cloned().collect::<Vec<_>>()
    };
    if lanes.is_empty() {
        let mut published = entry.write_lanes.write().await;
        if published.is_empty() {
            published.push(new_turso_write_lane(&database, turso_path).await?);
        }
        lanes = published.iter().cloned().collect();
    }

    let first_lane = entry
        .next_write_lane
        .fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        % lanes.len();
    for offset in 0..lanes.len() {
        let lane = std::sync::Arc::clone(&lanes[(first_lane + offset) % lanes.len()]);
        if let Ok(connection) = lane.try_lock_owned() {
            return Ok(TursoConnectionLease {
                _database: database,
                connection,
                schema_state: std::sync::Arc::clone(&entry.schema_state),
            });
        }
    }

    let lane = if lanes.len() < adaptive_turso_write_lane_ceiling() {
        let mut published = entry.write_lanes.write().await;
        if published.len() < adaptive_turso_write_lane_ceiling() {
            let lane = new_turso_write_lane(&database, turso_path).await?;
            published.push(std::sync::Arc::clone(&lane));
            lane
        } else {
            std::sync::Arc::clone(&published[first_lane % published.len()])
        }
    } else {
        std::sync::Arc::clone(&lanes[first_lane])
    };
    let connection = lane.lock_owned().await;
    Ok(TursoConnectionLease {
        _database: database,
        connection,
        schema_state: std::sync::Arc::clone(&entry.schema_state),
    })
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

async fn build_turso_database(turso_path: &Path) -> Result<turso::Database, String> {
    let max_attempts = 8;
    let mut last_lock_error = None;
    for attempt in 0..max_attempts {
        match turso_builder(turso_path).build().await {
            Ok(database) => return Ok(database),
            Err(error) => {
                let message = error.to_string();
                if !super::turso_lock_policy::is_turso_lock_error(&message) {
                    return Err(format!("failed to open Turso client DB: {message}"));
                }
                last_lock_error = Some(message);
                if attempt + 1 == max_attempts {
                    break;
                }
                tokio::time::sleep(super::turso_lock_policy::turso_lock_retry_delay(attempt)).await;
            }
        }
    }
    Err(format!(
        "failed to open Turso client DB after bounded lock retries: {}",
        last_lock_error.unwrap_or_else(|| "unknown Turso lock error".to_string())
    ))
}

/// Open an unreceipted legacy file only for bounded, read-only Turso 0.7 migration.
pub(super) async fn open_turso_0_7_migration_source(
    turso_path: &Path,
) -> Result<(turso::Database, turso::Connection), String> {
    let database = build_turso_database(turso_path).await?;
    let connection = database
        .connect()
        .map_err(|error| format!("failed to connect Turso 0.7 migration source: {error}"))?;
    connection
        .execute("PRAGMA query_only = 1", ())
        .await
        .map_err(|error| {
            format!("failed to enforce read-only Turso 0.7 migration source: {error}")
        })?;
    Ok((database, connection))
}

pub(super) fn turso_client_db_exists(db_path: &Path) -> bool {
    db_path.with_file_name(TURSO_CLIENT_DB_FILE).exists()
}

pub(super) async fn connect_turso_client_db(
    db_path: &Path,
) -> Result<TursoConnectionLease, String> {
    let turso_path = db_path.with_file_name(TURSO_CLIENT_DB_FILE);
    shared_turso_write_connection(&turso_path).await
}

pub(super) fn turso_search_projection_db_path(db_path: &Path) -> PathBuf {
    db_path.with_file_name(TURSO_SEARCH_PROJECTION_DB_FILE)
}

pub(super) async fn connect_turso_search_projection_db_for_write(
    db_path: &Path,
) -> Result<TursoConnectionLease, String> {
    shared_turso_write_connection(&turso_search_projection_db_path(db_path)).await
}

pub(super) async fn connect_turso_search_projection_db_read_only(
    db_path: &Path,
) -> Result<turso::Connection, String> {
    shared_turso_read_only_connection(&turso_search_projection_db_path(db_path)).await
}

pub(super) async fn connect_turso_client_db_read_only(
    db_path: &Path,
) -> Result<turso::Connection, String> {
    let turso_path = db_path.with_file_name(TURSO_CLIENT_DB_FILE);
    open_turso_client_db_read_only(turso_path).await
}

pub(super) async fn turso_table_exists(
    connection: &turso::Connection,
    table_name: &str,
) -> Result<bool, String> {
    let mut rows = run_turso_operation(
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

#[cfg(test)]
#[path = "../../tests/unit/db/engine/turso_pool.rs"]
mod tests;
