use std::collections::BTreeSet;

use agent_semantic_context_product::{
    ActiveProgram, ContextProductEvent, Digest, EffectClass, ExecutionAuthority,
    ExecutionGrantIssued, ExecutionGrantIssuedEventType, JSON_SAFE_INTEGER_MAX, ProtocolId,
    UncheckedContextProductStateV1, ValidationError, chained_event_log_digest,
};

use super::transition_core::{
    canonical_digest, commit_events, derived_id, next_clock, require_expected_head,
};
use crate::{
    GraphRouter, GraphRouterError, ProofResolver, RunCommitStore, TrustedClock,
    ValidatedContextProductStateV1, graph_router::authority_receipt_id,
};

#[derive(Clone, Debug)]
pub struct ExecutionGrantSpec {
    pub event_id: ProtocolId,
    pub admission_id: ProtocolId,
    pub effect_class: EffectClass,
    pub lease_duration_ms: u64,
}

#[derive(Clone, Debug)]
pub struct IssueExecutionGroupGrantsRequest {
    pub run_id: ProtocolId,
    pub expected_revision: u64,
    pub expected_state_digest: Digest,
    pub expected_context_binding_digest: Digest,
    pub execution_group_id: ProtocolId,
    pub grants: Vec<ExecutionGrantSpec>,
}

