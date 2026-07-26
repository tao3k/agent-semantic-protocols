use agent_semantic_context_product::{
    ContextProductEvent, Digest, JSON_SAFE_INTEGER_MAX, ProtocolId, UncheckedContextProductStateV1,
    ValidationError,
};
use serde::Serialize;

use crate::{
    AuthoritativeStateRecord, CompareAndAppendOutcome, GraphRouterError, RunCommit, RunCommitStore,
    TrustedClock, ValidatedContextProductStateV1,
};

pub(super) fn require_expected_head(
    current: &ValidatedContextProductStateV1,
    expected_revision: u64,
    expected_state_digest: &Digest,
    expected_context_binding_digest: &Digest,
) -> Result<(), GraphRouterError> {
    if current.revision() != expected_revision {
        return Err(GraphRouterError::Validation(
            ValidationError::RevisionMismatch,
        ));
    }
    if current.state_digest() != expected_state_digest {
        return Err(GraphRouterError::Validation(
            ValidationError::StateChainMismatch,
        ));
    }
    if current.context_binding_digest() != expected_context_binding_digest {
        return Err(GraphRouterError::Validation(
            ValidationError::StaleContextBinding,
        ));
    }
    Ok(())
}

pub(super) fn next_clock<C: TrustedClock>(
    clock: &C,
    current: &ValidatedContextProductStateV1,
) -> Result<(u64, u64, u64), GraphRouterError> {
    let committed_at_ms = clock.now_ms();
    if committed_at_ms > JSON_SAFE_INTEGER_MAX {
        return Err(GraphRouterError::UnsafeClock(committed_at_ms));
    }
    let next_revision = current
        .revision()
        .checked_add(1)
        .filter(|value| *value <= JSON_SAFE_INTEGER_MAX)
        .ok_or(GraphRouterError::Validation(
            ValidationError::RevisionExhausted,
        ))?;
    let next_sequence = current
        .wire()
        .last_event_sequence
        .checked_add(1)
        .filter(|value| *value <= JSON_SAFE_INTEGER_MAX)
        .ok_or(GraphRouterError::Validation(
            ValidationError::RevisionExhausted,
        ))?;
    Ok((committed_at_ms, next_revision, next_sequence))
}

pub(super) async fn commit_state<S: RunCommitStore>(
    store: &S,
    expected: crate::StateHead,
    event: ContextProductEvent,
    next_state: UncheckedContextProductStateV1,
    search_loop_capabilities: Vec<crate::search_capability::UncheckedSearchLoopCapabilityV1>,
    search_loop_runtime: Option<crate::search_runtime::UncheckedSearchLoopRuntimeBindingV1>,
    committed_at_ms: u64,
) -> Result<ValidatedContextProductStateV1, GraphRouterError> {
    commit_events(
        store,
        expected,
        vec![event],
        next_state,
        search_loop_capabilities,
        search_loop_runtime,
        committed_at_ms,
    )
    .await
}

pub(super) async fn commit_events<S: RunCommitStore>(
    store: &S,
    expected: crate::StateHead,
    events: Vec<ContextProductEvent>,
    next_state: UncheckedContextProductStateV1,
    search_loop_capabilities: Vec<crate::search_capability::UncheckedSearchLoopCapabilityV1>,
    search_loop_runtime: Option<crate::search_runtime::UncheckedSearchLoopRuntimeBindingV1>,
    committed_at_ms: u64,
) -> Result<ValidatedContextProductStateV1, GraphRouterError> {
    if events.is_empty() {
        return Err(GraphRouterError::InvalidTransition(
            "state commit requires at least one event",
        ));
    }
    match store
        .compare_and_append(RunCommit {
            expected,
            events,
            next_state: next_state.clone(),
            committed_at_ms,
            search_loop_capabilities: search_loop_capabilities.clone(),
            search_loop_runtime: search_loop_runtime.clone(),
        })
        .await
        .map_err(|error| GraphRouterError::Store(error.to_string()))?
    {
        CompareAndAppendOutcome::Committed(receipt) => {
            ValidatedContextProductStateV1::from_authoritative_record(AuthoritativeStateRecord {
                state: next_state,
                authority_receipt: receipt.authority_receipt,
                search_loop_capabilities,
                search_loop_runtime,
            })
            .map_err(GraphRouterError::from)
        }
        CompareAndAppendOutcome::Conflict(head) => Err(GraphRouterError::Conflict(head)),
    }
}

pub(crate) fn canonical_digest<T: Serialize>(value: &T) -> Digest {
    let bytes = serde_json::to_vec(value).expect("transition projection serializes");
    Digest::from_bytes(&bytes)
}

pub(super) fn derived_id(prefix: &str, digest: &Digest) -> Result<ProtocolId, GraphRouterError> {
    ProtocolId::parse(format!(
        "{prefix}:{}",
        digest.as_str().trim_start_matches("blake3:")
    ))
    .map_err(GraphRouterError::Validation)
}
