// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Typed single-writer admission queue for one workspace database owner.

use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use tokio::sync::{mpsc, oneshot};

use super::{
    ProviderIncrementalOwnerWrite, ProviderIncrementalWriteReceipt, ProviderOwnerInventoryWrite,
    ProviderOwnerInventoryWriteReceipt, ProviderTreeSitterOwnerResult,
    ProviderTreeSitterOwnerWriteReceipt, ProviderTreeSitterQueryIdentity,
    WorkspaceDbWriteFinishMode, WorkspaceDbWriteFinishReceipt,
};

/// A mutation admitted by the workspace owner's only writer actor.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum WorkspaceDbWriteOperation {
    CommitSourceIndexGeneration {
        request: crate::ClientDbSourceIndexRefreshRequest,
        materialization: crate::runtime_server_workspace::WorkspaceCanonicalMaterialization,
    },
    WriteProviderOwner(ProviderIncrementalOwnerWrite),
    UpsertProviderInventory(ProviderOwnerInventoryWrite),
    WriteTreeSitterOwner {
        query: ProviderTreeSitterQueryIdentity,
        result: ProviderTreeSitterOwnerResult,
    },
    FinishWrites(WorkspaceDbWriteFinishMode),
}

/// A committed result returned for one typed mutation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum WorkspaceDbWriteResult {
    SourceIndexGeneration {
        receipt: crate::ClientDbSourceIndexRefreshReport,
        materialization: crate::runtime_server_workspace::WorkspaceCanonicalMaterialization,
    },
    ProviderOwner(ProviderIncrementalWriteReceipt),
    ProviderInventory(ProviderOwnerInventoryWriteReceipt),
    TreeSitterOwner(ProviderTreeSitterOwnerWriteReceipt),
    WriteFinish(WorkspaceDbWriteFinishReceipt),
}

/// Client-supplied identity and typed payload for one queued mutation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorkspaceDbWriteRequest {
    pub workspace_identity: String,
    pub request_id: String,
    pub idempotency_key: String,
    pub operation: WorkspaceDbWriteOperation,
}

/// Actor-assigned ordering evidence for one admitted mutation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct WorkspaceDbWriteAdmission {
    pub sequence: u64,
    pub batch_sequence: u64,
    pub batch_index: u32,
}

/// Typed owner response carrying admission evidence and the committed result.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorkspaceDbWriteResponse {
    pub admission: WorkspaceDbWriteAdmission,
    pub result: WorkspaceDbWriteResult,
}

/// A bounded batch received in deterministic queue order.
pub struct WorkspaceDbWriteBatch {
    pub batch_sequence: u64,
    submissions: Vec<WorkspaceDbWriteSubmission>,
}

impl WorkspaceDbWriteBatch {
    /// Return whether this batch contains no mutations.
    pub fn is_empty(&self) -> bool {
        self.submissions.is_empty()
    }

    /// Inspect requests and actor-assigned ordering without exposing transport handles.
    #[allow(dead_code)]
    pub fn requests(
        &self,
    ) -> impl ExactSizeIterator<Item = (&WorkspaceDbWriteRequest, WorkspaceDbWriteAdmission)> {
        self.submissions
            .iter()
            .map(|submission| (&submission.request, submission.admission))
    }

    /// Complete every admitted request after the owner commits or rejects the batch.
    #[allow(dead_code)]
    pub fn complete(self, results: Vec<Result<WorkspaceDbWriteResult, String>>) {
        assert_eq!(
            self.submissions.len(),
            results.len(),
            "workspace writer batch completion must cover every admitted request"
        );
        for (submission, result) in self.submissions.into_iter().zip(results) {
            let result = result.map(|result| WorkspaceDbWriteResponse {
                admission: submission.admission,
                result,
            });
            let _ = submission.response.send(result);
        }
    }
}

