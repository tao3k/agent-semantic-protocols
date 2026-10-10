// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

use std::collections::BTreeSet;

use agent_semantic_context_product::ContextProductEvent;
use agent_semantic_context_product::Digest;
use agent_semantic_context_product::ExecutionAuthority;
use agent_semantic_context_product::ExecutionConsumed;
use agent_semantic_context_product::ExecutionConsumedEventType;
use agent_semantic_context_product::ExecutionRevoked;
use agent_semantic_context_product::ExecutionRevokedEventType;
use agent_semantic_context_product::ExecutionStarted;
use agent_semantic_context_product::ExecutionStartedEventType;
use agent_semantic_context_product::JSON_SAFE_INTEGER_MAX;
use agent_semantic_context_product::ProtocolId;
use agent_semantic_context_product::UncheckedContextProductStateV1;
use agent_semantic_context_product::ValidationError;
use agent_semantic_context_product::chained_event_log_digest;

use super::transition_core::canonical_digest;
use super::transition_core::commit_events;
use super::transition_core::derived_id;
use super::transition_core::next_clock;
use super::transition_core::require_expected_head;
use crate::GraphRouter;
use crate::GraphRouterError;
use crate::ProofResolver;
use crate::RunCommitStore;
use crate::TrustedClock;
use crate::ValidatedContextProductStateV1;
use crate::graph_router::authority_receipt_id;

#[derive(Clone, Debug)]
pub struct ExecutionStartSpec {
    pub event_id: ProtocolId,
    pub grant_id: ProtocolId,
}

#[derive(Clone, Debug)]
pub struct StartExecutionGroupRequest {
    pub run_id: ProtocolId,
    pub expected_revision: u64,
    pub expected_state_digest: Digest,
    pub expected_context_binding_digest: Digest,
    pub execution_group_id: ProtocolId,
    pub starts: Vec<ExecutionStartSpec>,
}

#[derive(Clone, Debug)]
pub struct ExecutionConsumptionSpec {
    pub event_id: ProtocolId,
    pub attempt_id: ProtocolId,
    pub result_receipt_ref: ProtocolId,
    pub result_digest: Digest,
}

#[derive(Clone, Debug)]
pub struct ConsumeExecutionGroupRequest {
    pub run_id: ProtocolId,
    pub expected_revision: u64,
    pub expected_state_digest: Digest,
    pub expected_context_binding_digest: Digest,
    pub execution_group_id: ProtocolId,
    pub consumptions: Vec<ExecutionConsumptionSpec>,
}

#[derive(Clone, Debug)]
pub struct ExecutionRevocationSpec {
    pub event_id: ProtocolId,
    pub admission_id: ProtocolId,
    pub reason_code: ProtocolId,
}

#[derive(Clone, Debug)]
pub struct RevokeExecutionGroupRequest {
    pub run_id: ProtocolId,
    pub expected_revision: u64,
    pub expected_state_digest: Digest,
    pub expected_context_binding_digest: Digest,
    pub execution_group_id: ProtocolId,
    pub revocations: Vec<ExecutionRevocationSpec>,
}

