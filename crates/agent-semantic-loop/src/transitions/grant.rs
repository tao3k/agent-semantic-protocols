use agent_semantic_context_product::{
    ContextProductEvent, Digest, EffectClass, ExecutionAuthority, ExecutionGrantIssued,
    ExecutionGrantIssuedEventType, JSON_SAFE_INTEGER_MAX, ProtocolId,
    UncheckedContextProductStateV1, ValidationError, chained_event_log_digest,
};

use super::transition_core::{
    canonical_digest, commit_state, derived_id, next_clock, require_expected_head,
};
use crate::{
    GraphRouter, GraphRouterError, ProofResolver, RunCommitStore, TrustedClock,
    ValidatedContextProductStateV1, graph_router::authority_receipt_id,
};

#[derive(Clone, Debug)]
pub struct IssueExecutionGrantRequest {
    pub run_id: ProtocolId,
    pub expected_revision: u64,
    pub expected_state_digest: Digest,
    pub expected_context_binding_digest: Digest,
    pub event_id: ProtocolId,
    pub effect_class: EffectClass,
    pub lease_duration_ms: u64,
}

impl<S, P, C> GraphRouter<S, P, C>
where
    S: RunCommitStore,
    P: ProofResolver,
    C: TrustedClock,
{
    pub async fn issue_execution_grant(
        &self,
        request: IssueExecutionGrantRequest,
    ) -> Result<ValidatedContextProductStateV1, GraphRouterError> {
        let current = self.load(&request.run_id).await?;
        require_expected_head(
            &current,
            request.expected_revision,
            &request.expected_state_digest,
            &request.expected_context_binding_digest,
        )?;
        let ExecutionAuthority::Admitted {
            admission_id,
            admission_digest,
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
        } = current.execution()
        else {
            return Err(GraphRouterError::InvalidTransition(
                "execution grant requires one admitted action",
            ));
        };
        if request.lease_duration_ms == 0 {
            return Err(GraphRouterError::Validation(
                ValidationError::InvalidExecutionGrant,
            ));
        }
        let (issued_at_ms, next_revision, next_sequence) = next_clock(&self.clock, &current)?;
        let expires_at_ms = issued_at_ms
            .checked_add(request.lease_duration_ms)
            .filter(|value| *value <= JSON_SAFE_INTEGER_MAX)
            .ok_or(GraphRouterError::Validation(
                ValidationError::InvalidExecutionGrant,
            ))?;
        let lease_fence = next_sequence;
        let grant_seed = canonical_digest(&serde_json::json!({
            "actionKey": action_key,
            "admissionDigest": admission_digest,
            "expiresAtMs": expires_at_ms,
            "leaseFence": lease_fence,
        }));
        let grant_id = derived_id("execution-grant", &grant_seed)?;
        let provider_idempotency_key = derived_id("provider-idempotency", &grant_seed)?;
        let program_id = match current.active_program() {
            agent_semantic_context_product::ActiveProgram::Admitted { program_id, .. } => {
                program_id.clone()
            }
            agent_semantic_context_product::ActiveProgram::None => {
                return Err(GraphRouterError::InvalidTransition(
                    "execution grant requires an active route program",
                ));
            }
        };
        let mut event = ExecutionGrantIssued {
            event_type: ExecutionGrantIssuedEventType::ExecutionGrantIssued,
            event_id: request.event_id,
            run_id: request.run_id,
            sequence: next_sequence,
            state_revision: current.revision(),
            pre_state_digest: current.state_digest().clone(),
            grant_id: grant_id.clone(),
            admission_id: admission_id.clone(),
            admission_digest: admission_digest.clone(),
            program_id,
            program_digest: program_digest.clone(),
            stage_id: stage_id.clone(),
            action_key: action_key.clone(),
            context_binding_digest: current.context_binding_digest().clone(),
            resolved_input_digest: resolved_input_digest.clone(),
            provider_id: provider_id.clone(),
            operation: operation.clone(),
            lease_fence,
            issued_at_ms,
            expires_at_ms,
            effect_class: request.effect_class,
            single_use: true,
            provider_idempotency_key: Some(provider_idempotency_key.clone()),
            grant_digest: Digest::from_bytes(b"pending-execution-grant"),
        };
        event.grant_digest = event.recompute_grant_digest();
        let next_event_log_digest =
            chained_event_log_digest(&current.wire().event_log_digest, &event.grant_digest);
        let expected = current.head();
        let authority_receipt_ref =
            authority_receipt_id(&expected, next_revision, &next_event_log_digest)?;
        let mut next_state: UncheckedContextProductStateV1 = current.wire().clone();
        next_state.revision = next_revision;
        next_state.previous_state_digest = Some(current.state_digest().clone());
        next_state.last_event_sequence = next_sequence;
        next_state.event_log_digest = next_event_log_digest;
        next_state.execution = ExecutionAuthority::Granted {
            admission_digest: admission_digest.clone(),
            admission_id: admission_id.clone(),
            grant_id,
            grant_digest: event.grant_digest.clone(),
            action_key: action_key.clone(),
            program_digest: program_digest.clone(),
            stage_id: stage_id.clone(),
            provider_id: provider_id.clone(),
            operation: operation.clone(),
            resolved_input_digest: resolved_input_digest.clone(),
            policy_digest: policy_digest.clone(),
            dependency_digest: dependency_digest.clone(),
            budget_reservation_id: budget_reservation_id.clone(),
            budget_charge_key: budget_charge_key.clone(),
            lease_fence,
            expires_at_ms,
            effect_class: request.effect_class,
            provider_idempotency_key,
        };
        next_state.authority_receipt_ref = authority_receipt_ref;
        next_state.state_digest = next_state.recompute_state_digest();
        next_state
            .validate()
            .map_err(GraphRouterError::Validation)?;
        commit_state(
            &self.store,
            expected,
            ContextProductEvent::ExecutionGrantIssued(event),
            next_state,
            issued_at_ms,
        )
        .await
    }
}
