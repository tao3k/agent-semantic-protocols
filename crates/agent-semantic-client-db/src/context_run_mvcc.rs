use agent_semantic_context_product::{
    ContextProductEvent, Digest, JSON_SAFE_INTEGER_MAX, ProtocolId, StateAuthorityReceipt,
    UncheckedContextProductStateV1,
};
use agent_semantic_loop::{
    AuthoritativeStateRecord, CompareAndAppendOutcome, PortFuture, RunCommit, RunCommitReceipt,
    RunCommitStore, StateHead,
};
use serde::{Deserialize, Serialize};

use crate::{
    storage_contract::{StorageError, StorageErrorCode},
    turso_mvcc_partition::{
        TursoMvccExpectedHead, TursoMvccPartitionCommit, TursoMvccPartitionCommitOutcome,
        TursoMvccPartitionHead, TursoMvccPartitionRecord,
    },
    turso_mvcc_store::TursoMvccStore,
};

const PROJECTION_SCHEMA_ID: &str = "asp.context-run-projection.v1";
const EVENT_RECORD_KIND: &str = "asp.context-product-event.v1";

#[derive(Clone)]
pub struct TursoMvccContextRunStore {
    store: TursoMvccStore,
    authority_id: ProtocolId,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ContextRunProjection {
    schema_id: String,
    state: UncheckedContextProductStateV1,
    authority_receipt: StateAuthorityReceipt,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct StateHeadDigestProjection<'a> {
    run_id: &'a ProtocolId,
    revision: u64,
    state_digest: &'a Digest,
    event_log_digest: &'a Digest,
    last_event_sequence: u64,
}

impl TursoMvccContextRunStore {
    pub fn new(store: TursoMvccStore, authority_id: ProtocolId) -> Self {
        Self {
            store,
            authority_id,
        }
    }

    pub fn mvcc_store(&self) -> &TursoMvccStore {
        &self.store
    }

    pub async fn initialize(
        &self,
        state: UncheckedContextProductStateV1,
        issued_at_ms: u64,
    ) -> Result<RunCommitReceipt, StorageError> {
        state.validate().map_err(contract_error)?;
        if state.revision != 0
            || state.previous_state_digest.is_some()
            || state.last_event_sequence != 0
        {
            return Err(invalid(
                "initial context state must be revision and sequence zero",
            ));
        }
        let authority_receipt = authority_receipt(&self.authority_id, &state, issued_at_ms)?;
        let projection = encode_projection(&state, &authority_receipt)?;
        let state_head = state_head(&state);
        let commit = TursoMvccPartitionCommit {
            partition_key: partition_key(&state.run_id),
            expected: None,
            next_revision: 0,
            next_head_digest: head_guard_digest(&state_head)?.as_str().to_string(),
            next_projection: projection,
            records: Vec::new(),
            committed_at_ms: safe_i64(issued_at_ms, "issuedAtMs")?,
        };
        match self
            .store
            .compare_and_append_partition(&commit)
            .await
            .map_err(backend)?
        {
            TursoMvccPartitionCommitOutcome::Committed(_) => {
                Ok(RunCommitReceipt { authority_receipt })
            }
            TursoMvccPartitionCommitOutcome::Conflict(Some(observed)) => {
                let observed = decode_head(&observed)?;
                if observed.state == state {
                    Ok(RunCommitReceipt {
                        authority_receipt: observed.authority_receipt,
                    })
                } else {
                    Err(StorageError::new(
                        StorageErrorCode::DuplicateIdentity,
                        false,
                        "context run is already initialized with a different state",
                    ))
                }
            }
            TursoMvccPartitionCommitOutcome::Conflict(None) => Err(invalid(
                "MVCC initialization conflicted without an observed partition head",
            )),
        }
    }
}

impl RunCommitStore for TursoMvccContextRunStore {
    type Error = StorageError;

