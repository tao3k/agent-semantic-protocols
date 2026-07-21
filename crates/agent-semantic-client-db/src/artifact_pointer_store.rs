//! Immutable artifact attempt preservation and compare-and-set root pointers.
//!
//! Artifact identity remains owned by `agent-semantic-content-identity` and
//! `agent-semantic-artifacts`; this module only persists hashes and pointer state.

use std::path::Path;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use tokio::sync::Mutex;
use turso::transaction::TransactionBehavior;

use crate::storage_contract::{StorageError, StorageErrorCode};

pub(crate) const CREATE_SCHEMA_SQL: &str = r#"
CREATE TABLE IF NOT EXISTS asp_artifact_pointer (
    repo_id TEXT NOT NULL,
    workspace_id TEXT NOT NULL,
    scope_id TEXT NOT NULL,
    pointer_kind TEXT NOT NULL,
    pointer_name TEXT NOT NULL,
    current_root_hash TEXT NOT NULL,
    revision INTEGER NOT NULL CHECK (revision > 0),
    updated_at_ms INTEGER NOT NULL,
    PRIMARY KEY (repo_id, workspace_id, scope_id, pointer_kind, pointer_name)
);
CREATE TABLE IF NOT EXISTS asp_failed_artifact_attempt (
    attempt_id TEXT PRIMARY KEY,
    repo_id TEXT NOT NULL,
    workspace_id TEXT NOT NULL,
    scope_id TEXT NOT NULL,
    pointer_kind TEXT NOT NULL,
    pointer_name TEXT NOT NULL,
    candidate_root_hash TEXT,
    error_digest TEXT NOT NULL,
    evidence BLOB NOT NULL,
    created_at_ms INTEGER NOT NULL
);
CREATE INDEX IF NOT EXISTS asp_failed_artifact_pointer_time
    ON asp_failed_artifact_attempt (
        repo_id, workspace_id, scope_id, pointer_kind, pointer_name, created_at_ms, attempt_id
    );
"#;

const SELECT_POINTER_SQL: &str = r#"
SELECT current_root_hash, revision, updated_at_ms
FROM asp_artifact_pointer
WHERE repo_id = ?1 AND workspace_id = ?2 AND scope_id = ?3
  AND pointer_kind = ?4 AND pointer_name = ?5
"#;

const INSERT_POINTER_SQL: &str = r#"
INSERT INTO asp_artifact_pointer (
    repo_id, workspace_id, scope_id, pointer_kind, pointer_name,
    current_root_hash, revision, updated_at_ms
) VALUES (?1, ?2, ?3, ?4, ?5, ?6, 1, ?7)
"#;

const UPDATE_POINTER_CAS_SQL: &str = r#"
UPDATE asp_artifact_pointer
SET current_root_hash = ?6, revision = revision + 1, updated_at_ms = ?7
WHERE repo_id = ?1 AND workspace_id = ?2 AND scope_id = ?3
  AND pointer_kind = ?4 AND pointer_name = ?5
  AND current_root_hash = ?8 AND revision = ?9
"#;

const INSERT_FAILED_ATTEMPT_SQL: &str = r#"
INSERT INTO asp_failed_artifact_attempt (
    attempt_id, repo_id, workspace_id, scope_id, pointer_kind, pointer_name,
    candidate_root_hash, error_digest, evidence, created_at_ms
) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)
"#;

const LIST_FAILED_ATTEMPTS_SQL: &str = r#"
SELECT attempt_id, candidate_root_hash, error_digest, evidence, created_at_ms
FROM asp_failed_artifact_attempt
WHERE repo_id = ?1 AND workspace_id = ?2 AND scope_id = ?3
  AND pointer_kind = ?4 AND pointer_name = ?5
ORDER BY created_at_ms, attempt_id
LIMIT ?6
"#;

#[derive(Clone, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ClientDbArtifactPointerKey {
    pub repo_id: String,
    pub workspace_id: String,
    pub scope_id: String,
    pub pointer_kind: String,
    pub pointer_name: String,
}

