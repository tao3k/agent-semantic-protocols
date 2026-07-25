use agent_semantic_context_product::{
    ContextProductEvent, Digest, ExecutionAuthority, ExecutionConsumed, ExecutionConsumedEventType,
    ExecutionRevoked, ExecutionRevokedEventType, ExecutionStarted, ExecutionStartedEventType,
    ProtocolId, UncheckedContextProductStateV1, ValidationError, chained_event_log_digest,
};

use super::transition_core::{
    canonical_digest, commit_state, derived_id, next_clock, require_expected_head,
};
use crate::{
    GraphRouter, GraphRouterError, ProofResolver, RunCommitStore, TrustedClock,
    ValidatedContextProductStateV1, graph_router::authority_receipt_id,
};

#[derive(Clone, Debug)]
pub struct StartExecutionRequest {
    pub run_id: ProtocolId,
    pub expected_revision: u64,
    pub expected_state_digest: Digest,
    pub expected_context_binding_digest: Digest,
    pub event_id: ProtocolId,
}

#[derive(Clone, Debug)]
pub struct ConsumeExecutionRequest {
    pub run_id: ProtocolId,
    pub expected_revision: u64,
    pub expected_state_digest: Digest,
    pub expected_context_binding_digest: Digest,
    pub event_id: ProtocolId,
    pub result_receipt_ref: ProtocolId,
    pub result_digest: Digest,
}

#[derive(Clone, Debug)]
pub struct RevokeExecutionRequest {
    pub run_id: ProtocolId,
    pub expected_revision: u64,
    pub expected_state_digest: Digest,
    pub expected_context_binding_digest: Digest,
    pub event_id: ProtocolId,
    pub reason_code: ProtocolId,
}