    fn load<'a>(
        &'a self,
        run_id: &'a ProtocolId,
    ) -> PortFuture<'a, AuthoritativeStateRecord, Self::Error> {
        Box::pin(async move {
            let head = self
                .store
                .load_partition_head(&partition_key(run_id))
                .await
                .map_err(backend)?
                .ok_or_else(|| invalid(format!("context run `{run_id}` is not initialized")))?;
            decode_head(&head)
        })
    }

    fn compare_and_append<'a>(
        &'a self,
        commit: RunCommit,
    ) -> PortFuture<'a, CompareAndAppendOutcome, Self::Error> {
        Box::pin(async move {
            validate_run_commit(&commit)?;
            let next_state = commit.next_state();
            let authority_receipt =
                authority_receipt(&self.authority_id, next_state, commit.committed_at_ms())?;
            let projection = encode_projection(next_state, &authority_receipt)?;
            let expected = commit.expected();
            let records = commit
                .events()
                .iter()
                .map(event_record)
                .collect::<Result<Vec<_>, _>>()?;
            let mvcc_commit = TursoMvccPartitionCommit {
                partition_key: partition_key(&expected.run_id),
                expected: Some(TursoMvccExpectedHead {
                    revision: expected.revision,
                    head_digest: head_guard_digest(expected)?.as_str().to_string(),
                    last_sequence: expected.last_event_sequence,
                }),
                next_revision: next_state.revision,
                next_head_digest: head_guard_digest(&state_head(next_state))?
                    .as_str()
                    .to_string(),
                next_projection: projection,
                records,
                committed_at_ms: safe_i64(commit.committed_at_ms(), "committedAtMs")?,
            };
            match self
                .store
                .compare_and_append_partition(&mvcc_commit)
                .await
                .map_err(backend)?
            {
                TursoMvccPartitionCommitOutcome::Committed(receipt) => {
                    let observed = decode_head(&receipt.head)?;
                    authority_receipt
                        .validate_for_state(&observed.state)
                        .map_err(contract_error)?;
                    Ok(CompareAndAppendOutcome::Committed(RunCommitReceipt {
                        authority_receipt,
                    }))
                }
                TursoMvccPartitionCommitOutcome::Conflict(Some(observed)) => {
                    let observed = decode_head(&observed)?;
                    Ok(CompareAndAppendOutcome::Conflict(state_head(
                        &observed.state,
                    )))
                }
                TursoMvccPartitionCommitOutcome::Conflict(None) => {
                    Err(invalid("context run disappeared before MVCC commit"))
                }
            }
        })
    }
}

fn validate_run_commit(commit: &RunCommit) -> Result<(), StorageError> {
    let expected = commit.expected();
    let next = commit.next_state();
    next.validate().map_err(contract_error)?;
    if commit.events().is_empty() {
        return Err(invalid("context run commit must append at least one event"));
    }
    if next.run_id != expected.run_id
        || expected.revision.checked_add(1) != Some(next.revision)
        || next.previous_state_digest.as_ref() != Some(&expected.state_digest)
        || expected
            .last_event_sequence
            .checked_add(commit.events().len() as u64)
            != Some(next.last_event_sequence)
    {
        return Err(invalid(
            "context run commit does not extend the expected state head",
        ));
    }
    for (offset, event) in commit.events().iter().enumerate() {
        let expected_sequence = expected.last_event_sequence + offset as u64 + 1;
        if event.run_id() != &expected.run_id || event.sequence() != expected_sequence {
            return Err(invalid(
                "context run event identity or sequence is not contiguous",
            ));
        }
        if commit.events()[..offset]
            .iter()
            .any(|prior| prior.event_id() == event.event_id())
        {
            return Err(invalid(
                "context run commit contains duplicate event identity",
            ));
        }
    }
    Ok(())
}

fn event_record(event: &ContextProductEvent) -> Result<TursoMvccPartitionRecord, StorageError> {
    let payload = serde_json::to_vec(event)
        .map_err(|error| backend(format!("failed to encode context event: {error}")))?;
    TursoMvccPartitionRecord::new(event.event_id().as_str(), EVENT_RECORD_KIND, payload)
        .map_err(contract_error)
}

