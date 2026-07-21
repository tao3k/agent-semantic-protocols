//! Typed MVCC conflict handling for Turso 0.7 concurrent transactions.

use std::collections::HashSet;
use std::fmt;
use std::time::Duration;

use crate::storage_contract::StorageRetryPolicy;
use crate::turso_mvcc_store::{
    BATCH_WRITE_RECEIPT_SCHEMA_ID, TursoMvccBatchWriteReceipt, TursoMvccEvent, TursoMvccStore,
    event_shard,
};

const INSERT_EVENT_SQL: [&str; 4] = [
    "INSERT INTO asp_mvcc_event_0 (partition_key, event_id, payload, created_at_ms) VALUES (?1, ?2, ?3, ?4)",
    "INSERT INTO asp_mvcc_event_1 (partition_key, event_id, payload, created_at_ms) VALUES (?1, ?2, ?3, ?4)",
    "INSERT INTO asp_mvcc_event_2 (partition_key, event_id, payload, created_at_ms) VALUES (?1, ?2, ?3, ?4)",
    "INSERT INTO asp_mvcc_event_3 (partition_key, event_id, payload, created_at_ms) VALUES (?1, ?2, ?3, ?4)",
];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TursoMvccWriteErrorCode {
    InvalidRequest,
    Busy,
    BusySnapshot,
    DuplicateIdentity,
    Io,
    Backend,
}

#[derive(Debug)]
pub struct TursoMvccWriteError {
    pub code: TursoMvccWriteErrorCode,
    pub retryable: bool,
    pub message: String,
}

impl fmt::Display for TursoMvccWriteError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}", self.message)
    }
}

impl std::error::Error for TursoMvccWriteError {}

impl TursoMvccStore {
    pub async fn append_batch_typed(
        &self,
        events: &[TursoMvccEvent],
        retry_policy: &StorageRetryPolicy,
    ) -> Result<TursoMvccBatchWriteReceipt, TursoMvccWriteError> {
        validate_events(events, self.inner.max_batch_rows)?;
        retry_policy
            .validate()
            .map_err(|error| TursoMvccWriteError {
                code: TursoMvccWriteErrorCode::InvalidRequest,
                retryable: false,
                message: error.message,
            })?;

        let mut retry_count = 0_usize;
        let mut busy_count = 0_usize;
        let mut snapshot_conflict_count = 0_usize;
        let mut retry_delay_ms = 0_u64;
        loop {
            match self.append_batch_once_typed(events).await {
                Ok(()) => {
                    return Ok(TursoMvccBatchWriteReceipt {
                        schema_id: BATCH_WRITE_RECEIPT_SCHEMA_ID.to_owned(),
                        attempted_rows: events.len(),
                        committed_rows: events.len(),
                        retry_count,
                        busy_count,
                        snapshot_conflict_count,
                        retry_delay_ms,
                        optimization: self.optimization_receipt(),
                    });
                }
                Err(error) => {
                    match error.code {
                        TursoMvccWriteErrorCode::Busy => busy_count += 1,
                        TursoMvccWriteErrorCode::BusySnapshot => {
                            snapshot_conflict_count += 1;
                        }
                        _ => {}
                    }
                    if !error.retryable || retry_count + 1 >= retry_policy.max_attempts as usize {
                        return Err(error);
                    }
                    retry_count += 1;
                    let attempt = retry_count as u32;
                    let exponential = retry_policy
                        .base_delay_ms
                        .saturating_mul(1_u64 << attempt.min(20));
                    let jitter = if retry_policy.jitter_ms == 0 {
                        0
                    } else {
                        (u64::from(attempt) * 17) % (retry_policy.jitter_ms + 1)
                    };
                    let delay_ms = exponential
                        .min(retry_policy.max_delay_ms)
                        .saturating_add(jitter);
                    retry_delay_ms = retry_delay_ms.saturating_add(delay_ms);
                    tokio::time::sleep(Duration::from_millis(delay_ms)).await;
                }
            }
        }
    }

    async fn append_batch_once_typed(
        &self,
        events: &[TursoMvccEvent],
    ) -> Result<(), TursoMvccWriteError> {
        let lane_index = event_shard(&events[0].partition_key) % self.inner.lanes.len();
        let lane = self.inner.lanes[lane_index].lock().await;
        lane.execute("BEGIN CONCURRENT", ())
            .await
            .map_err(classify_turso_write_error)?;

        let write_result = async {
            for shard in 0..INSERT_EVENT_SQL.len() {
                let shard_events = events
                    .iter()
                    .filter(|event| event_shard(&event.partition_key) == shard)
                    .collect::<Vec<_>>();
                if shard_events.is_empty() {
                    continue;
                }
                let mut statement = lane
                    .prepare_cached(INSERT_EVENT_SQL[shard])
                    .await
                    .map_err(classify_turso_write_error)?;
                for event in shard_events {
                    statement
                        .execute((
                            event.partition_key.as_str(),
                            event.event_id.as_str(),
                            event.payload.as_slice(),
                            event.created_at_ms,
                        ))
                        .await
                        .map_err(classify_turso_write_error)?;
                }
            }
            Ok::<(), TursoMvccWriteError>(())
        }
        .await;

        match write_result {
            Ok(()) => lane
                .execute("COMMIT", ())
                .await
                .map(|_| ())
                .map_err(classify_turso_write_error),
            Err(error) => {
                let _ = lane.execute("ROLLBACK", ()).await;
                Err(error)
            }
        }
    }
}

fn validate_events(
    events: &[TursoMvccEvent],
    max_batch_rows: usize,
) -> Result<(), TursoMvccWriteError> {
    if events.is_empty() || events.len() > max_batch_rows {
        return Err(TursoMvccWriteError {
            code: TursoMvccWriteErrorCode::InvalidRequest,
            retryable: false,
            message: format!("MVCC batch row count must be in 1..={max_batch_rows}"),
        });
    }
    let mut identities = HashSet::with_capacity(events.len());
    for event in events {
        if event.partition_key.is_empty() || event.event_id.is_empty() {
            return Err(TursoMvccWriteError {
                code: TursoMvccWriteErrorCode::InvalidRequest,
                retryable: false,
                message: "MVCC event partition_key and event_id must be non-empty".to_owned(),
            });
        }
        if !identities.insert((&event.partition_key, &event.event_id)) {
            return Err(TursoMvccWriteError {
                code: TursoMvccWriteErrorCode::DuplicateIdentity,
                retryable: false,
                message: format!(
                    "duplicate MVCC event identity in batch: {}/{}",
                    event.partition_key, event.event_id
                ),
            });
        }
    }
    Ok(())
}

fn classify_turso_write_error(error: turso::Error) -> TursoMvccWriteError {
    let code = match &error {
        turso::Error::Busy(_) => TursoMvccWriteErrorCode::Busy,
        turso::Error::BusySnapshot(_) => TursoMvccWriteErrorCode::BusySnapshot,
        turso::Error::Constraint(_) => TursoMvccWriteErrorCode::DuplicateIdentity,
        turso::Error::IoError(_, _) => TursoMvccWriteErrorCode::Io,
        _ => TursoMvccWriteErrorCode::Backend,
    };
    TursoMvccWriteError {
        retryable: matches!(
            code,
            TursoMvccWriteErrorCode::Busy | TursoMvccWriteErrorCode::BusySnapshot
        ),
        code,
        message: error.to_string(),
    }
}