impl<S, P, C> GraphRouter<S, P, C>
where
    S: RunCommitStore,
    P: ProofResolver,
    C: TrustedClock,
{
    pub async fn start_execution(
        &self,
        request: StartExecutionRequest,
    ) -> Result<ValidatedContextProductStateV1, GraphRouterError> {
        let current = self.load(&request.run_id).await?;
        require_expected_head(
            &current,
            request.expected_revision,
            &request.expected_state_digest,
            &request.expected_context_binding_digest,
        )?;
        let ExecutionAuthority::Granted {
            admission_digest,
            admission_id,
            grant_id,
            grant_digest,
            action_key,
            program_digest,
            stage_id,
            provider_id,
            operation,
            resolved_input_digest,
            policy_digest,
            dependency_digest,
            budget_reservation_id,
            budget_charge_key,
            lease_fence,
            expires_at_ms,
            effect_class,
            provider_idempotency_key,
        } = current.execution()
        else {
            return Err(GraphRouterError::InvalidTransition(
                "execution start requires one active grant",
            ));
        };
        let (started_at_ms, next_revision, next_sequence) = next_clock(&self.clock, &current)?;
        if started_at_ms > *expires_at_ms {
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
            event_id: request.event_id,
            run_id: request.run_id,
            sequence: next_sequence,
            state_revision: current.revision(),
            pre_state_digest: current.state_digest().clone(),
            grant_id: grant_id.clone(),
            grant_digest: grant_digest.clone(),
            attempt_id: attempt_id.clone(),
            attempt_digest: attempt_digest.clone(),
            provider_id: provider_id.clone(),
            operation: operation.clone(),
            lease_fence: *lease_fence,
            expires_at_ms: *expires_at_ms,
            provider_idempotency_key: provider_idempotency_key.clone(),
            event_digest: Digest::from_bytes(b"pending-execution-started"),
        };
        event.event_digest = event.recompute_event_digest();
        let next_event_log_digest =
            chained_event_log_digest(&current.wire().event_log_digest, &event.event_digest);
        let expected = current.head();
        let authority_receipt_ref =
            authority_receipt_id(&expected, next_revision, &next_event_log_digest)?;
        let mut next_state: UncheckedContextProductStateV1 = current.wire().clone();
        next_state.revision = next_revision;
        next_state.previous_state_digest = Some(current.state_digest().clone());
        next_state.last_event_sequence = next_sequence;
        next_state.event_log_digest = next_event_log_digest;
        next_state.execution = ExecutionAuthority::InFlight {
            admission_id: admission_id.clone(),
            admission_digest: admission_digest.clone(),
            grant_id: grant_id.clone(),
            grant_digest: grant_digest.clone(),
            action_key: action_key.clone(),
            attempt_id,
            attempt_digest,
            provider_id: provider_id.clone(),
            operation: operation.clone(),
            program_digest: program_digest.clone(),
            stage_id: stage_id.clone(),
            resolved_input_digest: resolved_input_digest.clone(),
            policy_digest: policy_digest.clone(),
            dependency_digest: dependency_digest.clone(),
            budget_reservation_id: budget_reservation_id.clone(),
            budget_charge_key: budget_charge_key.clone(),
            effect_class: *effect_class,
            lease_fence: *lease_fence,
            expires_at_ms: *expires_at_ms,
            provider_idempotency_key: provider_idempotency_key.clone(),
        };
        next_state.authority_receipt_ref = authority_receipt_ref;
        next_state.state_digest = next_state.recompute_state_digest();
        next_state
            .validate()
            .map_err(GraphRouterError::Validation)?;
        commit_state(
            &self.store,
            expected,
            ContextProductEvent::ExecutionStarted(event),
            next_state,
            started_at_ms,
        )
        .await
    }

    pub async fn consume_execution(
        &self,
        request: ConsumeExecutionRequest,
    ) -> Result<ValidatedContextProductStateV1, GraphRouterError> {
        let current = self.load(&request.run_id).await?;
        require_expected_head(
            &current,
            request.expected_revision,
            &request.expected_state_digest,
            &request.expected_context_binding_digest,
        )?;
        let ExecutionAuthority::InFlight {
            admission_id,
            grant_id,
            grant_digest,
            action_key,
            attempt_id,
            lease_fence,
            expires_at_ms,
            ..
        } = current.execution()
        else {
            return Err(GraphRouterError::InvalidTransition(
                "execution consumption requires one in-flight attempt",
            ));
        };
        let (consumed_at_ms, next_revision, next_sequence) = next_clock(&self.clock, &current)?;
        if consumed_at_ms > *expires_at_ms {
            return Err(GraphRouterError::Validation(
                ValidationError::InvalidExecutionGrant,
            ));
        }
        let mut event = ExecutionConsumed {
            event_type: ExecutionConsumedEventType::ExecutionConsumed,
            event_id: request.event_id,
            run_id: request.run_id,
            sequence: next_sequence,
            state_revision: current.revision(),
            pre_state_digest: current.state_digest().clone(),
            grant_id: grant_id.clone(),
            grant_digest: grant_digest.clone(),
            attempt_id: attempt_id.clone(),
            result_receipt_ref: request.result_receipt_ref.clone(),
            result_digest: request.result_digest.clone(),
            lease_fence: *lease_fence,
            event_digest: Digest::from_bytes(b"pending-execution-consumed"),
        };
        event.event_digest = event.recompute_event_digest();
        let next_event_log_digest =
            chained_event_log_digest(&current.wire().event_log_digest, &event.event_digest);
        let expected = current.head();
        let authority_receipt_ref =
            authority_receipt_id(&expected, next_revision, &next_event_log_digest)?;
        let mut next_state: UncheckedContextProductStateV1 = current.wire().clone();
        next_state.revision = next_revision;
        next_state.previous_state_digest = Some(current.state_digest().clone());
        next_state.last_event_sequence = next_sequence;
        next_state.event_log_digest = next_event_log_digest;
        next_state.execution = ExecutionAuthority::Consumed {
            admission_id: admission_id.clone(),
            grant_id: grant_id.clone(),
            grant_digest: grant_digest.clone(),
            action_key: action_key.clone(),
            attempt_id: attempt_id.clone(),
            result_receipt_ref: request.result_receipt_ref,
            result_digest: request.result_digest,
            lease_fence: *lease_fence,
        };
        next_state.authority_receipt_ref = authority_receipt_ref;
        next_state.state_digest = next_state.recompute_state_digest();
        next_state
            .validate()
            .map_err(GraphRouterError::Validation)?;
        commit_state(
            &self.store,
            expected,
            ContextProductEvent::ExecutionConsumed(event),
            next_state,
            consumed_at_ms,
        )
        .await
    }

    pub async fn revoke_execution(
        &self,
        request: RevokeExecutionRequest,
    ) -> Result<ValidatedContextProductStateV1, GraphRouterError> {
        let current = self.load(&request.run_id).await?;
        require_expected_head(
            &current,
            request.expected_revision,
            &request.expected_state_digest,
            &request.expected_context_binding_digest,
        )?;
        let (admission_id, grant_id, action_key) = revocable_identity(current.execution())?;
        let (revoked_at_ms, next_revision, next_sequence) = next_clock(&self.clock, &current)?;
        let mut event = ExecutionRevoked {
            event_type: ExecutionRevokedEventType::ExecutionRevoked,
            event_id: request.event_id,
            run_id: request.run_id,
            sequence: next_sequence,
            state_revision: current.revision(),
            pre_state_digest: current.state_digest().clone(),
            reason_code: request.reason_code.clone(),
            admission_id: admission_id.clone(),
            grant_id: grant_id.clone(),
            action_key: action_key.clone(),
            event_digest: Digest::from_bytes(b"pending-execution-revoked"),
        };
        event.event_digest = event.recompute_event_digest();
        let next_event_log_digest =
            chained_event_log_digest(&current.wire().event_log_digest, &event.event_digest);
        let expected = current.head();
        let authority_receipt_ref =
            authority_receipt_id(&expected, next_revision, &next_event_log_digest)?;
        let mut next_state: UncheckedContextProductStateV1 = current.wire().clone();
        next_state.revision = next_revision;
        next_state.previous_state_digest = Some(current.state_digest().clone());
        next_state.last_event_sequence = next_sequence;
        next_state.event_log_digest = next_event_log_digest;
        next_state.execution = ExecutionAuthority::Revoked {
            reason_code: request.reason_code,
            admission_id,
            grant_id,
            action_key,
        };
        next_state.authority_receipt_ref = authority_receipt_ref;
        next_state.state_digest = next_state.recompute_state_digest();
        next_state
            .validate()
            .map_err(GraphRouterError::Validation)?;
        commit_state(
            &self.store,
            expected,
            ContextProductEvent::ExecutionRevoked(event),
            next_state,
            revoked_at_ms,
        )
        .await
    }
}

type RevocableIdentity = (Option<ProtocolId>, Option<ProtocolId>, Option<Digest>);

fn revocable_identity(
    execution: &ExecutionAuthority,
) -> Result<RevocableIdentity, GraphRouterError> {
    match execution {
        ExecutionAuthority::Admitted {
            admission_id,
            action_key,
            ..
        } => Ok((Some(admission_id.clone()), None, Some(action_key.clone()))),
        ExecutionAuthority::Granted {
            admission_id,
            grant_id,
            action_key,
            ..
        }
        | ExecutionAuthority::InFlight {
            admission_id,
            grant_id,
            action_key,
            ..
        } => Ok((
            Some(admission_id.clone()),
            Some(grant_id.clone()),
            Some(action_key.clone()),
        )),
        ExecutionAuthority::None
        | ExecutionAuthority::Consumed { .. }
        | ExecutionAuthority::Revoked { .. } => Err(GraphRouterError::InvalidTransition(
            "execution revoke requires admitted, granted, or in-flight authority",
        )),
    }
}