struct WorkspaceDbWriteSubmission {
    request: WorkspaceDbWriteRequest,
    admission: WorkspaceDbWriteAdmission,
    response: oneshot::Sender<Result<WorkspaceDbWriteResponse, String>>,
}

struct PendingWorkspaceDbWriteSubmission {
    request: WorkspaceDbWriteRequest,
    response: oneshot::Sender<Result<WorkspaceDbWriteResponse, String>>,
}

/// Cloneable typed submission surface for concurrent owner clients.
#[derive(Clone, Debug)]
pub struct WorkspaceDbWriterClient {
    sender: mpsc::Sender<PendingWorkspaceDbWriteSubmission>,
}

impl WorkspaceDbWriterClient {
    /// Submit one typed mutation and await its committed result.
    pub async fn submit(
        &self,
        request: WorkspaceDbWriteRequest,
    ) -> Result<WorkspaceDbWriteResponse, String> {
        let (response, receipt) = oneshot::channel();
        self.sender
            .send(PendingWorkspaceDbWriteSubmission { request, response })
            .await
            .map_err(|_| "workspace database writer actor is unavailable".to_owned())?;
        receipt
            .await
            .map_err(|_| "workspace database writer actor dropped the request".to_owned())?
    }
}

/// Lifecycle-owned receiver for the workspace's only writer actor.
pub struct WorkspaceDbWriterActor {
    receiver: mpsc::Receiver<PendingWorkspaceDbWriteSubmission>,
    next_sequence: Arc<AtomicU64>,
    next_batch_sequence: u64,
}

impl WorkspaceDbWriterActor {
    /// Receive one request and drain an immediately available bounded batch.
    pub async fn receive_batch(
        &mut self,
        maximum_batch_size: usize,
    ) -> Option<WorkspaceDbWriteBatch> {
        let first = self.receiver.recv().await?;
        let maximum_batch_size = maximum_batch_size.max(1);
        let mut pending = Vec::with_capacity(maximum_batch_size);
        pending.push(first);
        while pending.len() < maximum_batch_size {
            match self.receiver.try_recv() {
                Ok(submission) => pending.push(submission),
                Err(mpsc::error::TryRecvError::Empty | mpsc::error::TryRecvError::Disconnected) => {
                    break;
                }
            }
        }
        let batch_sequence = self.next_batch_sequence;
        self.next_batch_sequence = self.next_batch_sequence.saturating_add(1);
        let submissions = pending
            .into_iter()
            .enumerate()
            .map(|(batch_index, pending)| WorkspaceDbWriteSubmission {
                request: pending.request,
                admission: WorkspaceDbWriteAdmission {
                    sequence: self.next_sequence.fetch_add(1, Ordering::Relaxed),
                    batch_sequence,
                    batch_index: batch_index.try_into().unwrap_or(u32::MAX),
                },
                response: pending.response,
            })
            .collect();
        Some(WorkspaceDbWriteBatch {
            batch_sequence,
            submissions,
        })
    }
}

/// Construct one bounded client/actor pair for a workspace owner epoch.
pub fn workspace_db_writer_channel(
    queue_capacity: usize,
) -> (WorkspaceDbWriterClient, WorkspaceDbWriterActor) {
    let (sender, receiver) = mpsc::channel(queue_capacity.max(1));
    (
        WorkspaceDbWriterClient { sender },
        WorkspaceDbWriterActor {
            receiver,
            next_sequence: Arc::new(AtomicU64::new(0)),
            next_batch_sequence: 0,
        },
    )
}

/// Run the lifecycle-owned single writer until every client sender is closed.
pub async fn run_workspace_db_writer_actor(
    mut actor: WorkspaceDbWriterActor,
    mut connection: turso::Connection,
    maximum_batch_size: usize,
) {
    while let Some(batch) = actor.receive_batch(maximum_batch_size).await {
        if batch.is_empty() {
            continue;
        }
        for (index, submission) in batch.submissions.into_iter().enumerate() {
            let expected_index = u32::try_from(index).unwrap_or(u32::MAX);
            let result = if submission.admission.batch_sequence != batch.batch_sequence
                || submission.admission.batch_index != expected_index
            {
                Err("workspace database writer admission ordering drift".to_owned())
            } else {
                execute_operation(&mut connection, submission.request).await
            }
            .map(|result| WorkspaceDbWriteResponse {
                admission: submission.admission,
                result,
            });
            let _ = submission.response.send(result);
        }
    }
}

