use agent_semantic_context_product::{
    ContextProductEvent, Digest, JSON_SAFE_INTEGER_MAX, ProtocolId, StateAuthorityReceipt,
    UncheckedContextProductStateV1,
};
use agent_semantic_loop::{
    AuthoritativeStateRecord, CompareAndAppendOutcome, PortFuture, RunCommit, RunCommitReceipt,
    RunCommitStore, StateHead,
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

use crate::{
    storage_contract::{StorageError, StorageErrorCode},
    turso_mvcc_partition::{
        TursoMvccExpectedHead, TursoMvccPartitionAlias, TursoMvccPartitionCommit,
        TursoMvccPartitionCommitOutcome, TursoMvccPartitionHead, TursoMvccPartitionRecord,
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
    search_loop_capabilities:
        Vec<agent_semantic_loop::search_capability::UncheckedSearchLoopCapabilityV1>,
    search_loop_runtime:
        Option<agent_semantic_loop::search_runtime::UncheckedSearchLoopRuntimeBindingV1>,
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

struct PreparedInitialization {
    state: UncheckedContextProductStateV1,
    authority_receipt: StateAuthorityReceipt,
    initial_capabilities:
        Vec<agent_semantic_loop::search_capability::UncheckedSearchLoopCapabilityV1>,
    search_loop_runtime:
        Option<agent_semantic_loop::search_runtime::UncheckedSearchLoopRuntimeBindingV1>,
    commit: TursoMvccPartitionCommit,
    aliases: Vec<TursoMvccPartitionAlias>,
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

    pub async fn load_search_loop_runtime(
        &self,
        loop_id: &ProtocolId,
    ) -> Result<Option<AuthoritativeStateRecord>, StorageError> {
        let Some(head) = self
            .store
            .load_partition_head_by_alias("search-loop", loop_id.as_str())
            .await
            .map_err(backend)?
        else {
            if self
                .store
                .resolve_partition_alias("search-loop", loop_id.as_str())
                .await
                .map_err(backend)?
                .is_some()
            {
                return Err(invalid(
                    "search-loop alias points to a missing MVCC partition",
                ));
            }
            return Ok(None);
        };
        let record = decode_head(&head)?;
        let Some(runtime) = &record.search_loop_runtime else {
            return Err(invalid(
                "search-loop alias points to a partition without a runtime binding",
            ));
        };
        let runtime = agent_semantic_loop::search_runtime::SearchLoopRuntimeBindingV1::validate(
            runtime.clone(),
        )
        .map_err(|error| invalid(format!("invalid search-loop runtime binding: {error}")))?;
        if runtime.loop_id() != loop_id {
            return Err(invalid(
                "search-loop alias resolved a different loop identity",
            ));
        }
        Ok(Some(record))
    }

    pub async fn load_search_loop_capability(
        &self,
        run_id: &ProtocolId,
        token_digest: &Digest,
    ) -> Result<Option<agent_semantic_loop::search_capability::SearchLoopCapabilityV1>, StorageError>
    {
        let Some(head) = self
            .store
            .load_partition_head(&partition_key(run_id))
            .await
            .map_err(backend)?
        else {
            return Ok(None);
        };
        decode_head(&head)?;
        let projection: ContextRunProjection =
            serde_json::from_slice(&head.projection).map_err(|error| {
                backend(format!("failed to decode context run projection: {error}"))
            })?;
        projection
            .search_loop_capabilities
            .into_iter()
            .find(|capability| capability.token_digest() == token_digest)
            .map(agent_semantic_loop::search_capability::SearchLoopCapabilityV1::from_unchecked)
            .transpose()
            .map_err(|error| invalid(format!("invalid search-loop capability: {error}")))
    }

    pub async fn initialize(
        &self,
        state: UncheckedContextProductStateV1,
        initial_capabilities: Vec<agent_semantic_loop::search_capability::SearchLoopCapabilityV1>,
        search_loop_runtime: Option<
            agent_semantic_loop::search_runtime::SearchLoopRuntimeBindingV1,
        >,
        issued_at_ms: u64,
    ) -> Result<RunCommitReceipt, StorageError> {
        let prepared = prepare_initialization(
            &self.authority_id,
            state,
            initial_capabilities,
            search_loop_runtime,
            issued_at_ms,
        )?;
        let outcome = self
            .store
            .compare_and_append_partition_with_aliases(&prepared.commit, &prepared.aliases)
            .await
            .map_err(backend)?;
        resolve_initialization_outcome(prepared, outcome)
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
            let expected = commit.expected();
            for capability in commit.search_loop_capabilities() {
                agent_semantic_loop::search_capability::SearchLoopCapabilityV1::from_unchecked(
                    capability.clone(),
                )
                .map_err(|error| invalid(format!("invalid search-loop capability: {error}")))?;
            }
            let projection = encode_projection(
                next_state,
                &authority_receipt,
                commit.search_loop_capabilities(),
                commit.search_loop_runtime(),
            )?;
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

#[cfg(test)]
#[path = "../tests/unit/context_run_mvcc.rs"]
mod tests;

fn prepare_initialization(
    authority_id: &ProtocolId,
    state: UncheckedContextProductStateV1,
    capabilities: Vec<agent_semantic_loop::search_capability::SearchLoopCapabilityV1>,
    runtime: Option<agent_semantic_loop::search_runtime::SearchLoopRuntimeBindingV1>,
    issued_at_ms: u64,
) -> Result<PreparedInitialization, StorageError> {
    validate_initial_state(&state)?;
    let authority_receipt = authority_receipt(authority_id, &state, issued_at_ms)?;
    let initial_capabilities = materialize_initial_capabilities(capabilities, &state)?;
    if let Some(runtime) = &runtime {
        validate_runtime_binding(runtime, &state, &authority_receipt)?;
    }
    let search_loop_runtime = runtime
        .map(agent_semantic_loop::search_runtime::SearchLoopRuntimeBindingV1::into_unchecked);
    let commit = build_initial_commit(
        &state,
        &authority_receipt,
        &initial_capabilities,
        search_loop_runtime.as_ref(),
        issued_at_ms,
    )?;
    let aliases = initial_runtime_aliases(search_loop_runtime.as_ref())?;
    Ok(PreparedInitialization {
        state,
        authority_receipt,
        initial_capabilities,
        search_loop_runtime,
        commit,
        aliases,
    })
}

fn validate_initial_state(state: &UncheckedContextProductStateV1) -> Result<(), StorageError> {
    state.validate().map_err(contract_error)?;
    if state.revision != 0
        || state.previous_state_digest.is_some()
        || state.last_event_sequence != 0
    {
        return Err(invalid(
            "initial context state must be revision and sequence zero",
        ));
    }
    Ok(())
}

fn materialize_initial_capabilities(
    capabilities: Vec<agent_semantic_loop::search_capability::SearchLoopCapabilityV1>,
    state: &UncheckedContextProductStateV1,
) -> Result<
    Vec<agent_semantic_loop::search_capability::UncheckedSearchLoopCapabilityV1>,
    StorageError,
> {
    for capability in &capabilities {
        validate_capability_binding(capability, state)?;
    }
    let mutations = capabilities
        .into_iter()
        .map(|capability| {
            agent_semantic_loop::search_capability::SearchLoopCapabilityMutation::Issue(Box::new(
                capability,
            ))
        })
        .collect::<Vec<_>>();
    let mut materialized = Vec::new();
    agent_semantic_loop::search_capability::apply_capability_mutations(
        &mut materialized,
        &mutations,
    )
    .map_err(|error| invalid(format!("invalid initial search-loop capability: {error}")))?;
    Ok(materialized)
}

fn build_initial_commit(
    state: &UncheckedContextProductStateV1,
    authority_receipt: &StateAuthorityReceipt,
    initial_capabilities: &[agent_semantic_loop::search_capability::UncheckedSearchLoopCapabilityV1],
    search_loop_runtime: Option<
        &agent_semantic_loop::search_runtime::UncheckedSearchLoopRuntimeBindingV1,
    >,
    issued_at_ms: u64,
) -> Result<TursoMvccPartitionCommit, StorageError> {
    let projection = encode_projection(
        state,
        authority_receipt,
        initial_capabilities,
        search_loop_runtime,
    )?;
    Ok(TursoMvccPartitionCommit {
        partition_key: partition_key(&state.run_id),
        expected: None,
        next_revision: 0,
        next_head_digest: head_guard_digest(&state_head(state))?.as_str().to_string(),
        next_projection: projection,
        records: Vec::new(),
        committed_at_ms: safe_i64(issued_at_ms, "issuedAtMs")?,
    })
}

fn initial_runtime_aliases(
    runtime: Option<&agent_semantic_loop::search_runtime::UncheckedSearchLoopRuntimeBindingV1>,
) -> Result<Vec<TursoMvccPartitionAlias>, StorageError> {
    runtime
        .map(|runtime| {
            agent_semantic_loop::search_runtime::SearchLoopRuntimeBindingV1::validate(
                runtime.clone(),
            )
            .map_err(|error| invalid(format!("invalid search-loop runtime binding: {error}")))
            .and_then(|runtime| {
                TursoMvccPartitionAlias::parse("search-loop", runtime.loop_id().as_str())
                    .map_err(invalid)
            })
        })
        .transpose()
        .map(|alias| alias.into_iter().collect())
}

fn resolve_initialization_outcome(
    prepared: PreparedInitialization,
    outcome: TursoMvccPartitionCommitOutcome,
) -> Result<RunCommitReceipt, StorageError> {
    match outcome {
        TursoMvccPartitionCommitOutcome::Committed(_) => Ok(RunCommitReceipt {
            authority_receipt: prepared.authority_receipt,
        }),
        TursoMvccPartitionCommitOutcome::Conflict(Some(observed)) => {
            resolve_existing_initialization(prepared, observed)
        }
        TursoMvccPartitionCommitOutcome::Conflict(None) => Err(invalid(
            "MVCC initialization conflicted without an observed partition head",
        )),
    }
}

fn resolve_existing_initialization(
    prepared: PreparedInitialization,
    observed: TursoMvccPartitionHead,
) -> Result<RunCommitReceipt, StorageError> {
    let observed_projection: ContextRunProjection = serde_json::from_slice(&observed.projection)
        .map_err(|error| backend(format!("failed to decode context run projection: {error}")))?;
    let observed = decode_head(&observed)?;
    if observed.state == prepared.state
        && observed_projection.search_loop_capabilities == prepared.initial_capabilities
        && observed_projection.search_loop_runtime == prepared.search_loop_runtime
    {
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
    let mut event_ids = BTreeSet::new();
    for (offset, event) in commit.events().iter().enumerate() {
        let expected_sequence = expected.last_event_sequence + offset as u64 + 1;
        if event.run_id() != &expected.run_id || event.sequence() != expected_sequence {
            return Err(invalid(
                "context run event identity or sequence is not contiguous",
            ));
        }
        if !event_ids.insert(event.event_id()) {
            return Err(invalid(
                "context run commit contains duplicate event identity",
            ));
        }
    }
    Ok(())
}

impl agent_semantic_loop::SearchLoopRuntimeStore for TursoMvccContextRunStore {
    fn load_by_loop_id<'a>(
        &'a self,
        loop_id: &'a ProtocolId,
    ) -> PortFuture<'a, Option<AuthoritativeStateRecord>, Self::Error> {
        Box::pin(async move { self.load_search_loop_runtime(loop_id).await })
    }
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

fn validate_capability_binding(
    capability: &agent_semantic_loop::search_capability::SearchLoopCapabilityV1,
    state: &UncheckedContextProductStateV1,
) -> Result<(), StorageError> {
    let binding = capability.state_binding();
    if binding.run_id() != &state.run_id
        || binding.revision() != state.revision
        || binding.state_digest() != &state.state_digest
        || binding.authority_receipt_ref() != &state.authority_receipt_ref
        || capability.context_binding_digest() != &state.context.binding_digest
    {
        return Err(invalid(
            "search-loop capability binding disagrees with context run state",
        ));
    }
    Ok(())
}

fn encode_projection(
    state: &UncheckedContextProductStateV1,
    authority_receipt: &StateAuthorityReceipt,
    search_loop_capabilities: &[
        agent_semantic_loop::search_capability::UncheckedSearchLoopCapabilityV1
    ],
    search_loop_runtime: Option<
        &agent_semantic_loop::search_runtime::UncheckedSearchLoopRuntimeBindingV1,
    >,
) -> Result<Vec<u8>, StorageError> {
    serde_json::to_vec(&ContextRunProjection {
        schema_id: PROJECTION_SCHEMA_ID.to_string(),
        state: state.clone(),
        authority_receipt: authority_receipt.clone(),
        search_loop_capabilities: search_loop_capabilities.to_vec(),
        search_loop_runtime: search_loop_runtime.cloned(),
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
    for capability in &projection.search_loop_capabilities {
        agent_semantic_loop::search_capability::SearchLoopCapabilityV1::from_unchecked(
            capability.clone(),
        )
        .map_err(|error| invalid(format!("invalid search-loop capability: {error}")))?;
    }
    if let Some(runtime) = &projection.search_loop_runtime {
        let runtime = agent_semantic_loop::search_runtime::SearchLoopRuntimeBindingV1::validate(
            runtime.clone(),
        )
        .map_err(|error| invalid(format!("invalid search-loop runtime binding: {error}")))?;
        validate_runtime_binding(&runtime, &projection.state, &projection.authority_receipt)?;
    }
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
        search_loop_capabilities: projection.search_loop_capabilities,
        search_loop_runtime: projection.search_loop_runtime,
    })
}

fn validate_runtime_binding(
    runtime: &agent_semantic_loop::search_runtime::SearchLoopRuntimeBindingV1,
    state: &UncheckedContextProductStateV1,
    authority_receipt: &StateAuthorityReceipt,
) -> Result<(), StorageError> {
    if runtime.run_id() != &state.run_id
        || runtime.context_binding_digest() != &state.context.binding_digest
        || authority_receipt.run_id != state.run_id
    {
        return Err(invalid(
            "search-loop runtime binding disagrees with authoritative context state",
        ));
    }
    Ok(())
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
