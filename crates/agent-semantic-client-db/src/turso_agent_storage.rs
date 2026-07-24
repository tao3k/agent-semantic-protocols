//! Backend-neutral `AgentStorage` adapter for the append-heavy Turso MVCC authority.

use crate::storage_contract::{
    AgentStorage, SESSION_EVENT_BATCH_RECEIPT_SCHEMA_ID, SessionEvent, SessionEventBatch,
    SessionEventBatchWriteReceipt, SessionEventCursor, SessionEventPage, SessionEventPageRequest,
    StorageAuthorityKind, StorageError, StorageErrorCode, StorageFuture,
    StorageOptimizationProfile, StorageTransactionMode, StorageTransactionState,
};
use crate::turso_mvcc_store::{TursoMvccEvent, TursoMvccStore, TursoMvccStoreConfig};

#[derive(Clone)]
pub struct TursoMvccAgentStorage {
    store: TursoMvccStore,
}

impl TursoMvccAgentStorage {
    pub async fn open(config: TursoMvccStoreConfig) -> Result<Self, StorageError> {
        let store = TursoMvccStore::open(config)
            .await
            .map_err(classify_turso_storage_error)?;
        Ok(Self { store })
    }

    pub fn store(&self) -> &TursoMvccStore {
        &self.store
    }

    fn validate_profile(&self, batch: &SessionEventBatch) -> Result<(), StorageError> {
        let passive_checkpoint = self.store.optimization_receipt().passive_checkpoint;
        match (
            batch.optimization_profile,
            batch.transaction_mode,
            passive_checkpoint,
        ) {
            (
                StorageOptimizationProfile::MvccConcurrent,
                StorageTransactionMode::Concurrent,
                false,
            )
            | (
                StorageOptimizationProfile::MvccConcurrentPassiveCheckpoint,
                StorageTransactionMode::Concurrent,
                true,
            ) => Ok(()),
            _ => Err(StorageError::unsupported_profile(
                "session batch profile does not match the opened Turso MVCC authority",
            )),
        }
    }

    fn decode_event(row: TursoMvccEvent) -> Result<SessionEvent, StorageError> {
        let event: SessionEvent = serde_json::from_slice(&row.payload).map_err(|error| {
            StorageError::backend(format!(
                "decode Turso MVCC session event {}: {error}",
                row.event_id
            ))
        })?;
        if event.event_id != row.event_id || event.created_at_ms != row.created_at_ms {
            return Err(StorageError::backend(format!(
                "Turso MVCC session event envelope mismatch: {}",
                row.event_id
            )));
        }
        Ok(event)
    }
}