impl<S, P, C> GraphRouter<S, P, C>
where
    S: RunCommitStore,
    P: ProofResolver,
    C: TrustedClock,
{
    pub async fn start_execution_group(
        &self,
        request: StartExecutionGroupRequest,
    ) -> Result<ValidatedContextProductStateV1, GraphRouterError> {
        let current = self.load(&request.run_id).await?;
        require_expected_head(
            &current,
            request.expected_revision,
            &request.expected_state_digest,
            &request.expected_context_binding_digest,
        )?;
        if current
            .executions()
            .iter()
            .filter(|execution| {
                execution.execution_group_id() == &request.execution_group_id
                    && matches!(execution, ExecutionAuthority::Granted(_))
            })
            .count()
            != request.starts.len()
        {
            return Err(GraphRouterError::InvalidTransition(
                "execution start requires every group grant exactly once",
            ));
        }
        let indices = select_group_authorities(
            current.executions(),
            &request.execution_group_id,
            request.starts.iter().map(|start| &start.grant_id),
            |execution, id| {
                matches!(execution, ExecutionAuthority::Granted(_))
                    && execution.grant_id() == Some(id)
            },
            "execution start requires every group grant exactly once",
        )?;
        let (started_at_ms, next_revision, first_sequence) = next_clock(&self.clock, &current)?;
        let expected = current.head();
        let mut next_state: UncheckedContextProductStateV1 = current.wire().clone();
        let mut next_event_log_digest = current.wire().event_log_digest.clone();
        let mut events = Vec::with_capacity(indices.len());

        for (offset, (start, index)) in request.starts.iter().zip(indices).enumerate() {
            let sequence = checked_sequence(first_sequence, offset)?;
            let authority = &current.executions()[index];
            if !matches!(authority, ExecutionAuthority::Granted(_)) {
                unreachable!("selected execution is granted");
            }
            let grant_id = authority
                .grant_id()
                .expect("granted authority has grant id");
            let grant_digest = authority
                .grant_digest()
                .expect("granted authority has grant digest");
            let execution_group_id = authority.execution_group_id();
            let stage_ids = authority.stage_ids();
            let provider_id = authority.provider_id();
            let operation = authority.operation();
            let lease_fence = authority
                .lease_fence()
                .expect("granted authority has lease fence");
            let expires_at_ms = authority
                .expires_at_ms()
                .expect("granted authority has expiry");
            let provider_idempotency_key = authority
                .provider_idempotency_key()
                .expect("granted authority has provider idempotency key");
            if started_at_ms > expires_at_ms {
                return Err(GraphRouterError::Validation(
                    ValidationError::InvalidExecutionGrant,
                ));
            }
            let attempt_digest = canonical_digest(&serde_json::json!({
                "grantDigest": grant_digest,
                "leaseFence": lease_fence,
                "providerIdempotencyKey": provider_idempotency_key,
            }));
            let attempt_id = derived_id("execution-attempt", &attempt_digest)?;
            let mut event = ExecutionStarted {
                event_type: ExecutionStartedEventType::ExecutionStarted,
                event_id: start.event_id.clone(),
                run_id: request.run_id.clone(),
                sequence,
                state_revision: current.revision(),
                pre_state_digest: current.state_digest().clone(),
                grant_id: grant_id.clone(),
                grant_digest: grant_digest.clone(),
                attempt_id: attempt_id.clone(),
                attempt_digest: attempt_digest.clone(),
                execution_group_id: execution_group_id.clone(),
                stage_ids: stage_ids.to_vec(),
                provider_id: provider_id.clone(),
                operation: operation.clone(),
                lease_fence,
                expires_at_ms,
                provider_idempotency_key: provider_idempotency_key.clone(),
                event_digest: Digest::from_bytes(b"pending-execution-started"),
            };
            event.event_digest = event.recompute_event_digest();
            next_event_log_digest =
                chained_event_log_digest(&next_event_log_digest, &event.event_digest);
            next_state.executions[index] = ExecutionAuthority::in_flight(authority, &event)
                .expect("selected authority is granted");
            events.push(ContextProductEvent::ExecutionStarted(event));
        }
        finish_group_transition(
            &self.store,
            &current,
            expected,
            next_state,
            events,
            next_event_log_digest,
            next_revision,
            first_sequence,
            started_at_ms,
        )
        .await
    }

    pub async fn consume_execution_group(
        &self,
        request: ConsumeExecutionGroupRequest,
    ) -> Result<ValidatedContextProductStateV1, GraphRouterError> {
        let current = self.load(&request.run_id).await?;
        require_expected_head(
            &current,
            request.expected_revision,
            &request.expected_state_digest,
            &request.expected_context_binding_digest,
        )?;
        if request.consumptions.is_empty() {
            return Err(GraphRouterError::InvalidTransition(
                "execution consumption requires at least one in-flight group dispatch",
            ));
        }
        let indices = select_group_authorities(
            current.executions(),
            &request.execution_group_id,
            request
                .consumptions
                .iter()
                .map(|consumption| &consumption.attempt_id),
            |execution, id| {
                matches!(execution, ExecutionAuthority::InFlight(_))
                    && execution.attempt_id() == Some(id)
            },
            "execution consumption requires matching in-flight group dispatches",
        )?;
        let (consumed_at_ms, next_revision, first_sequence) = next_clock(&self.clock, &current)?;
        let expected = current.head();
        let mut next_state: UncheckedContextProductStateV1 = current.wire().clone();
        let mut next_event_log_digest = current.wire().event_log_digest.clone();
        let mut events = Vec::with_capacity(indices.len());

        for (offset, (consumption, index)) in request.consumptions.iter().zip(indices).enumerate() {
            let sequence = checked_sequence(first_sequence, offset)?;
            let authority = &current.executions()[index];
            if !matches!(authority, ExecutionAuthority::InFlight(_)) {
                unreachable!("selected execution is in flight");
            }
            let grant_id = authority
                .grant_id()
                .expect("in-flight authority has grant id");
            let grant_digest = authority
                .grant_digest()
                .expect("in-flight authority has grant digest");
            let attempt_id = authority
                .attempt_id()
                .expect("in-flight authority has attempt id");
            let execution_group_id = authority.execution_group_id();
            let stage_ids = authority.stage_ids();
            let provider_id = authority.provider_id();
            let operation = authority.operation();
            let lease_fence = authority
                .lease_fence()
                .expect("in-flight authority has lease fence");
            let expires_at_ms = authority
                .expires_at_ms()
                .expect("in-flight authority has expiry");
            if consumed_at_ms > expires_at_ms {
                return Err(GraphRouterError::Validation(
                    ValidationError::InvalidExecutionGrant,
                ));
            }
            let mut event = ExecutionConsumed {
                event_type: ExecutionConsumedEventType::ExecutionConsumed,
                event_id: consumption.event_id.clone(),
                run_id: request.run_id.clone(),
                sequence,
                state_revision: current.revision(),
                pre_state_digest: current.state_digest().clone(),
                grant_id: grant_id.clone(),
                grant_digest: grant_digest.clone(),
                attempt_id: attempt_id.clone(),
                execution_group_id: execution_group_id.clone(),
                stage_ids: stage_ids.to_vec(),
                provider_id: provider_id.clone(),
                operation: operation.clone(),
                result_receipt_ref: consumption.result_receipt_ref.clone(),
                result_digest: consumption.result_digest.clone(),
                lease_fence,
                event_digest: Digest::from_bytes(b"pending-execution-consumed"),
            };
            event.event_digest = event.recompute_event_digest();
            next_event_log_digest =
                chained_event_log_digest(&next_event_log_digest, &event.event_digest);
            next_state.executions[index] = ExecutionAuthority::consumed(authority, &event)
                .expect("selected authority is in flight");
            events.push(ContextProductEvent::ExecutionConsumed(event));
        }
        finish_group_transition(
            &self.store,
            &current,
            expected,
            next_state,
            events,
            next_event_log_digest,
            next_revision,
            first_sequence,
            consumed_at_ms,
        )
        .await
    }

    pub async fn revoke_execution_group(
        &self,
        request: RevokeExecutionGroupRequest,
    ) -> Result<ValidatedContextProductStateV1, GraphRouterError> {
        let current = self.load(&request.run_id).await?;
        require_expected_head(
            &current,
            request.expected_revision,
            &request.expected_state_digest,
            &request.expected_context_binding_digest,
        )?;
        if current
            .executions()
            .iter()
            .filter(|execution| {
                execution.execution_group_id() == &request.execution_group_id
                    && matches!(
                        execution,
                        ExecutionAuthority::Admitted(_)
                            | ExecutionAuthority::Granted(_)
                            | ExecutionAuthority::InFlight(_)
                    )
            })
            .count()
            != request.revocations.len()
        {
            return Err(GraphRouterError::InvalidTransition(
                "execution revocation requires every active group dispatch exactly once",
            ));
        }
        let indices = select_group_authorities(
            current.executions(),
            &request.execution_group_id,
            request
                .revocations
                .iter()
                .map(|revocation| &revocation.admission_id),
            |execution, id| {
                execution.admission_id() == Some(id)
                    && matches!(
                        execution,
                        ExecutionAuthority::Admitted(_)
                            | ExecutionAuthority::Granted(_)
                            | ExecutionAuthority::InFlight(_)
                    )
            },
            "execution revocation requires every active group dispatch exactly once",
        )?;
        let (revoked_at_ms, next_revision, first_sequence) = next_clock(&self.clock, &current)?;
        let expected = current.head();
        let mut next_state: UncheckedContextProductStateV1 = current.wire().clone();
        let mut next_event_log_digest = current.wire().event_log_digest.clone();
        let mut events = Vec::with_capacity(indices.len());

        for (offset, (revocation, index)) in request.revocations.iter().zip(indices).enumerate() {
            let sequence = checked_sequence(first_sequence, offset)?;
            let identity = revocable_identity(&current.executions()[index])?;
            let mut event = ExecutionRevoked {
                event_type: ExecutionRevokedEventType::ExecutionRevoked,
                event_id: revocation.event_id.clone(),
                run_id: request.run_id.clone(),
                sequence,
                state_revision: current.revision(),
                pre_state_digest: current.state_digest().clone(),
                reason_code: revocation.reason_code.clone(),
                execution_group_id: identity.execution_group_id.clone(),
                stage_ids: identity.stage_ids.clone(),
                provider_id: identity.provider_id.clone(),
                operation: identity.operation.clone(),
                admission_id: Some(identity.admission_id.clone()),
                grant_id: identity.grant_id.clone(),
                action_key: Some(identity.action_key.clone()),
                event_digest: Digest::from_bytes(b"pending-execution-revoked"),
            };
            event.event_digest = event.recompute_event_digest();
            next_event_log_digest =
                chained_event_log_digest(&next_event_log_digest, &event.event_digest);
            next_state.executions[index] =
                ExecutionAuthority::revoked(&current.executions()[index], &event);
            events.push(ContextProductEvent::ExecutionRevoked(event));
        }
        finish_group_transition(
            &self.store,
            &current,
            expected,
            next_state,
            events,
            next_event_log_digest,
            next_revision,
            first_sequence,
            revoked_at_ms,
        )
        .await
    }
}

