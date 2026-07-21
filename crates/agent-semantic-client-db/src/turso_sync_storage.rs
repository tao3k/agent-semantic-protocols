use std::fmt;
use std::path::PathBuf;
use std::time::Instant;

use agent_semantic_content_identity::hash_blob;
use serde::{Deserialize, Serialize};

pub const TURSO_SYNC_OPERATION_RECEIPT_SCHEMA_ID: &str =
    "agent.semantic-protocols.client-db.turso-sync-operation-receipt.v1";

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TursoSyncProfileConfig {
    pub path: PathBuf,
    pub remote_url: String,
    pub auth_token: String,
    pub bootstrap_if_empty: bool,
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
    pub schema_id: String,
    pub operation: TursoSyncOperation,
    pub outcome: TursoSyncOperationOutcome,
    pub elapsed_ms: u64,
    pub pulled_changes: Option<bool>,
    pub stats: Option<TursoSyncStatsReceipt>,
    pub error_kind: Option<String>,
    pub error_digest: Option<String>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TursoSyncStorageErrorCode {
    InvalidConfiguration,
    Open,
    Connect,
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
    database: turso::sync::Database,
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
        if config.remote_url.trim().is_empty() || config.auth_token.trim().is_empty() {
            return Err(TursoSyncStorageError {
                code: TursoSyncStorageErrorCode::InvalidConfiguration,
                message: "remote URL and auth token must be non-empty".to_owned(),
            });
        }
        let path = config.path.to_string_lossy();
        let database = turso::sync::Builder::new_remote(path.as_ref())
            .with_remote_url(&config.remote_url)
            .with_auth_token(&config.auth_token)
            .bootstrap_if_empty(config.bootstrap_if_empty)
            .build()
            .await
            .map_err(|error| TursoSyncStorageError {
                code: TursoSyncStorageErrorCode::Open,
                message: error.to_string(),
            })?;
        Ok(Self { database })
    }

    pub async fn connect(&self) -> Result<turso::Connection, TursoSyncStorageError> {
        self.database
            .connect()
            .await
            .map_err(|error| TursoSyncStorageError {
                code: TursoSyncStorageErrorCode::Connect,
                message: error.to_string(),
            })
    }

    pub async fn push(&self) -> TursoSyncOperationReceipt {
        let started = Instant::now();
        match self.database.push().await {
            Ok(()) => {
                self.success_receipt(TursoSyncOperation::Push, started, None, None)
                    .await
            }
            Err(error) => failure_receipt(TursoSyncOperation::Push, started, &error),
        }
    }

    pub async fn pull(&self) -> TursoSyncOperationReceipt {
        let started = Instant::now();
        match self.database.pull().await {
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
            Err(error) => failure_receipt(TursoSyncOperation::Pull, started, &error),
        }
    }

    pub async fn checkpoint(&self) -> TursoSyncOperationReceipt {
        let started = Instant::now();
        match self.database.checkpoint().await {
            Ok(()) => {
                self.success_receipt(TursoSyncOperation::Checkpoint, started, None, None)
                    .await
            }
            Err(error) => failure_receipt(TursoSyncOperation::Checkpoint, started, &error),
        }
    }

    pub async fn stats(&self) -> TursoSyncOperationReceipt {
        let started = Instant::now();
        match self.database.stats().await {
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
            Err(error) => failure_receipt(TursoSyncOperation::Stats, started, &error),
        }
    }

    async fn success_receipt(
        &self,
        operation: TursoSyncOperation,
        started: Instant,
        pulled_changes: Option<bool>,
        outcome: Option<TursoSyncOperationOutcome>,
    ) -> TursoSyncOperationReceipt {
        let stats = self
            .database
            .stats()
            .await
            .ok()
            .map(|stats| stats_receipt!(stats));
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