impl ClientDbArtifactPointerKey {
    fn validate(&self) -> Result<(), StorageError> {
        if [
            &self.repo_id,
            &self.workspace_id,
            &self.scope_id,
            &self.pointer_kind,
            &self.pointer_name,
        ]
        .iter()
        .any(|value| value.is_empty())
        {
            return Err(storage_error(
                StorageErrorCode::InvalidRequest,
                false,
                "artifact pointer key fields must be non-empty",
            ));
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ClientDbArtifactPointer {
    pub key: ClientDbArtifactPointerKey,
    pub current_root_hash: String,
    pub revision: u64,
    pub updated_at_ms: i64,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ClientDbArtifactPointerCasRequest {
    pub key: ClientDbArtifactPointerKey,
    pub expected_root_hash: Option<String>,
    pub expected_revision: u64,
    pub new_root_hash: String,
    pub updated_at_ms: i64,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum ClientDbArtifactPointerCasOutcome {
    Applied,
    Conflict,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ClientDbArtifactPointerCasReceipt {
    pub schema_id: String,
    pub outcome: ClientDbArtifactPointerCasOutcome,
    pub expected_root_hash: Option<String>,
    pub expected_revision: u64,
    pub observed_root_hash: Option<String>,
    pub observed_revision: u64,
    pub current: Option<ClientDbArtifactPointer>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ClientDbFailedArtifact {
    pub attempt_id: String,
    pub key: ClientDbArtifactPointerKey,
    pub candidate_root_hash: Option<String>,
    pub error_digest: String,
    pub evidence: Vec<u8>,
    pub created_at_ms: i64,
}

pub struct TursoArtifactPointerStore {
    _database: turso::Database,
    connection: Mutex<turso::Connection>,
}

impl TursoArtifactPointerStore {
    pub async fn open(path: impl AsRef<Path>) -> Result<Self, StorageError> {
        let path = path.as_ref().to_str().ok_or_else(|| {
            storage_error(
                StorageErrorCode::InvalidRequest,
                false,
                "artifact pointer database path must be valid UTF-8",
            )
        })?;
        let database = turso::Builder::new_local(path)
            .experimental_multiprocess_wal(true)
            .build()
            .await
            .map_err(classify_turso_error)?;
        let connection = database.connect().map_err(classify_turso_error)?;
        connection
            .busy_timeout(Duration::from_secs(5))
            .map_err(classify_turso_error)?;
        connection
            .execute_batch(CREATE_SCHEMA_SQL)
            .await
            .map_err(classify_turso_error)?;
        Ok(Self {
            _database: database,
            connection: Mutex::new(connection),
        })
    }

    pub async fn compare_and_set(
        &self,
        request: &ClientDbArtifactPointerCasRequest,
    ) -> Result<ClientDbArtifactPointerCasReceipt, StorageError> {
        request.key.validate()?;
        if request.new_root_hash.is_empty() {
            return Err(storage_error(
                StorageErrorCode::InvalidRequest,
                false,
                "new artifact root hash must be non-empty",
            ));
        }
        let mut connection = self.connection.lock().await;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .await
            .map_err(classify_turso_error)?;
        let observed = select_pointer(&transaction, &request.key).await?;
        let matches_expectation = match &observed {
            None => request.expected_root_hash.is_none() && request.expected_revision == 0,
            Some(pointer) => {
                request.expected_root_hash.as_deref() == Some(pointer.current_root_hash.as_str())
                    && request.expected_revision == pointer.revision
            }
        };
        if !matches_expectation {
            transaction.rollback().await.map_err(classify_turso_error)?;
            return Ok(cas_receipt(
                ClientDbArtifactPointerCasOutcome::Conflict,
                request,
                observed,
            ));
        }

        match observed {
            None => {
                transaction
                    .execute(
                        INSERT_POINTER_SQL,
                        (
                            request.key.repo_id.as_str(),
                            request.key.workspace_id.as_str(),
                            request.key.scope_id.as_str(),
                            request.key.pointer_kind.as_str(),
                            request.key.pointer_name.as_str(),
                            request.new_root_hash.as_str(),
                            request.updated_at_ms,
                        ),
                    )
                    .await
                    .map_err(classify_turso_error)?;
            }
            Some(pointer) => {
                let changed = transaction
                    .execute(
                        UPDATE_POINTER_CAS_SQL,
                        (
                            request.key.repo_id.as_str(),
                            request.key.workspace_id.as_str(),
                            request.key.scope_id.as_str(),
                            request.key.pointer_kind.as_str(),
                            request.key.pointer_name.as_str(),
                            request.new_root_hash.as_str(),
                            request.updated_at_ms,
                            pointer.current_root_hash.as_str(),
                            pointer.revision as i64,
                        ),
                    )
                    .await
                    .map_err(classify_turso_error)?;
                if changed != 1 {
                    transaction.rollback().await.map_err(classify_turso_error)?;
                    let observed = select_pointer(&connection, &request.key).await?;
                    return Ok(cas_receipt(
                        ClientDbArtifactPointerCasOutcome::Conflict,
                        request,
                        observed,
                    ));
                }
            }
        }
        transaction.commit().await.map_err(classify_turso_error)?;
        let current = select_pointer(&connection, &request.key).await?;
        Ok(cas_receipt(
            ClientDbArtifactPointerCasOutcome::Applied,
            request,
            current,
        ))
    }

    pub async fn preserve_failed_artifact(
        &self,
        failed: &ClientDbFailedArtifact,
    ) -> Result<(), StorageError> {
        failed.key.validate()?;
        if failed.attempt_id.is_empty() || failed.error_digest.is_empty() {
            return Err(storage_error(
                StorageErrorCode::InvalidRequest,
                false,
                "failed artifact attempt_id and error_digest must be non-empty",
            ));
        }
        let connection = self.connection.lock().await;
        connection
            .execute(
                INSERT_FAILED_ATTEMPT_SQL,
                (
                    failed.attempt_id.as_str(),
                    failed.key.repo_id.as_str(),
                    failed.key.workspace_id.as_str(),
                    failed.key.scope_id.as_str(),
                    failed.key.pointer_kind.as_str(),
                    failed.key.pointer_name.as_str(),
                    failed.candidate_root_hash.as_deref(),
                    failed.error_digest.as_str(),
                    failed.evidence.as_slice(),
                    failed.created_at_ms,
                ),
            )
            .await
            .map(|_| ())
            .map_err(classify_turso_error)
    }

    pub async fn list_failed_artifacts(
        &self,
        key: &ClientDbArtifactPointerKey,
        limit: usize,
    ) -> Result<Vec<ClientDbFailedArtifact>, StorageError> {
        key.validate()?;
        if limit == 0 || limit > 1_000 {
            return Err(storage_error(
                StorageErrorCode::InvalidRequest,
                false,
                "failed artifact limit must be in 1..=1000",
            ));
        }
        let connection = self.connection.lock().await;
        let mut statement = connection
            .prepare_cached(LIST_FAILED_ATTEMPTS_SQL)
            .await
            .map_err(classify_turso_error)?;
        let mut rows = statement
            .query((
                key.repo_id.as_str(),
                key.workspace_id.as_str(),
                key.scope_id.as_str(),
                key.pointer_kind.as_str(),
                key.pointer_name.as_str(),
                limit as i64,
            ))
            .await
            .map_err(classify_turso_error)?;
        let mut failed = Vec::new();
        while let Some(row) = rows.next().await.map_err(classify_turso_error)? {
            failed.push(ClientDbFailedArtifact {
                attempt_id: row.get(0).map_err(classify_turso_error)?,
                key: key.clone(),
                candidate_root_hash: row.get(1).map_err(classify_turso_error)?,
                error_digest: row.get(2).map_err(classify_turso_error)?,
                evidence: row.get(3).map_err(classify_turso_error)?,
                created_at_ms: row.get(4).map_err(classify_turso_error)?,
            });
        }
        Ok(failed)
    }
}

async fn select_pointer(
    connection: &turso::Connection,
    key: &ClientDbArtifactPointerKey,
) -> Result<Option<ClientDbArtifactPointer>, StorageError> {
    let mut statement = connection
        .prepare_cached(SELECT_POINTER_SQL)
        .await
        .map_err(classify_turso_error)?;
    let mut rows = statement
        .query((
            key.repo_id.as_str(),
            key.workspace_id.as_str(),
            key.scope_id.as_str(),
            key.pointer_kind.as_str(),
            key.pointer_name.as_str(),
        ))
        .await
        .map_err(classify_turso_error)?;
    let Some(row) = rows.next().await.map_err(classify_turso_error)? else {
        return Ok(None);
    };
    let revision: i64 = row.get(1).map_err(classify_turso_error)?;
    Ok(Some(ClientDbArtifactPointer {
        key: key.clone(),
        current_root_hash: row.get(0).map_err(classify_turso_error)?,
        revision: revision.max(0) as u64,
        updated_at_ms: row.get(2).map_err(classify_turso_error)?,
    }))
}

fn cas_receipt(
    outcome: ClientDbArtifactPointerCasOutcome,
    request: &ClientDbArtifactPointerCasRequest,
    current: Option<ClientDbArtifactPointer>,
) -> ClientDbArtifactPointerCasReceipt {
    ClientDbArtifactPointerCasReceipt {
        schema_id: "asp.client-db-artifact-pointer-cas-receipt.v1".to_owned(),
        outcome,
        expected_root_hash: request.expected_root_hash.clone(),
        expected_revision: request.expected_revision,
        observed_root_hash: current
            .as_ref()
            .map(|pointer| pointer.current_root_hash.clone()),
        observed_revision: current.as_ref().map_or(0, |pointer| pointer.revision),
        current,
    }
}

fn classify_turso_error(error: turso::Error) -> StorageError {
    let code = match &error {
        turso::Error::Busy(_) => StorageErrorCode::Busy,
        turso::Error::BusySnapshot(_) => StorageErrorCode::SnapshotConflict,
        turso::Error::Constraint(_) => StorageErrorCode::DuplicateIdentity,
        turso::Error::IoError(_, _) => StorageErrorCode::Io,
        _ => StorageErrorCode::Backend,
    };
    storage_error(
        code,
        matches!(
            code,
            StorageErrorCode::Busy | StorageErrorCode::SnapshotConflict
        ),
        error.to_string(),
    )
}

fn storage_error(
    code: StorageErrorCode,
    retryable: bool,
    message: impl Into<String>,
) -> StorageError {
    StorageError {
        code,
        retryable,
        message: message.into(),
    }
}
