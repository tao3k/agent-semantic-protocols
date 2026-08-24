use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::Arc;

#[path = "turso_mvcc_maintenance.rs"]
mod maintenance;
#[path = "turso_mvcc_typed.rs"]
mod typed;

pub use maintenance::{
    TURSO_MVCC_MAINTENANCE_RECEIPT_SCHEMA_ID, TursoMvccMaintenanceReceipt,
};
pub use typed::{TursoMvccWriteError, TursoMvccWriteErrorCode};
use std::time::Duration;

const DEFAULT_CONNECTION_LANES: usize = 4;
const DEFAULT_BUSY_TIMEOUT_MS: u64 = 5_000;
const DEFAULT_RETRY_ATTEMPTS: usize = 16;
const DEFAULT_MAX_BATCH_ROWS: usize = 1_024;
const EVENT_SHARDS: usize = 4;
const MAX_CONNECTION_LANES: usize = EVENT_SHARDS;
pub(crate) const BATCH_WRITE_RECEIPT_SCHEMA_ID: &str = "asp.turso-mvcc-batch-write-receipt.v1";

const CREATE_EVENT_TABLES_SQL: &str = "
CREATE TABLE IF NOT EXISTS asp_mvcc_event_0 (
    partition_key TEXT NOT NULL,
    event_id TEXT NOT NULL,
    payload BLOB NOT NULL,
    created_at_ms INTEGER NOT NULL,
    PRIMARY KEY (partition_key, event_id)
);
CREATE TABLE IF NOT EXISTS asp_mvcc_event_1 (
    partition_key TEXT NOT NULL,
    event_id TEXT NOT NULL,
    payload BLOB NOT NULL,
    created_at_ms INTEGER NOT NULL,
    PRIMARY KEY (partition_key, event_id)
);
CREATE TABLE IF NOT EXISTS asp_mvcc_event_2 (
    partition_key TEXT NOT NULL,
    event_id TEXT NOT NULL,
    payload BLOB NOT NULL,
    created_at_ms INTEGER NOT NULL,
    PRIMARY KEY (partition_key, event_id)
);
CREATE TABLE IF NOT EXISTS asp_mvcc_event_3 (
    partition_key TEXT NOT NULL,
    event_id TEXT NOT NULL,
    payload BLOB NOT NULL,
    created_at_ms INTEGER NOT NULL,
    PRIMARY KEY (partition_key, event_id)
);
CREATE INDEX IF NOT EXISTS asp_mvcc_event_0_keyset
    ON asp_mvcc_event_0 (partition_key, created_at_ms, event_id);
CREATE INDEX IF NOT EXISTS asp_mvcc_event_1_keyset
    ON asp_mvcc_event_1 (partition_key, created_at_ms, event_id);
CREATE INDEX IF NOT EXISTS asp_mvcc_event_2_keyset
    ON asp_mvcc_event_2 (partition_key, created_at_ms, event_id);
CREATE INDEX IF NOT EXISTS asp_mvcc_event_3_keyset
    ON asp_mvcc_event_3 (partition_key, created_at_ms, event_id)";

const INSERT_EVENT_SQL: [&str; EVENT_SHARDS] = [
    "INSERT INTO asp_mvcc_event_0 (partition_key, event_id, payload, created_at_ms) VALUES (?1, ?2, ?3, ?4)",
    "INSERT INTO asp_mvcc_event_1 (partition_key, event_id, payload, created_at_ms) VALUES (?1, ?2, ?3, ?4)",
    "INSERT INTO asp_mvcc_event_2 (partition_key, event_id, payload, created_at_ms) VALUES (?1, ?2, ?3, ?4)",
    "INSERT INTO asp_mvcc_event_3 (partition_key, event_id, payload, created_at_ms) VALUES (?1, ?2, ?3, ?4)",
];