async fn execute_operation(
    connection: &mut turso::Connection,
    request: WorkspaceDbWriteRequest,
) -> Result<WorkspaceDbWriteResult, String> {
    match request.operation {
        WorkspaceDbWriteOperation::CommitSourceIndexGeneration {
            request: refresh,
            mut materialization,
        } => {
            if materialization.workspace_identity != request.workspace_identity {
                return Err(format!(
                    "workspace generation materialization writer identity mismatch: admitted={} materialized={}",
                    request.workspace_identity, materialization.workspace_identity
                ));
            }
            super::core::refresh_turso_source_index_import_on_connection(
                connection,
                refresh,
                &mut materialization,
            )
            .await
            .map(|receipt| WorkspaceDbWriteResult::SourceIndexGeneration {
                receipt,
                materialization,
            })
        }
        WorkspaceDbWriteOperation::WriteProviderOwner(request) => {
            super::provider_incremental::write_provider_incremental_owner_on_connection(
                connection, &request,
            )
            .await
            .map(WorkspaceDbWriteResult::ProviderOwner)
        }
        WorkspaceDbWriteOperation::UpsertProviderInventory(request) => {
            super::provider_treesitter_write::upsert_provider_owner_inventory_on_connection(
                connection, &request,
            )
            .await
            .map(WorkspaceDbWriteResult::ProviderInventory)
        }
        WorkspaceDbWriteOperation::WriteTreeSitterOwner { query, result } => {
            super::provider_treesitter_write::write_provider_treesitter_owner_result_on_connection(
                connection, &query, &result,
            )
            .await
            .map(WorkspaceDbWriteResult::TreeSitterOwner)
        }
        WorkspaceDbWriteOperation::FinishWrites(mode) => finish_writes(connection, mode)
            .await
            .map(WorkspaceDbWriteResult::WriteFinish),
    }
}

async fn finish_writes(
    connection: &turso::Connection,
    mode: WorkspaceDbWriteFinishMode,
) -> Result<WorkspaceDbWriteFinishReceipt, String> {
    connection
        .cacheflush()
        .map_err(|error| format!("failed to flush workspace owner database: {error}"))?;
    if mode == WorkspaceDbWriteFinishMode::ResidentBatch {
        return Ok(WorkspaceDbWriteFinishReceipt {
            cache_flush_count: 1,
            checkpoint_mode: mode,
            checkpoint_busy: None,
            log_frames: None,
            checkpointed_frames: None,
        });
    }
    let mut rows = connection
        .query("PRAGMA wal_checkpoint(TRUNCATE)", ())
        .await
        .map_err(|error| format!("failed to checkpoint workspace owner database: {error}"))?;
    let row = rows
        .next()
        .await
        .map_err(|error| format!("failed to read workspace owner checkpoint: {error}"))?
        .ok_or_else(|| "workspace owner checkpoint returned no receipt row".to_owned())?;
    Ok(WorkspaceDbWriteFinishReceipt {
        cache_flush_count: 1,
        checkpoint_mode: mode,
        checkpoint_busy: Some(
            row.get::<i64>(0)
                .map_err(|error| format!("failed to decode checkpoint busy state: {error}"))?,
        ),
        log_frames: Some(
            row.get::<i64>(1)
                .map_err(|error| format!("failed to decode checkpoint log frames: {error}"))?,
        ),
        checkpointed_frames: Some(
            row.get::<i64>(2)
                .map_err(|error| format!("failed to decode checkpointed frames: {error}"))?,
        ),
    })
}
