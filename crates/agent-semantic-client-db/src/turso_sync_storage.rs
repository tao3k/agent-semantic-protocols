use std::fmt;
use std::future::Future;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use agent_semantic_content_identity::hash_blob;
use serde::{Deserialize, Serialize};

pub const TURSO_SYNC_OPERATION_RECEIPT_SCHEMA_ID: &str =
    "agent.semantic-protocols.client-db.turso-sync-operation-receipt.v1";
pub const DEFAULT_TURSO_SYNC_OPERATION_TIMEOUT: Duration = Duration::from_secs(5);

fn default_turso_sync_operation_timeout() -> Duration {
    DEFAULT_TURSO_SYNC_OPERATION_TIMEOUT
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TursoSyncProfileConfig {
    pub path: PathBuf,
    pub mode: TursoSyncProfileMode,
    #[serde(default = "default_turso_sync_operation_timeout")]
    pub operation_timeout: Duration,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", tag = "kind")]
pub enum TursoSyncProfileMode {
    Local,
    Remote {
        remote_url: String,
        auth_token: String,
        bootstrap_if_empty: bool,
    },
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum TursoSyncOperation {
    Push,
    Pull,
    Checkpoint,
    Stats,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum TursoSyncOperationOutcome {
    Applied,
    NoRemoteChanges,
    Observed,
    Failed,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TursoSyncStatsReceipt {
    pub cdc_operations: u64,
    pub main_wal_size: u64,
    pub revert_wal_size: u64,
    pub network_received_bytes: u64,
    pub network_sent_bytes: u64,
    pub last_pull_unix_time: Option<i64>,
    pub last_push_unix_time: Option<i64>,
    pub revision: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TursoSyncOperationReceipt {
    schema_id: String,
    operation: TursoSyncOperation,
    outcome: TursoSyncOperationOutcome,
    elapsed_ms: u64,
    pulled_changes: Option<bool>,
    stats: Option<TursoSyncStatsReceipt>,
    error_kind: Option<String>,
    error_digest: Option<String>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TursoSyncStorageErrorCode {
    InvalidConfiguration,
    Open,
    Connect,
    Timeout,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TursoSyncStorageError {
    pub code: TursoSyncStorageErrorCode,
    pub message: String,
}

impl fmt::Display for TursoSyncStorageError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{:?}: {}", self.code, self.message)
    }
}

impl std::error::Error for TursoSyncStorageError {}

pub struct TursoSyncStorage {
    backend: TursoSyncBackend,
    operation_timeout: Duration,
}

enum TursoSyncBackend {
    Local(turso::Database),
    Remote(turso::sync::Database),
}

enum TursoSyncWaitError<E> {
    Operation(E),
    Timeout,
}

async fn await_turso_operation<T, E, F>(
    operation_timeout: Duration,
    future: F,
) -> Result<T, TursoSyncWaitError<E>>
where
    F: Future<Output = Result<T, E>>,
{
    match tokio::time::timeout(operation_timeout, future).await {
        Ok(Ok(value)) => Ok(value),
        Ok(Err(error)) => Err(TursoSyncWaitError::Operation(error)),
        Err(_) => Err(TursoSyncWaitError::Timeout),
    }
}

macro_rules! stats_receipt {
    ($stats:expr) => {{
        let stats = $stats;
        TursoSyncStatsReceipt {
            cdc_operations: stats.cdc_operations as u64,
            main_wal_size: stats.main_wal_size as u64,
            revert_wal_size: stats.revert_wal_size as u64,
            network_received_bytes: stats.network_received_bytes as u64,
            network_sent_bytes: stats.network_sent_bytes as u64,
            last_pull_unix_time: stats.last_pull_unix_time,
            last_push_unix_time: stats.last_push_unix_time,
            revision: stats.revision,
        }
    }};
}

impl TursoSyncStorage {
    pub async fn open(config: TursoSyncProfileConfig) -> Result<Self, TursoSyncStorageError> {
        if config.operation_timeout.is_zero() {
            return Err(TursoSyncStorageError {
                code: TursoSyncStorageErrorCode::InvalidConfiguration,
                message: "operation timeout must be greater than zero".to_owned(),
            });
        }
        let path = config.path.to_string_lossy();
        let operation_timeout = config.operation_timeout;
        let backend = match config.mode {
            TursoSyncProfileMode::Local => {
                let database = await_turso_operation(
                    operation_timeout,
                    turso::Builder::new_local(path.as_ref()).build(),
                )
                .await
                .map_err(|error| match error {
                    TursoSyncWaitError::Operation(error) => TursoSyncStorageError {
                        code: TursoSyncStorageErrorCode::Open,
                        message: error.to_string(),
                    },
                    TursoSyncWaitError::Timeout => timeout_storage_error("open"),
                })?;
                TursoSyncBackend::Local(database)
            }
            TursoSyncProfileMode::Remote {
                remote_url,
                auth_token,
                bootstrap_if_empty,
            } => {
                if remote_url.trim().is_empty() || auth_token.trim().is_empty() {
                    return Err(TursoSyncStorageError {
                        code: TursoSyncStorageErrorCode::InvalidConfiguration,
                        message: "remote URL and auth token must be non-empty".to_owned(),
                    });
                }
                let database = await_turso_operation(
                    operation_timeout,
                    turso::sync::Builder::new_remote(path.as_ref())
                        .with_remote_url(&remote_url)
                        .with_auth_token(&auth_token)
                        .bootstrap_if_empty(bootstrap_if_empty)
                        .build(),
                )
                .await
                .map_err(|error| match error {
                    TursoSyncWaitError::Operation(error) => TursoSyncStorageError {
                        code: TursoSyncStorageErrorCode::Open,
                        message: error.to_string(),
                    },
                    TursoSyncWaitError::Timeout => timeout_storage_error("open"),
                })?;
                TursoSyncBackend::Remote(database)
            }
        };
        Ok(Self {
            backend,
            operation_timeout,
        })
    }

    pub async fn connect(&self) -> Result<turso::Connection, TursoSyncStorageError> {
        match &self.backend {
            TursoSyncBackend::Local(database) => {
                database.connect().map_err(|error| TursoSyncStorageError {
                    code: TursoSyncStorageErrorCode::Connect,
                    message: error.to_string(),
                })
            }
            TursoSyncBackend::Remote(database) => {
                await_turso_operation(self.operation_timeout, database.connect())
                    .await
                    .map_err(|error| match error {
                        TursoSyncWaitError::Operation(error) => TursoSyncStorageError {
                            code: TursoSyncStorageErrorCode::Connect,
                            message: error.to_string(),
                        },
                        TursoSyncWaitError::Timeout => timeout_storage_error("connect"),
                    })
            }
        }
    }

    pub async fn push(&self) -> TursoSyncOperationReceipt {
        let started = Instant::now();
        let TursoSyncBackend::Remote(database) = &self.backend else {
            return local_mode_failure_receipt(TursoSyncOperation::Push, started);
        };
        match await_turso_operation(self.operation_timeout, database.push()).await {
            Ok(()) => {
                self.success_receipt(TursoSyncOperation::Push, started, None, None)
                    .await
            }
            Err(TursoSyncWaitError::Operation(error)) => {
                failure_receipt(TursoSyncOperation::Push, started, &error)
            }
            Err(TursoSyncWaitError::Timeout) => {
                timeout_failure_receipt(TursoSyncOperation::Push, started)
            }
        }
    }

    pub async fn pull(&self) -> TursoSyncOperationReceipt {
        let started = Instant::now();
        let TursoSyncBackend::Remote(database) = &self.backend else {
            return local_mode_failure_receipt(TursoSyncOperation::Pull, started);
        };
        match await_turso_operation(self.operation_timeout, database.pull()).await {
            Ok(changed) => {
                let outcome = if changed {
                    TursoSyncOperationOutcome::Applied
                } else {
                    TursoSyncOperationOutcome::NoRemoteChanges
                };
                self.success_receipt(
                    TursoSyncOperation::Pull,
                    started,
                    Some(changed),
                    Some(outcome),
                )
                .await
            }
            Err(TursoSyncWaitError::Operation(error)) => {
                failure_receipt(TursoSyncOperation::Pull, started, &error)
            }
            Err(TursoSyncWaitError::Timeout) => {
                timeout_failure_receipt(TursoSyncOperation::Pull, started)
            }
        }
    }

    pub async fn checkpoint(&self) -> TursoSyncOperationReceipt {
        let started = Instant::now();
        let checkpoint = match &self.backend {
            TursoSyncBackend::Local(database) => {
                await_turso_operation(self.operation_timeout, async {
                    let connection = database.connect()?;
                    connection.cacheflush()?;
                    Ok::<(), turso::Error>(())
                })
                .await
            }
            TursoSyncBackend::Remote(database) => {
                await_turso_operation(self.operation_timeout, database.checkpoint()).await
            }
        };
        match checkpoint {
            Ok(()) => {
                self.success_receipt(TursoSyncOperation::Checkpoint, started, None, None)
                    .await
            }
            Err(TursoSyncWaitError::Operation(error)) => {
                failure_receipt(TursoSyncOperation::Checkpoint, started, &error)
            }
            Err(TursoSyncWaitError::Timeout) => {
                timeout_failure_receipt(TursoSyncOperation::Checkpoint, started)
            }
        }
    }

    pub async fn stats(&self) -> TursoSyncOperationReceipt {
        let started = Instant::now();
        let TursoSyncBackend::Remote(database) = &self.backend else {
            return TursoSyncOperationReceipt {
                schema_id: TURSO_SYNC_OPERATION_RECEIPT_SCHEMA_ID.to_owned(),
                operation: TursoSyncOperation::Stats,
                outcome: TursoSyncOperationOutcome::Observed,
                elapsed_ms: started.elapsed().as_millis() as u64,
                pulled_changes: None,
                stats: None,
                error_kind: None,
                error_digest: None,
            };
        };
        match await_turso_operation(self.operation_timeout, database.stats()).await {
            Ok(stats) => TursoSyncOperationReceipt {
                schema_id: TURSO_SYNC_OPERATION_RECEIPT_SCHEMA_ID.to_owned(),
                operation: TursoSyncOperation::Stats,
                outcome: TursoSyncOperationOutcome::Observed,
                elapsed_ms: started.elapsed().as_millis() as u64,
                pulled_changes: None,
                stats: Some(stats_receipt!(stats)),
                error_kind: None,
                error_digest: None,
            },
            Err(TursoSyncWaitError::Operation(error)) => {
                failure_receipt(TursoSyncOperation::Stats, started, &error)
            }
            Err(TursoSyncWaitError::Timeout) => {
                timeout_failure_receipt(TursoSyncOperation::Stats, started)
            }
        }
    }

    async fn success_receipt(
        &self,
        operation: TursoSyncOperation,
        started: Instant,
        pulled_changes: Option<bool>,
        outcome: Option<TursoSyncOperationOutcome>,
    ) -> TursoSyncOperationReceipt {
        let stats = match &self.backend {
            TursoSyncBackend::Local(_) => None,
            TursoSyncBackend::Remote(database) => {
                await_turso_operation(self.operation_timeout, database.stats())
                    .await
                    .ok()
                    .map(|stats| stats_receipt!(stats))
            }
        };
        TursoSyncOperationReceipt {
            schema_id: TURSO_SYNC_OPERATION_RECEIPT_SCHEMA_ID.to_owned(),
            operation,
            outcome: outcome.unwrap_or(TursoSyncOperationOutcome::Applied),
            elapsed_ms: started.elapsed().as_millis() as u64,
            pulled_changes,
            stats,
            error_kind: None,
            error_digest: None,
        }
    }
}

fn timeout_storage_error(operation: &str) -> TursoSyncStorageError {
    TursoSyncStorageError {
        code: TursoSyncStorageErrorCode::Timeout,
        message: format!("turso sync {operation} timed out"),
    }
}

#[derive(Debug)]
struct TursoLocalModeUnsupportedOperation {
    operation: TursoSyncOperation,
}

impl fmt::Display for TursoLocalModeUnsupportedOperation {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "Turso local mode does not support remote sync operation {:?}",
            self.operation
        )
    }
}

fn local_mode_failure_receipt(
    operation: TursoSyncOperation,
    started: Instant,
) -> TursoSyncOperationReceipt {
    failure_receipt(
        operation,
        started,
        &TursoLocalModeUnsupportedOperation { operation },
    )
}

fn timeout_failure_receipt(
    operation: TursoSyncOperation,
    started: Instant,
) -> TursoSyncOperationReceipt {
    const ERROR_KIND: &str = "timeout";
    const ERROR_MESSAGE: &str = "turso sync operation timed out";

    TursoSyncOperationReceipt {
        schema_id: TURSO_SYNC_OPERATION_RECEIPT_SCHEMA_ID.to_owned(),
        operation,
        outcome: TursoSyncOperationOutcome::Failed,
        elapsed_ms: started.elapsed().as_millis() as u64,
        pulled_changes: None,
        stats: None,
        error_kind: Some(ERROR_KIND.to_owned()),
        error_digest: Some(hash_blob(ERROR_MESSAGE.as_bytes()).to_string()),
    }
}

fn failure_receipt(
    operation: TursoSyncOperation,
    started: Instant,
    error: &impl fmt::Display,
) -> TursoSyncOperationReceipt {
    let message = error.to_string();
    TursoSyncOperationReceipt {
        schema_id: TURSO_SYNC_OPERATION_RECEIPT_SCHEMA_ID.to_owned(),
        operation,
        outcome: TursoSyncOperationOutcome::Failed,
        elapsed_ms: started.elapsed().as_millis() as u64,
        pulled_changes: None,
        stats: None,
        error_kind: Some(std::any::type_name_of_val(error).to_owned()),
        error_digest: Some(hash_blob(message.as_bytes()).to_string()),
    }
}