const READ_PARTITION_SQL: [&str; EVENT_SHARDS] = [
    "SELECT partition_key, event_id, payload, created_at_ms FROM asp_mvcc_event_0 WHERE partition_key = ?1 ORDER BY event_id",
    "SELECT partition_key, event_id, payload, created_at_ms FROM asp_mvcc_event_1 WHERE partition_key = ?1 ORDER BY event_id",
    "SELECT partition_key, event_id, payload, created_at_ms FROM asp_mvcc_event_2 WHERE partition_key = ?1 ORDER BY event_id",
    "SELECT partition_key, event_id, payload, created_at_ms FROM asp_mvcc_event_3 WHERE partition_key = ?1 ORDER BY event_id",
];

/// Configuration for the append-oriented Turso 0.7 MVCC authority.
///
/// This store intentionally excludes FTS and multiprocess WAL. Turso 0.7 rejects
/// both custom index modules and multiprocess WAL when `journal_mode=mvcc`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TursoMvccStoreConfig {
    pub path: PathBuf,
    pub connection_lanes: usize,
    pub passive_checkpoint: bool,
    pub busy_timeout_ms: u64,
    pub retry_attempts: usize,
    pub max_batch_rows: usize,
}

impl TursoMvccStoreConfig {
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self {
            path: path.into(),
            passive_checkpoint: false,
            connection_lanes: DEFAULT_CONNECTION_LANES,
            busy_timeout_ms: DEFAULT_BUSY_TIMEOUT_MS,
            retry_attempts: DEFAULT_RETRY_ATTEMPTS,
            max_batch_rows: DEFAULT_MAX_BATCH_ROWS,
        }
    }
}

/// One immutable append record. Stable `(partition_key, event_id)` identity
/// makes duplicate batches fail atomically instead of silently overwriting data.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TursoMvccEvent {
    partition_key: String,
    event_id: String,
    payload: Vec<u8>,
    created_at_ms: i64,
}

impl TursoMvccEvent {
    pub fn new(
        partition_key: crate::turso_mvcc_keyset::TursoMvccPartitionKey,
        event_id: crate::turso_mvcc_keyset::TursoMvccEventId,
        payload: Vec<u8>,
        created_at_ms: i64,
    ) -> Self {
        Self {
            partition_key: partition_key.as_str().to_owned(),
            event_id: event_id.as_str().to_owned(),
            payload,
            created_at_ms,
        }
    }

    pub fn partition_key(&self) -> &str {
        self.partition_key.as_str()
    }

    pub fn event_id(&self) -> &str {
        self.event_id.as_str()
    }

    pub fn payload(&self) -> &[u8] {
        self.payload.as_slice()
    }

    pub fn created_at_ms(&self) -> i64 {
        self.created_at_ms
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
enum TursoMvccOptimizationReceiptSchemaId {
    #[serde(rename = "asp.turso-mvcc-optimization-receipt.v1")]
    V1,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
enum TursoMvccOptimizationProfile {
    #[serde(rename = "async-io-mvcc")]
    Concurrent,
    #[serde(rename = "async-io-mvcc-passive-checkpoint")]
    ConcurrentPassiveCheckpoint,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
enum TursoMvccStatementCacheMode {
    #[serde(rename = "prepared-cached-per-connection")]
    PreparedPerConnection,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
enum TursoMvccTransactionMode {
    #[serde(rename = "begin-concurrent")]
    Concurrent,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(transparent)]
struct TursoMvccConnectionLaneCount(usize);

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(transparent)]
struct TursoMvccPartitionShardCount(usize);

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(transparent)]
struct TursoMvccInsertRowsPerStatement(usize);

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(transparent)]
struct TursoMvccFeatureState(bool);

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TursoMvccOptimizationReceipt {
    schema_id: TursoMvccOptimizationReceiptSchemaId,
    profile: TursoMvccOptimizationProfile,
    connection_lanes: TursoMvccConnectionLaneCount,
    partition_shards: TursoMvccPartitionShardCount,
    statement_cache: TursoMvccStatementCacheMode,
    insert_rows_per_statement: TursoMvccInsertRowsPerStatement,
    transaction_mode: TursoMvccTransactionMode,
    mvcc: TursoMvccFeatureState,
    passive_checkpoint: TursoMvccFeatureState,
    multiprocess_wal: TursoMvccFeatureState,
    fts: TursoMvccFeatureState,
}

impl TursoMvccOptimizationReceipt {
    /// Stable schema identity of this optimization receipt.
    #[must_use]
    pub const fn schema_id(&self) -> &'static str {
        match self.schema_id {
            TursoMvccOptimizationReceiptSchemaId::V1 => "asp.turso-mvcc-optimization-receipt.v1",
        }
    }

    /// Effective connection and checkpoint optimization profile.
    #[must_use]
    pub const fn profile(&self) -> &'static str {
        match self.profile {
            TursoMvccOptimizationProfile::Concurrent => "async-io-mvcc",
            TursoMvccOptimizationProfile::ConcurrentPassiveCheckpoint => {
                "async-io-mvcc-passive-checkpoint"
            }
        }
    }