impl<S, P, C> GraphRouter<S, P, C>
where
    S: RunCommitStore,
    P: ProofResolver,
    C: TrustedClock,
{
    pub async fn issue_execution_group_grants(
        &self,
        request: IssueExecutionGroupGrantsRequest,
    ) -> Result<ValidatedContextProductStateV1, GraphRouterError> {
        let current = self.load(&request.run_id).await?;
        require_expected_head(
            &current,
            request.expected_revision,
            &request.expected_state_digest,
            &request.expected_context_binding_digest,
        )?;
        if request.grants.is_empty() {
            return Err(GraphRouterError::InvalidTransition(
                "execution-group grant requires at least one admitted dispatch",
            ));
        }
        let ActiveProgram::Admitted {
            program_id,
            program_digest: active_program_digest,
            program,
            ..
        } = current.active_program()
        else {
            return Err(GraphRouterError::InvalidTransition(
                "execution-group grant requires an active route program",
            ));
        };
        let group = program
            .execution_groups
            .iter()
            .find(|group| group.group_id == request.execution_group_id)
            .ok_or_else(|| {
                GraphRouterError::Validation(ValidationError::MissingReference {
                    kind: "route execution group",
                    id: request.execution_group_id.clone(),
                })
            })?;
        let mut admission_ids = BTreeSet::new();
        let authority_indices = request
            .grants
            .iter()
            .map(|grant| {
                if grant.lease_duration_ms == 0 || !admission_ids.insert(&grant.admission_id) {
                    return Err(GraphRouterError::Validation(
                        ValidationError::InvalidExecutionGrant,
                    ));
                }
                current
                    .executions()
                    .iter()
                    .position(|execution| {
                        execution.admission_id() == Some(&grant.admission_id)
                            && execution.execution_group_id() == &group.group_id
                            && matches!(execution, ExecutionAuthority::Admitted(_))
                    })
                    .ok_or(GraphRouterError::InvalidTransition(
                        "execution-group grant requires matching admitted dispatches",
                    ))
            })
            .collect::<Result<Vec<_>, _>>()?;
        let admitted_in_group = current
            .executions()
            .iter()
            .filter(|execution| {
                execution.execution_group_id() == &group.group_id
                    && matches!(execution, ExecutionAuthority::Admitted(_))
            })
            .count();
        if authority_indices.len() != admitted_in_group {
            return Err(GraphRouterError::InvalidTransition(
                "execution-group grants must cover every admitted dispatch atomically",
            ));
        }

        let (issued_at_ms, next_revision, first_sequence) = next_clock(&self.clock, &current)?;
        let expected = current.head();
        let mut next_state: UncheckedContextProductStateV1 = current.wire().clone();
        let mut next_event_log_digest = current.wire().event_log_digest.clone();
        let mut events = Vec::with_capacity(request.grants.len());

        for (offset, (grant_spec, authority_index)) in
            request.grants.iter().zip(authority_indices).enumerate()
        {
            let sequence = first_sequence
                .checked_add(offset as u64)
                .filter(|value| *value <= JSON_SAFE_INTEGER_MAX)
                .ok_or(GraphRouterError::Validation(
                    ValidationError::RevisionExhausted,
                ))?;
            let authority = &current.executions()[authority_index];
            if !matches!(authority, ExecutionAuthority::Admitted(_)) {
                unreachable!("authority indices were selected from admitted executions");
            }
            let admission_id = authority
                .admission_id()
                .expect("admitted authority has admission id");
            let admission_digest = authority
                .admission_digest()
                .expect("admitted authority has admission digest");
            let action_key = authority
                .action_key()
                .expect("admitted authority has action key");
            let program_digest = authority.program_digest();
            let execution_group_id = authority.execution_group_id();
            let stage_ids = authority.stage_ids();
            let provider_id = authority.provider_id();
            let operation = authority.operation();
            let resolved_input_digest = authority
                .resolved_input_digest()
                .expect("admitted authority has resolved input digest");
            if program_digest != active_program_digest {
                return Err(GraphRouterError::Validation(
                    ValidationError::InvalidExecutionGrant,
                ));
            }
            let expires_at_ms = issued_at_ms
                .checked_add(grant_spec.lease_duration_ms)
                .filter(|value| *value <= JSON_SAFE_INTEGER_MAX)
                .ok_or(GraphRouterError::Validation(
                    ValidationError::InvalidExecutionGrant,
                ))?;
            let lease_fence = sequence;
            let grant_seed = canonical_digest(&serde_json::json!({
                "actionKey": action_key,
                "admissionDigest": admission_digest,
                "expiresAtMs": expires_at_ms,
                "leaseFence": lease_fence,
            }));
            let grant_id = derived_id("execution-grant", &grant_seed)?;
            let provider_idempotency_key = derived_id("provider-idempotency", &grant_seed)?;
            let mut event = ExecutionGrantIssued {
                event_type: ExecutionGrantIssuedEventType::ExecutionGrantIssued,
                event_id: grant_spec.event_id.clone(),
                run_id: request.run_id.clone(),
                sequence,
                state_revision: current.revision(),
                pre_state_digest: current.state_digest().clone(),
                grant_id: grant_id.clone(),
                admission_id: admission_id.clone(),
                admission_digest: admission_digest.clone(),
                program_id: program_id.clone(),
                program_digest: program_digest.clone(),
                execution_group_id: execution_group_id.clone(),
                stage_ids: stage_ids.to_vec(),
                action_key: action_key.clone(),
                context_binding_digest: current.context_binding_digest().clone(),
                resolved_input_digest: resolved_input_digest.clone(),
                provider_id: provider_id.clone(),
                operation: operation.clone(),
                lease_fence,
                issued_at_ms,
                expires_at_ms,
                effect_class: grant_spec.effect_class,
                single_use: true,
                provider_idempotency_key: Some(provider_idempotency_key.clone()),
                grant_digest: Digest::from_bytes(b"pending-execution-grant"),
            };
            event.grant_digest = event.recompute_grant_digest();
            next_event_log_digest =
                chained_event_log_digest(&next_event_log_digest, &event.grant_digest);
            next_state.executions[authority_index] =
                ExecutionAuthority::granted(authority, &event, provider_idempotency_key)
                    .expect("selected authority is admitted");
            events.push(ContextProductEvent::ExecutionGrantIssued(event));
        }

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
            &self.store,
            expected,
            events,
            next_state,
            current.search_loop_capabilities().to_vec(),
            current.search_loop_runtime().cloned(),
            issued_at_ms,
        )
        .await
    }
}