impl AgentStorage for TursoMvccAgentStorage {
    fn append_session_events_atomically<'a>(
        &'a self,
        batch: &'a SessionEventBatch,
    ) -> StorageFuture<'a, SessionEventBatchWriteReceipt> {
        Box::pin(async move {
            batch.validate()?;
            self.validate_profile(batch)?;
            let partition_key = batch.partition.canonical_key();
            let mut rows = Vec::with_capacity(batch.events.len());
            for event in &batch.events {
                rows.push(TursoMvccEvent {
                    partition_key: partition_key.clone(),
                    event_id: event.event_id.clone(),
                    payload: serde_json::to_vec(event).map_err(|error| {
                        StorageError::backend(format!(
                            "encode Turso MVCC session event {}: {error}",
                            event.event_id
                        ))
                    })?,
                    created_at_ms: event.created_at_ms,
                });
            }
            let write = self
                .store
                .append_batch_typed(&rows, &batch.retry_policy)
                .await
                .map_err(classify_typed_turso_storage_error)?;
            Ok(SessionEventBatchWriteReceipt {
                schema_id: SESSION_EVENT_BATCH_RECEIPT_SCHEMA_ID.to_string(),
                batch_id: batch.batch_id.clone(),
                partition: batch.partition.clone(),
                authority: StorageAuthorityKind::Local,
                backend: "turso".to_string(),
                backend_version: "0.7.0".to_string(),
                optimization_profile: batch.optimization_profile,
                transaction_mode: batch.transaction_mode,
                transaction_state: StorageTransactionState::Committed,
                attempted_rows: write.attempted_rows,
                committed_rows: write.committed_rows,
                retry_count: write.retry_count.try_into().unwrap_or(u32::MAX),
                busy_count: write.busy_count as u32,
                snapshot_conflict_count: write.snapshot_conflict_count as u32,
                retry_delay_ms: write.retry_delay_ms,
                conflict_count: (write.busy_count + write.snapshot_conflict_count) as u32,
                execution_digest: batch.execution_digest()?,
            })
        })
    }

    fn list_session_events<'a>(
        &'a self,
        request: &'a SessionEventPageRequest,
    ) -> StorageFuture<'a, SessionEventPage> {
        Box::pin(async move {
            request.validate()?;
            let rows = self
                .store
                .read_partition_page(
                    request.partition.canonical_key().into(),
                    request.after.as_ref().map(|cursor| {
                        crate::turso_mvcc_keyset::TursoMvccPageCursor {
                            created_at_ms: cursor.created_at_ms,
                            event_id: cursor.event_id.as_str().into(),
                        }
                    }),
                    request.limit.into(),
                )
                .await
                .map_err(|error| classify_turso_storage_error(error.to_string()))?;
            let after = request
                .after
                .as_ref()
                .map(|cursor| (cursor.created_at_ms, cursor.event_id.as_str()));
            let mut events = rows
                .into_iter()
                .map(Self::decode_event)
                .collect::<Result<Vec<_>, _>>()?;
            events.sort_by(|left, right| {
                (left.created_at_ms, left.event_id.as_str())
                    .cmp(&(right.created_at_ms, right.event_id.as_str()))
            });
            let mut selected = events
                .into_iter()
                .filter(|event| {
                    after.as_ref().is_none_or(|cursor| {
                        (event.created_at_ms, event.event_id.as_str()) > *cursor
                    })
                })
                .take(request.limit + 1)
                .collect::<Vec<_>>();
            let has_more = selected.len() > request.limit;
            if has_more {
                selected.truncate(request.limit);
            }
            let next = has_more.then(|| {
                let last = selected.last().expect("non-empty Turso keyset page");
                SessionEventCursor {
                    created_at_ms: last.created_at_ms,
                    event_id: last.event_id.clone(),
                }
            });
            Ok(SessionEventPage {
                items: selected,
                next,
            })
        })
    }
}

fn classify_typed_turso_storage_error(
    error: crate::turso_mvcc_typed::TursoMvccWriteError,
) -> StorageError {
    let code = match error.code {
        crate::turso_mvcc_typed::TursoMvccWriteErrorCode::InvalidRequest => {
            StorageErrorCode::InvalidRequest
        }
        crate::turso_mvcc_typed::TursoMvccWriteErrorCode::Busy => StorageErrorCode::Busy,
        crate::turso_mvcc_typed::TursoMvccWriteErrorCode::BusySnapshot => {
            StorageErrorCode::SnapshotConflict
        }
        crate::turso_mvcc_typed::TursoMvccWriteErrorCode::DuplicateIdentity => {
            StorageErrorCode::DuplicateIdentity
        }
        crate::turso_mvcc_typed::TursoMvccWriteErrorCode::Io => StorageErrorCode::Io,
        crate::turso_mvcc_typed::TursoMvccWriteErrorCode::Backend => StorageErrorCode::Backend,
    };
    StorageError {
        code,
        retryable: error.retryable,
        message: error.message,
    }
}

fn classify_turso_storage_error(message: String) -> StorageError {
    let normalized = message.to_ascii_lowercase();
    let (code, retryable) = if normalized.contains("snapshot") {
        (StorageErrorCode::SnapshotConflict, true)
    } else if normalized.contains("schema") && normalized.contains("conflict") {
        (StorageErrorCode::SchemaConflict, true)
    } else if normalized.contains("busy") {
        (StorageErrorCode::Busy, true)
    } else if normalized.contains("locked") || normalized.contains("lock") {
        (StorageErrorCode::Locked, true)
    } else if normalized.contains("conflict") {
        (StorageErrorCode::SnapshotConflict, true)
    } else if normalized.contains("unique") || normalized.contains("primary key") {
        (StorageErrorCode::DuplicateIdentity, false)
    } else {
        (StorageErrorCode::Backend, false)
    };
    StorageError::new(code, retryable, message)
}
