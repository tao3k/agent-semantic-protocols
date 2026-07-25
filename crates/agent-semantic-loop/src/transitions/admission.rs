use agent_semantic_context_product::{
    ActionAdmitted, ActionAdmittedEventType, ActiveProgram, ContextProductEvent, Digest,
    ExecutionAuthority, ProtocolId, UncheckedContextProductStateV1, ValidationError,
    chained_event_log_digest,
};
use serde::Serialize;

use super::transition_core::{
    canonical_digest, commit_state, derived_id, next_clock, require_expected_head,
};
use crate::{
    GraphRouter, GraphRouterError, ProofResolver, RunCommitStore, TrustedClock,
    ValidatedContextProductStateV1, graph_router::authority_receipt_id,
};

#[derive(Clone, Debug)]
pub struct AdmitActionRequest {
    pub run_id: ProtocolId,
    pub expected_revision: u64,
    pub expected_state_digest: Digest,
    pub expected_context_binding_digest: Digest,
    pub event_id: ProtocolId,
    pub stage_id: ProtocolId,
    pub input_facts_digest: Digest,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ResolvedInputProjection<'a> {
    input_template_digest: &'a Digest,
    input_facts_digest: &'a Digest,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct DependencyProjection<'a> {
    stage_id: &'a ProtocolId,
    incoming_stage_ids: Vec<&'a ProtocolId>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ActionProjection<'a> {
    run_id: &'a ProtocolId,
    program_digest: &'a Digest,
    stage_id: &'a ProtocolId,
    context_binding_digest: &'a Digest,
    resolved_input_digest: &'a Digest,
    policy_digest: &'a Digest,
    dependency_digest: &'a Digest,
    budget_reservation_id: &'a ProtocolId,
    budget_charge_key: &'a Digest,
}

impl<S, P, C> GraphRouter<S, P, C>
where
    S: RunCommitStore,
    P: ProofResolver,
    C: TrustedClock,
{
    pub async fn admit_action(
        &self,
        request: AdmitActionRequest,
    ) -> Result<ValidatedContextProductStateV1, GraphRouterError> {
        let current = self.load(&request.run_id).await?;
        require_expected_head(
            &current,
            request.expected_revision,
            &request.expected_state_digest,
            &request.expected_context_binding_digest,
        )?;
        if !matches!(
            current.execution(),
            ExecutionAuthority::None
                | ExecutionAuthority::Consumed { .. }
                | ExecutionAuthority::Revoked { .. }
        ) {
            return Err(GraphRouterError::InvalidTransition(
                "action admission requires no execution authority",
            ));
        }
        let ActiveProgram::Admitted {
            program_id,
            program_digest,
            program,
            ..
        } = current.active_program()
        else {
            return Err(GraphRouterError::InvalidTransition(
                "action admission requires an admitted route program",
            ));
        };
        let stage = program
            .stages
            .iter()
            .find(|stage| stage.stage_id == request.stage_id)
            .ok_or_else(|| {
                GraphRouterError::Validation(ValidationError::MissingReference {
                    kind: "route stage",
                    id: request.stage_id.clone(),
                })
            })?;

        let resolved_input_digest = canonical_digest(&ResolvedInputProjection {
            input_template_digest: &stage.input_template_digest,
            input_facts_digest: &request.input_facts_digest,
        });
        let policy_digest = canonical_digest(&stage.evidence_predicate);
        let mut incoming_stage_ids = program
            .edges
            .iter()
            .filter(|edge| edge.to == stage.stage_id)
            .map(|edge| &edge.from)
            .collect::<Vec<_>>();
        incoming_stage_ids.sort();
        let dependency_digest = canonical_digest(&DependencyProjection {
            stage_id: &stage.stage_id,
            incoming_stage_ids,
        });
        let budget_seed = canonical_digest(&serde_json::json!({
            "programDigest": program_digest,
            "runId": request.run_id,
            "stageId": stage.stage_id,
        }));
        let budget_reservation_id = derived_id("budget-reservation", &budget_seed)?;
        let budget_charge_key = canonical_digest(&serde_json::json!({
            "budgetReservationId": budget_reservation_id,
            "stageId": stage.stage_id,
        }));
        let action_key = canonical_digest(&ActionProjection {
            run_id: &request.run_id,
            program_digest,
            stage_id: &stage.stage_id,
            context_binding_digest: current.context_binding_digest(),
            resolved_input_digest: &resolved_input_digest,
            policy_digest: &policy_digest,
            dependency_digest: &dependency_digest,
            budget_reservation_id: &budget_reservation_id,
            budget_charge_key: &budget_charge_key,
        });
        if current
            .wire()
            .spent_action_keys
            .binary_search(&action_key)
            .is_ok()
        {
            return Err(GraphRouterError::Validation(
                ValidationError::DuplicateActionIdentity,
            ));
        }
        let admission_id = derived_id("action-admission", &action_key)?;
        let (committed_at_ms, next_revision, next_sequence) = next_clock(&self.clock, &current)?;
        let mut event = ActionAdmitted {
            event_type: ActionAdmittedEventType::ActionAdmitted,
            event_id: request.event_id,
            run_id: request.run_id,
            sequence: next_sequence,
            state_revision: current.revision(),
            pre_state_digest: current.state_digest().clone(),
            admission_id: admission_id.clone(),
            program_id: program_id.clone(),
            stage_id: stage.stage_id.clone(),
            context_binding_digest: current.context_binding_digest().clone(),
            resolved_input_digest: resolved_input_digest.clone(),
            action_key: action_key.clone(),
            policy_digest: policy_digest.clone(),
            dependency_digest: dependency_digest.clone(),
            budget_reservation_id: budget_reservation_id.clone(),
            budget_charge_key: budget_charge_key.clone(),
            admission_digest: Digest::from_bytes(b"pending-action-admission"),
        };
        event.admission_digest = event.recompute_admission_digest();
        let next_event_log_digest =
            chained_event_log_digest(&current.wire().event_log_digest, &event.admission_digest);
        let expected = current.head();
        let authority_receipt_ref =
            authority_receipt_id(&expected, next_revision, &next_event_log_digest)?;
        let mut next_state: UncheckedContextProductStateV1 = current.wire().clone();
        next_state.revision = next_revision;
        next_state.previous_state_digest = Some(current.state_digest().clone());
        next_state.last_event_sequence = next_sequence;
        next_state.event_log_digest = next_event_log_digest;
        next_state.spent_action_keys.push(action_key.clone());
        next_state.spent_action_keys.sort();
        next_state.spent_action_ledger_digest = next_state.recompute_spent_action_ledger_digest();
        next_state.execution = ExecutionAuthority::Admitted {
            admission_id,
            admission_digest: event.admission_digest.clone(),
            action_key,
            program_digest: program_digest.clone(),
            stage_id: stage.stage_id.clone(),
            provider_id: stage.provider_id.clone(),
            operation: stage.operation.clone(),
            resolved_input_digest,
            policy_digest,
            dependency_digest,
            budget_reservation_id,
            budget_charge_key,
        };
        next_state.authority_receipt_ref = authority_receipt_ref;
        next_state.state_digest = next_state.recompute_state_digest();
        next_state
            .validate()
            .map_err(GraphRouterError::Validation)?;
        commit_state(
            &self.store,
            expected,
            ContextProductEvent::ActionAdmitted(event),
            next_state,
            committed_at_ms,
        )
        .await
    }
}
