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
const TURSO_CLIENT_DB_SCHEMA_BOOTSTRAP_PENDING: &str = "pending-cutover";
const TURSO_CLIENT_DB_SCHEMA_BOOTSTRAP_READY: &str = "ready";
const TURSO_CLIENT_DB_INDEX_METHOD: bool = true;
const TURSO_CLIENT_DB_MVCC_ENABLED: bool = true;
const TURSO_CLIENT_DB_BEGIN_CONCURRENT_ENABLED: bool = false;
const TURSO_CLIENT_DB_CONNECTION_LANES: usize = 4;

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

pub(super) fn prepare_turso_client_db_path(db_path: &Path) -> Result<PathBuf, String> {
    if let Some(parent) = db_path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|error| format!("failed to create Turso client DB dir: {error}"))?;
    }
    Ok(db_path.with_file_name(TURSO_CLIENT_DB_FILE))
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
                "asp_artifact_pointer",
                "asp_failed_artifact_attempt",
            ] {
                if !turso_table_exists(connection, table).await? {
                    complete = false;
                }
            }
            if complete {
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

async fn open_turso_client_db_read_only(turso_path: PathBuf) -> Result<turso::Connection, String> {
    shared_turso_read_only_connection(&turso_path).await
}

fn turso_builder(turso_path: &Path) -> turso::Builder {
    turso::Builder::new_local(turso_path.to_string_lossy().as_ref())
        .experimental_index_method(TURSO_CLIENT_DB_INDEX_METHOD)
}

/// A connection paired with the shared database authority that created it.
pub(super) struct TursoConnectionLease {
    _database: std::sync::Arc<turso::Database>,
    connection: tokio::sync::OwnedMutexGuard<turso::Connection>,
    schema_state: std::sync::Arc<tokio::sync::Mutex<std::collections::HashSet<&'static str>>>,
}

/// Exclusive first-use bootstrap authority for one logical schema in one database.
pub(super) struct TursoSchemaBootstrapGuard {
    schema_state: tokio::sync::OwnedMutexGuard<std::collections::HashSet<&'static str>>,
    schema_id: &'static str,
}

impl TursoSchemaBootstrapGuard {
    pub(super) fn mark_ready(mut self) {
        self.schema_state.insert(self.schema_id);
    }
}

impl TursoConnectionLease {
    pub(super) async fn begin_schema_bootstrap(
        &self,
        schema_id: &'static str,
    ) -> Option<TursoSchemaBootstrapGuard> {
        let schema_state = std::sync::Arc::clone(&self.schema_state).lock_owned().await;
        if schema_state.contains(schema_id) {
            return None;
        }
        Some(TursoSchemaBootstrapGuard {
            schema_state,
            schema_id,
        })
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
    database: std::sync::Arc<turso::Database>,
    write_lanes: Vec<std::sync::Arc<tokio::sync::Mutex<turso::Connection>>>,
    next_write_lane: usize,
    schema_state: std::sync::Arc<tokio::sync::Mutex<std::collections::HashSet<&'static str>>>,
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
    let database = std::sync::Arc::new(build_turso_database(turso_path).await?);
    pool.insert(
        turso_path.to_path_buf(),
        TursoDatabasePoolEntry {
            database: std::sync::Arc::clone(&database),
            write_lanes: Vec::new(),
            next_write_lane: 0,
            schema_state: std::sync::Arc::new(tokio::sync::Mutex::new(
                std::collections::HashSet::new(),
            )),
        },
    );
    Ok(database)
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

async fn shared_turso_write_connection(turso_path: &Path) -> Result<TursoConnectionLease, String> {
    let (database, lane, schema_state) = {
        let mut pool = turso_database_pool().lock().await;
        if !pool.contains_key(turso_path) {
            let database = std::sync::Arc::new(build_turso_database(turso_path).await?);
            pool.insert(
                turso_path.to_path_buf(),
                TursoDatabasePoolEntry {
                    database,
                    write_lanes: Vec::new(),
                    next_write_lane: 0,
                    schema_state: std::sync::Arc::new(tokio::sync::Mutex::new(
                        std::collections::HashSet::new(),
                    )),
                },
            );
        }

        let entry = pool
            .get_mut(turso_path)
            .expect("Turso database pool entry was inserted above");
        if entry.write_lanes.is_empty() {
            entry.write_lanes.reserve(TURSO_CLIENT_DB_CONNECTION_LANES);
            for _ in 0..TURSO_CLIENT_DB_CONNECTION_LANES {
                let connection = entry.database.connect().map_err(|error| {
                    format!("failed to connect Turso client DB write lane: {error}")
                })?;
                configure_turso_write_connection(
                    &connection,
                    TURSO_CLIENT_DB_MVCC_ENABLED
                        && turso_path.file_name().and_then(|name| name.to_str())
                            != Some(TURSO_SEARCH_PROJECTION_DB_FILE),
                )
                .await?;
                entry
                    .write_lanes
                    .push(std::sync::Arc::new(tokio::sync::Mutex::new(connection)));
            }
        }

        let lane_index = entry.next_write_lane % entry.write_lanes.len();
        entry.next_write_lane = entry.next_write_lane.wrapping_add(1);
        (
            std::sync::Arc::clone(&entry.database),
            std::sync::Arc::clone(&entry.write_lanes[lane_index]),
            std::sync::Arc::clone(&entry.schema_state),
        )
    };
    let connection = lane.lock_owned().await;
    Ok(TursoConnectionLease {
        _database: database,
        connection,
        schema_state,
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
    turso_builder(turso_path)
        .build()
        .await
        .map_err(|error| format!("failed to open Turso client DB: {error}"))
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

pub(super) async fn connect_turso_search_projection_db(
    db_path: &Path,
) -> Result<TursoConnectionLease, String> {
    shared_turso_write_connection(&turso_search_projection_db_path(db_path)).await
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