fn authority_receipt(
    authority_id: &ProtocolId,
    state: &UncheckedContextProductStateV1,
    issued_at_ms: u64,
) -> Result<StateAuthorityReceipt, StorageError> {
    safe_i64(issued_at_ms, "issuedAtMs")?;
    let mut receipt = StateAuthorityReceipt {
        receipt_id: state.authority_receipt_ref.clone(),
        authority_id: authority_id.clone(),
        run_id: state.run_id.clone(),
        revision: state.revision,
        state_digest: state.state_digest.clone(),
        event_log_digest: state.event_log_digest.clone(),
        issued_at_ms,
        receipt_digest: Digest::from_bytes(b"pending-state-authority-receipt"),
    };
    receipt.receipt_digest = receipt.recompute_receipt_digest();
    receipt.validate_for_state(state).map_err(contract_error)?;
    Ok(receipt)
}

fn encode_projection(
    state: &UncheckedContextProductStateV1,
    authority_receipt: &StateAuthorityReceipt,
) -> Result<Vec<u8>, StorageError> {
    serde_json::to_vec(&ContextRunProjection {
        schema_id: PROJECTION_SCHEMA_ID.to_string(),
        state: state.clone(),
        authority_receipt: authority_receipt.clone(),
    })
    .map_err(|error| backend(format!("failed to encode context run projection: {error}")))
}

fn decode_head(head: &TursoMvccPartitionHead) -> Result<AuthoritativeStateRecord, StorageError> {
    let projection: ContextRunProjection = serde_json::from_slice(&head.projection)
        .map_err(|error| backend(format!("failed to decode context run projection: {error}")))?;
    if projection.schema_id != PROJECTION_SCHEMA_ID {
        return Err(invalid("unknown context run projection schema"));
    }
    projection.state.validate().map_err(contract_error)?;
    projection
        .authority_receipt
        .validate_for_state(&projection.state)
        .map_err(contract_error)?;
    let state_head = state_head(&projection.state);
    if projection.state.revision != head.revision
        || projection.state.last_event_sequence != head.last_sequence
        || head_guard_digest(&state_head)?.as_str() != head.head_digest
    {
        return Err(invalid(
            "MVCC partition head disagrees with context run projection",
        ));
    }
    Ok(AuthoritativeStateRecord {
        state: projection.state,
        authority_receipt: projection.authority_receipt,
    })
}

fn state_head(state: &UncheckedContextProductStateV1) -> StateHead {
    StateHead {
        run_id: state.run_id.clone(),
        revision: state.revision,
        state_digest: state.state_digest.clone(),
        event_log_digest: state.event_log_digest.clone(),
        last_event_sequence: state.last_event_sequence,
    }
}

fn head_guard_digest(head: &StateHead) -> Result<Digest, StorageError> {
    let bytes = serde_json::to_vec(&StateHeadDigestProjection {
        run_id: &head.run_id,
        revision: head.revision,
        state_digest: &head.state_digest,
        event_log_digest: &head.event_log_digest,
        last_event_sequence: head.last_event_sequence,
    })
    .map_err(|error| backend(format!("failed to encode context run head: {error}")))?;
    Ok(Digest::from_bytes(&bytes))
}

fn partition_key(run_id: &ProtocolId) -> String {
    format!("context-run:{}", run_id.as_str())
}

fn safe_i64(value: u64, field: &'static str) -> Result<i64, StorageError> {
    if value > JSON_SAFE_INTEGER_MAX {
        Err(invalid(format!(
            "{field} exceeds the JSON safe integer range"
        )))
    } else {
        Ok(value as i64)
    }
}

fn contract_error(error: impl std::fmt::Display) -> StorageError {
    invalid(format!("context product contract rejected value: {error}"))
}

fn invalid(message: impl Into<String>) -> StorageError {
    StorageError::new(StorageErrorCode::InvalidRequest, false, message)
}

fn backend(message: impl Into<String>) -> StorageError {
    StorageError::new(StorageErrorCode::Backend, false, message)
}