    /// Number of independent connection lanes.
    #[must_use]
    pub const fn connection_lanes(&self) -> usize {
        self.connection_lanes.0
    }

    /// Number of deterministic partition shards.
    #[must_use]
    pub const fn partition_shards(&self) -> usize {
        self.partition_shards.0
    }

    /// Prepared statement cache strategy.
    #[must_use]
    pub const fn statement_cache(&self) -> &'static str {
        match self.statement_cache {
            TursoMvccStatementCacheMode::PreparedPerConnection => "prepared-cached-per-connection",
        }
    }

    /// Maximum rows emitted by one insert statement.
    #[must_use]
    pub const fn insert_rows_per_statement(&self) -> usize {
        self.insert_rows_per_statement.0
    }

    /// Transaction mode used by write sessions.
    #[must_use]
    pub const fn transaction_mode(&self) -> &'static str {
        match self.transaction_mode {
            TursoMvccTransactionMode::Concurrent => "begin-concurrent",
        }
    }

    /// Whether MVCC is active.
    #[must_use]
    pub const fn mvcc(&self) -> bool {
        self.mvcc.0
    }

    #[must_use]
    pub const fn passive_checkpoint(&self) -> bool {
        self.passive_checkpoint.0
    }

    /// Whether multi-process WAL ownership is active.
    #[must_use]
    pub const fn multiprocess_wal(&self) -> bool {
        self.multiprocess_wal.0
    }

    /// Whether FTS support is active.
    #[must_use]
    pub const fn fts(&self) -> bool {
        self.fts.0
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TursoMvccBatchWriteReceipt {
    pub schema_id: String,
    pub attempted_rows: usize,
    pub committed_rows: usize,
    pub retry_count: usize,
    pub busy_count: usize,
    pub snapshot_conflict_count: usize,
    pub retry_delay_ms: u64,
    pub optimization: TursoMvccOptimizationReceipt,
}

pub(crate) struct TursoMvccStoreInner {
    pub(crate) path: PathBuf,
    pub(crate) passive_checkpoint: bool,
    _database: Arc<turso::Database>,
    pub(crate) lanes: Vec<Arc<tokio::sync::Mutex<turso::Connection>>>,
    pub(crate) retry_attempts: usize,
    pub(crate) max_batch_rows: usize,
}

/// A bounded, four-lane Turso 0.7 store for append-heavy single-process work.
///
/// Each lane owns one long-lived connection and its prepared-statement cache.
/// The lane mutex is the backpressure boundary: at most one transaction runs on
/// a connection, while independent lanes can commit with `BEGIN CONCURRENT`.
#[derive(Clone)]
pub struct TursoMvccStore {
    pub(crate) inner: Arc<TursoMvccStoreInner>,
}

impl TursoMvccStore {
    pub async fn open(config: TursoMvccStoreConfig) -> Result<Self, String> {
        validate_config(&config)?;
        ensure_parent_directory(&config.path)?;

        let database = Arc::new(
            turso::Builder::new_local(config.path.to_string_lossy().as_ref())
                .experimental_index_method(false)
                .experimental_multiprocess_wal(false)
                .experimental_mvcc_passive_checkpoint(config.passive_checkpoint)
                .build()
                .await
                .map_err(|error| format!("failed to open Turso MVCC store: {error}"))?,
        );
        let mut lanes = Vec::with_capacity(config.connection_lanes);
        for lane_index in 0..config.connection_lanes {
            let connection = database
                .connect()
                .map_err(|error| format!("failed to connect Turso MVCC lane: {error}"))?;
            connection
                .busy_timeout(Duration::from_millis(config.busy_timeout_ms))
                .map_err(|error| format!("failed to configure Turso MVCC busy timeout: {error}"))?;
            enable_and_verify_mvcc(&connection).await?;
            if lane_index == 0 {
                connection
                    .execute_batch(CREATE_EVENT_TABLES_SQL)
                    .await
                    .map_err(|error| format!("failed to bootstrap Turso MVCC schema: {error}"))?;
                connection
                    .execute_batch(crate::turso_mvcc_partition_sql::CREATE_PARTITION_TABLES_SQL)
                    .await
                    .map_err(|error| {
                        format!("failed to bootstrap Turso MVCC partition schema: {error}")
                    })?;
            }
            lanes.push(Arc::new(tokio::sync::Mutex::new(connection)));
        }

        Ok(Self {
            inner: Arc::new(TursoMvccStoreInner {
                path: config.path.clone(),
                passive_checkpoint: config.passive_checkpoint,
                _database: database,
                lanes,
                retry_attempts: config.retry_attempts,
                max_batch_rows: config.max_batch_rows,
            }),
        })
    }

    pub fn optimization_receipt(&self) -> TursoMvccOptimizationReceipt {
        TursoMvccOptimizationReceipt {
            insert_rows_per_statement: TursoMvccInsertRowsPerStatement(INSERT_ROWS_PER_STATEMENT),
            schema_id: TursoMvccOptimizationReceiptSchemaId::V1,
            profile: if self.inner.passive_checkpoint {
                TursoMvccOptimizationProfile::ConcurrentPassiveCheckpoint
            } else {
                TursoMvccOptimizationProfile::Concurrent
            },
            connection_lanes: TursoMvccConnectionLaneCount(self.inner.lanes.len()),
            partition_shards: TursoMvccPartitionShardCount(EVENT_SHARDS),
            statement_cache: TursoMvccStatementCacheMode::PreparedPerConnection,
            transaction_mode: TursoMvccTransactionMode::Concurrent,
            mvcc: TursoMvccFeatureState(true),
            passive_checkpoint: TursoMvccFeatureState(self.inner.passive_checkpoint),
            multiprocess_wal: TursoMvccFeatureState(false),
            fts: TursoMvccFeatureState(false),
        }
    }

    pub async fn append_batch(
        &self,
        events: &[TursoMvccEvent],
    ) -> Result<TursoMvccBatchWriteReceipt, String> {
        if events.is_empty() {
            return Ok(TursoMvccBatchWriteReceipt {
                schema_id: BATCH_WRITE_RECEIPT_SCHEMA_ID.to_string(),
                attempted_rows: 0,
                committed_rows: 0,
                retry_count: 0,
                busy_count: 0,
                snapshot_conflict_count: 0,
                retry_delay_ms: 0,
                optimization: self.optimization_receipt(),
            });
        }
        if events.len() > self.inner.max_batch_rows {
            return Err(format!(
                "Turso MVCC batch has {} rows; maximum is {}",
                events.len(),
                self.inner.max_batch_rows
            ));
        }
        validate_events(events)?;

        let shard = event_shard(&events[0].partition_key);
        let lane_index = shard % self.inner.lanes.len();
        let lane = Arc::clone(&self.inner.lanes[lane_index]);
        let connection = lane.lock_owned().await;

        let mut last_retryable_error = None;
        for attempt in 0..self.inner.retry_attempts {
            match append_batch_once(&connection, shard, events).await {
                Ok(()) => {
                    return Ok(TursoMvccBatchWriteReceipt {
                        schema_id: BATCH_WRITE_RECEIPT_SCHEMA_ID.to_string(),
                        attempted_rows: events.len(),
                        committed_rows: events.len(),
                        retry_count: attempt,
                        busy_count: 0,
                        snapshot_conflict_count: 0,
                        retry_delay_ms: 0,
                        optimization: self.optimization_receipt(),
                    });
                }
                Err(error) if is_retryable_mvcc_error(&error) => {
                    last_retryable_error = Some(error);
                    tokio::time::sleep(retry_delay(attempt)).await;
                }
                Err(error) => return Err(error),
            }
        }
        Err(format!(
            "{} after {} Turso MVCC transaction attempts",
            last_retryable_error
                .unwrap_or_else(|| "Turso MVCC transaction conflict persisted".to_string()),
            self.inner.retry_attempts
        ))
    }

    pub async fn read_partition(&self, partition_key: &str) -> Result<Vec<TursoMvccEvent>, String> {
        let shard = event_shard(partition_key);
        let lane_index = shard % self.inner.lanes.len();
        let lane = Arc::clone(&self.inner.lanes[lane_index]);
        let connection = lane.lock_owned().await;
        let mut statement = connection
            .prepare_cached(READ_PARTITION_SQL[shard])
            .await
            .map_err(|error| format!("failed to prepare Turso MVCC partition read: {error}"))?;
        let mut rows = statement
            .query((partition_key,))
            .await
            .map_err(|error| format!("failed to query Turso MVCC partition: {error}"))?;
        let mut events = Vec::new();
        while let Some(row) = rows
            .next()
            .await
            .map_err(|error| format!("failed to step Turso MVCC partition rows: {error}"))?
        {
            events.push(TursoMvccEvent {
                partition_key: row
                    .get(0)
                    .map_err(|error| format!("failed to decode partition key: {error}"))?,
                event_id: row
                    .get(1)
                    .map_err(|error| format!("failed to decode event id: {error}"))?,
                payload: row
                    .get(2)
                    .map_err(|error| format!("failed to decode event payload: {error}"))?,
                created_at_ms: row
                    .get(3)
                    .map_err(|error| format!("failed to decode event timestamp: {error}"))?,
            });
        }
        Ok(events)
    }
}

async fn enable_and_verify_mvcc(connection: &turso::Connection) -> Result<(), String> {
    let mut rows = connection
        .query("PRAGMA journal_mode = 'mvcc'", ())
        .await
        .map_err(|error| format!("failed to enable Turso MVCC journal mode: {error}"))?;
    let row = rows
        .next()
        .await
        .map_err(|error| format!("failed to read Turso MVCC journal mode: {error}"))?
        .ok_or_else(|| "Turso MVCC journal mode returned no row".to_string())?;
    let journal_mode: String = row
        .get(0)
        .map_err(|error| format!("failed to decode Turso MVCC journal mode: {error}"))?;
    if journal_mode != "mvcc" {
        return Err(format!(
            "Turso MVCC store requires journal_mode=mvcc, observed {journal_mode}"
        ));
    }
    Ok(())
}

const INSERT_ROWS_PER_STATEMENT: usize = 32;

fn insert_event_batch_sql(shard: usize, rows: usize) -> String {
    debug_assert!(shard < EVENT_SHARDS);
    debug_assert!(rows > 1 && rows <= INSERT_ROWS_PER_STATEMENT);
    let values = (0..rows)
        .map(|_| "(?, ?, ?, ?)")
        .collect::<Vec<_>>()
        .join(", ");
    format!(
        "INSERT INTO asp_mvcc_event_{shard} \
         (partition_key, event_id, payload, created_at_ms) VALUES {values}"
    )
}

async fn append_batch_once(
    connection: &turso::Connection,
    shard: usize,
    events: &[TursoMvccEvent],
) -> Result<(), String> {
    connection
        .execute("BEGIN CONCURRENT", ())
        .await
        .map_err(|error| format!("failed to begin Turso MVCC concurrent batch: {error}"))?;

    let result = async {
        for event_chunk in events.chunks(INSERT_ROWS_PER_STATEMENT) {
            let batch_sql =
                (event_chunk.len() > 1).then(|| insert_event_batch_sql(shard, event_chunk.len()));
            let insert_sql = batch_sql.as_deref().unwrap_or(INSERT_EVENT_SQL[shard]);
            let mut statement = connection
                .prepare_cached(insert_sql)
                .await
                .map_err(|error| format!("failed to prepare Turso MVCC event insert: {error}"))?;
            let mut parameters = Vec::with_capacity(event_chunk.len() * 4);
            for event in event_chunk {
                parameters.push(turso::Value::Text(event.partition_key.clone()));
                parameters.push(turso::Value::Text(event.event_id.clone()));
                parameters.push(turso::Value::Blob(event.payload.clone()));
                parameters.push(turso::Value::Integer(event.created_at_ms));
            }
            statement
                .execute(parameters)
                .await
                .map_err(|error| format!("failed to append Turso MVCC event: {error}"))?;
            drop(statement);
        }
        connection
            .execute("COMMIT", ())
            .await
            .map_err(|error| format!("failed to commit Turso MVCC concurrent batch: {error}"))?;
        Ok(())
    }
    .await;

    if result.is_err() {
        let _ = connection.execute("ROLLBACK", ()).await;
    }
    result
}

fn validate_config(config: &TursoMvccStoreConfig) -> Result<(), String> {
    if config.connection_lanes == 0 || config.connection_lanes > MAX_CONNECTION_LANES {
        return Err(format!(
            "Turso MVCC connection_lanes must be between 1 and {MAX_CONNECTION_LANES}"
        ));
    }
    if config.retry_attempts == 0 {
        return Err("Turso MVCC retry_attempts must be greater than zero".to_string());
    }
    if config.max_batch_rows == 0 {
        return Err("Turso MVCC max_batch_rows must be greater than zero".to_string());
    }
    Ok(())
}

fn validate_events(events: &[TursoMvccEvent]) -> Result<(), String> {
    let partition_key = events
        .first()
        .map(|event| event.partition_key.as_str())
        .unwrap_or_default();
    for event in events {
        if event.partition_key.is_empty() {
            return Err("Turso MVCC event partition_key must not be empty".to_string());
        }
        if event.event_id.is_empty() {
            return Err("Turso MVCC event event_id must not be empty".to_string());
        }
        if event.partition_key != partition_key {
            return Err("Turso MVCC atomic batches must contain exactly one partition".to_string());
        }
    }
    Ok(())
}

pub(crate) fn event_shard(partition_key: &str) -> usize {
    let mut hash = 0xcbf2_9ce4_8422_2325_u64;
    for byte in partition_key.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    (hash as usize) % EVENT_SHARDS
}

fn ensure_parent_directory(path: &Path) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|error| {
            format!(
                "failed to create Turso MVCC store directory {}: {error}",
                parent.display()
            )
        })?;
    }
    Ok(())
}

fn is_retryable_mvcc_error(error: &str) -> bool {
    let error = error.to_ascii_lowercase();
    error.contains("busy")
        || error.contains("locked")
        || error.contains("conflict")
        || error.contains("snapshot")
}

fn retry_delay(attempt: usize) -> Duration {
    let shift = attempt.min(6) as u32;
    Duration::from_millis((1_u64 << shift).min(64))
}