fn checked_sequence(first_sequence: u64, offset: usize) -> Result<u64, GraphRouterError> {
    first_sequence
        .checked_add(offset as u64)
        .filter(|value| *value <= JSON_SAFE_INTEGER_MAX)
        .ok_or(GraphRouterError::Validation(
            ValidationError::RevisionExhausted,
        ))
}

fn select_group_authorities<'a, I, F>(
    executions: &[ExecutionAuthority],
    group_id: &ProtocolId,
    identities: I,
    matches_identity: F,
    error: &'static str,
) -> Result<Vec<usize>, GraphRouterError>
where
    I: IntoIterator<Item = &'a ProtocolId>,
    F: Fn(&ExecutionAuthority, &ProtocolId) -> bool,
{
    let identities = identities.into_iter().collect::<Vec<_>>();
    if identities.is_empty() {
        return Err(GraphRouterError::InvalidTransition(error));
    }
    let mut seen = BTreeSet::new();
    let indices = identities
        .iter()
        .map(|identity| {
            if !seen.insert(*identity) {
                return Err(GraphRouterError::InvalidTransition(error));
            }
            executions
                .iter()
                .position(|execution| {
                    execution.execution_group_id() == group_id
                        && matches_identity(execution, identity)
                })
                .ok_or(GraphRouterError::InvalidTransition(error))
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(indices)
}

struct RevocableIdentity {
    admission_id: ProtocolId,
    grant_id: Option<ProtocolId>,
    action_key: Digest,
    execution_group_id: ProtocolId,
    stage_ids: Vec<ProtocolId>,
    provider_id: ProtocolId,
    operation: ProtocolId,
}

fn revocable_identity(
    execution: &ExecutionAuthority,
) -> Result<RevocableIdentity, GraphRouterError> {
    match execution {
        ExecutionAuthority::Admitted(_)
        | ExecutionAuthority::Granted(_)
        | ExecutionAuthority::InFlight(_) => Ok(RevocableIdentity {
            admission_id: execution
                .admission_id()
                .expect("revocable authority has admission id")
                .clone(),
            grant_id: execution.grant_id().cloned(),
            action_key: execution
                .action_key()
                .expect("revocable authority has action key")
                .clone(),
            execution_group_id: execution.execution_group_id().clone(),
            stage_ids: execution.stage_ids().to_vec(),
            provider_id: execution.provider_id().clone(),
            operation: execution.operation().clone(),
        }),
        ExecutionAuthority::Consumed(_) | ExecutionAuthority::Revoked(_) => {
            Err(GraphRouterError::InvalidTransition(
                "execution revoke requires admitted, granted, or in-flight authority",
            ))
        }
    }
}

#[allow(clippy::too_many_arguments)]
async fn finish_group_transition<S: RunCommitStore>(
    store: &S,
    current: &ValidatedContextProductStateV1,
    expected: crate::StateHead,
    mut next_state: UncheckedContextProductStateV1,
    events: Vec<ContextProductEvent>,
    next_event_log_digest: Digest,
    next_revision: u64,
    first_sequence: u64,
    committed_at_ms: u64,
) -> Result<ValidatedContextProductStateV1, GraphRouterError> {
    next_state.revision = next_revision;
    next_state.previous_state_digest = Some(current.state_digest().clone());
    next_state.last_event_sequence = first_sequence + events.len() as u64 - 1;
    next_state.event_log_digest = next_event_log_digest;
    next_state.authority_receipt_ref =
        authority_receipt_id(&expected, next_revision, &next_state.event_log_digest)?;
    next_state.state_digest = next_state.recompute_state_digest();
    next_state
        .validate()
        .map_err(GraphRouterError::Validation)?;
    commit_events(
        store,
        expected,
        events,
        next_state,
        current.search_loop_capabilities().to_vec(),
        current.search_loop_runtime().cloned(),
        committed_at_ms,
    )
    .await
}
