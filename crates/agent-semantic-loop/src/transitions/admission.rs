// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

use std::collections::BTreeSet;

use agent_semantic_context_product::ActionAdmitted;
use agent_semantic_context_product::ActionAdmittedEventType;
use agent_semantic_context_product::ActiveProgram;
use agent_semantic_context_product::ContextProductEvent;
use agent_semantic_context_product::Digest;
use agent_semantic_context_product::ExecutionAuthority;
use agent_semantic_context_product::JSON_SAFE_INTEGER_MAX;
use agent_semantic_context_product::ProtocolId;
use agent_semantic_context_product::RouteExecutionMode;
use agent_semantic_context_product::UncheckedContextProductStateV1;
use agent_semantic_context_product::ValidationError;
use agent_semantic_context_product::chained_event_log_digest;
use serde::Serialize;

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
use crate::search_loop::SearchLoopDirective;

#[derive(Clone, Debug)]
pub struct ExecutionDispatchAdmission {
    pub event_id: ProtocolId,
    pub stage_ids: Vec<ProtocolId>,
    pub input_facts_digest: Digest,
}

#[derive(Clone, Debug)]
struct AdmitExecutionGroupRequest {
    run_id: ProtocolId,
    expected_revision: u64,
    expected_state_digest: Digest,
    expected_context_binding_digest: Digest,
    execution_group_id: ProtocolId,
    dispatches: Vec<ExecutionDispatchAdmission>,
}

#[derive(Clone, Debug)]
pub struct AdmitSearchLoopDirectiveRequest {
    pub run_id: ProtocolId,
    pub expected_revision: u64,
    pub expected_state_digest: Digest,
    pub expected_context_binding_digest: Digest,
    pub directive: SearchLoopDirective,
    pub dispatches: Vec<ExecutionDispatchAdmission>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ResolvedStageInputProjection<'a> {
    stage_id: &'a ProtocolId,
    input_template_digest: &'a Digest,
    input_facts_digest: &'a Digest,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct DependencyProjection<'a> {
    stage_ids: &'a [ProtocolId],
    incoming_stage_ids: Vec<&'a ProtocolId>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ActionProjection<'a> {
    run_id: &'a ProtocolId,
    program_digest: &'a Digest,
    execution_group_id: &'a ProtocolId,
    stage_ids: &'a [ProtocolId],
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
    pub async fn admit_search_loop_directive(
        &self,
        request: AdmitSearchLoopDirectiveRequest,
    ) -> Result<ValidatedContextProductStateV1, GraphRouterError> {
        let execution_group_id =
            validate_directive_dispatches(&request.directive, &request.dispatches)?;
        self.admit_execution_group(AdmitExecutionGroupRequest {
            run_id: request.run_id,
            expected_revision: request.expected_revision,
            expected_state_digest: request.expected_state_digest,
            expected_context_binding_digest: request.expected_context_binding_digest,
            execution_group_id,
            dispatches: request.dispatches,
        })
        .await
    }

    async fn admit_execution_group(
        &self,
        request: AdmitExecutionGroupRequest,
    ) -> Result<ValidatedContextProductStateV1, GraphRouterError> {
        let current = self.load(&request.run_id).await?;
        require_expected_head(
            &current,
            request.expected_revision,
            &request.expected_state_digest,
            &request.expected_context_binding_digest,
        )?;
        let ActiveProgram::Admitted {
            program_id,
            program_digest,
            program,
            ..
        } = current.active_program()
        else {
            return Err(GraphRouterError::InvalidTransition(
                "execution-group admission requires an admitted route program",
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
        validate_dispatch_shape(group.mode, &group.stage_ids, &request.dispatches)?;

        let dispatch_stage_ids = request
            .dispatches
            .iter()
            .flat_map(|dispatch| dispatch.stage_ids.iter())
            .collect::<BTreeSet<_>>();
        for execution in current.executions() {
            if execution
                .stage_ids()
                .iter()
                .any(|stage_id| dispatch_stage_ids.contains(stage_id))
                && !matches!(execution, ExecutionAuthority::Revoked(_))
            {
                return Err(GraphRouterError::InvalidTransition(
                    "execution-group admission requires pending or revoked stages",
                ));
            }
        }

        let (committed_at_ms, next_revision, first_sequence) = next_clock(&self.clock, &current)?;
        let expected = current.head();
        let mut next_state: UncheckedContextProductStateV1 = current.wire().clone();
        let mut next_event_log_digest = current.wire().event_log_digest.clone();
        let mut events = Vec::with_capacity(request.dispatches.len());

        for (offset, dispatch) in request.dispatches.iter().enumerate() {
            let sequence = first_sequence
                .checked_add(offset as u64)
                .filter(|value| *value <= JSON_SAFE_INTEGER_MAX)
                .ok_or(GraphRouterError::Validation(
                    ValidationError::RevisionExhausted,
                ))?;
            let stages = dispatch
                .stage_ids
                .iter()
                .map(|stage_id| {
                    program
                        .stages
                        .iter()
                        .find(|stage| &stage.stage_id == stage_id)
                        .ok_or_else(|| {
                            GraphRouterError::Validation(ValidationError::MissingReference {
                                kind: "route stage",
                                id: stage_id.clone(),
                            })
                        })
                })
                .collect::<Result<Vec<_>, _>>()?;
            let provider_id = stages[0].provider_id.clone();
            let operation = stages[0].catalog_id.clone();
            if stages
                .iter()
                .any(|stage| stage.provider_id != provider_id || stage.catalog_id != operation)
            {
                return Err(GraphRouterError::InvalidTransition(
                    "one provider dispatch must use one provider and catalog operation",
                ));
            }
            let dependencies_ready = program
                .edges
                .iter()
                .filter(|edge| dispatch.stage_ids.contains(&edge.to))
                .filter(|edge| !dispatch.stage_ids.contains(&edge.from))
                .all(|edge| {
                    current.executions().iter().any(|execution| {
                        execution.stage_ids().contains(&edge.from)
                            && matches!(execution, ExecutionAuthority::Consumed(_))
                    })
                });
            if !dependencies_ready {
                return Err(GraphRouterError::InvalidTransition(
                    "execution-group admission requires all external graph predecessors consumed",
                ));
            }

            let resolved_inputs = stages
                .iter()
                .map(|stage| ResolvedStageInputProjection {
                    stage_id: &stage.stage_id,
                    input_template_digest: &stage.input_template_digest,
                    input_facts_digest: &dispatch.input_facts_digest,
                })
                .collect::<Vec<_>>();
            let resolved_input_digest = canonical_digest(&resolved_inputs);
            let policy_digest = canonical_digest(
                &stages
                    .iter()
                    .map(|stage| &stage.evidence_predicate)
                    .collect::<Vec<_>>(),
            );
            let mut incoming_stage_ids = program
                .edges
                .iter()
                .filter(|edge| dispatch.stage_ids.contains(&edge.to))
                .map(|edge| &edge.from)
                .collect::<Vec<_>>();
            incoming_stage_ids.sort();
            incoming_stage_ids.dedup();
            let dependency_digest = canonical_digest(&DependencyProjection {
                stage_ids: &dispatch.stage_ids,
                incoming_stage_ids,
            });
            let budget_seed = canonical_digest(&serde_json::json!({
                "programDigest": program_digest,
                "runId": request.run_id,
                "executionGroupId": group.group_id,
                "stageIds": dispatch.stage_ids,
            }));
            let budget_reservation_id = derived_id("budget-reservation", &budget_seed)?;
            let budget_charge_key = canonical_digest(&serde_json::json!({
                "budgetReservationId": budget_reservation_id,
                "executionGroupId": group.group_id,
                "stageIds": dispatch.stage_ids,
            }));
            let action_key = canonical_digest(&ActionProjection {
                run_id: &request.run_id,
                program_digest,
                execution_group_id: &group.group_id,
                stage_ids: &dispatch.stage_ids,
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
                || next_state
                    .spent_action_keys
                    .binary_search(&action_key)
                    .is_ok()
            {
                return Err(GraphRouterError::Validation(
                    ValidationError::DuplicateActionIdentity,
                ));
            }
            let admission_id = derived_id("action-admission", &action_key)?;
            let mut event = ActionAdmitted {
                event_type: ActionAdmittedEventType::ActionAdmitted,
                event_id: dispatch.event_id.clone(),
                run_id: request.run_id.clone(),
                sequence,
                state_revision: current.revision(),
                pre_state_digest: current.state_digest().clone(),
                admission_id: admission_id.clone(),
                program_id: program_id.clone(),
                execution_group_id: group.group_id.clone(),
                stage_ids: dispatch.stage_ids.clone(),
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
            next_event_log_digest =
                chained_event_log_digest(&next_event_log_digest, &event.admission_digest);
            next_state.spent_action_keys.push(action_key.clone());
            next_state.spent_action_keys.sort();
            let next_execution = ExecutionAuthority::admitted(
                &event,
                program_digest.clone(),
                provider_id,
                operation,
            );
            if let Some(index) = next_state.executions.iter().position(|execution| {
                matches!(execution, ExecutionAuthority::Revoked(_))
                    && execution.stage_ids() == dispatch.stage_ids
            }) {
                next_state.executions[index] = next_execution;
            } else {
                next_state.executions.push(next_execution);
            }
            events.push(ContextProductEvent::ActionAdmitted(event));
        }

        next_state.revision = next_revision;
        next_state.previous_state_digest = Some(current.state_digest().clone());
        next_state.last_event_sequence = first_sequence + events.len() as u64 - 1;
        next_state.event_log_digest = next_event_log_digest;
        next_state.spent_action_ledger_digest = next_state.recompute_spent_action_ledger_digest();
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
            committed_at_ms,
        )
        .await
    }
}

fn validate_directive_dispatches(
    directive: &SearchLoopDirective,
    dispatches: &[ExecutionDispatchAdmission],
) -> Result<ProtocolId, GraphRouterError> {
    let (group_id, expected_stage_ids, expected_dispatch_count) = match directive {
        SearchLoopDirective::AdmitSerial { group_id, stage_id } => (group_id, vec![stage_id], 1),
        SearchLoopDirective::AdmitBatch {
            group_id,
            stage_ids,
            ..
        } => (group_id, stage_ids.iter().collect(), 1),
        SearchLoopDirective::AdmitParallel {
            group_id,
            stage_ids,
            max_parallel,
            ..
        } => {
            if stage_ids.len() as u64 > *max_parallel {
                return Err(GraphRouterError::InvalidTransition(
                    "parallel directive exceeds its admitted concurrency ceiling",
                ));
            }
            (group_id, stage_ids.iter().collect(), stage_ids.len())
        }
        _ => {
            return Err(GraphRouterError::InvalidTransition(
                "search-loop directive does not admit provider dispatches",
            ));
        }
    };
    if dispatches.len() != expected_dispatch_count {
        return Err(GraphRouterError::InvalidTransition(
            "provider dispatch count does not match the search-loop directive",
        ));
    }
    let mut actual_stage_ids = dispatches
        .iter()
        .flat_map(|dispatch| dispatch.stage_ids.iter())
        .collect::<Vec<_>>();
    actual_stage_ids.sort();
    let mut expected_stage_ids = expected_stage_ids;
    expected_stage_ids.sort();
    if actual_stage_ids != expected_stage_ids {
        return Err(GraphRouterError::InvalidTransition(
            "provider dispatch stage set does not match the search-loop directive",
        ));
    }
    Ok(group_id.clone())
}

fn validate_dispatch_shape(
    mode: RouteExecutionMode,
    group_stage_ids: &[ProtocolId],
    dispatches: &[ExecutionDispatchAdmission],
) -> Result<(), GraphRouterError> {
    if dispatches.is_empty() {
        return Err(GraphRouterError::InvalidTransition(
            "execution-group admission requires at least one provider dispatch",
        ));
    }
    let mut seen = BTreeSet::new();
    if dispatches.iter().any(|dispatch| {
        dispatch.stage_ids.is_empty()
            || dispatch
                .stage_ids
                .iter()
                .any(|stage_id| !group_stage_ids.contains(stage_id) || !seen.insert(stage_id))
    }) {
        return Err(GraphRouterError::InvalidTransition(
            "execution-group dispatches must uniquely cover group stages",
        ));
    }
    match mode {
        RouteExecutionMode::Serial
            if dispatches.len() != 1 || dispatches[0].stage_ids.len() != 1 =>
        {
            Err(GraphRouterError::InvalidTransition(
                "serial admission requires one single-stage dispatch",
            ))
        }
        RouteExecutionMode::Batch
            if dispatches.len() != 1 || dispatches[0].stage_ids != group_stage_ids =>
        {
            Err(GraphRouterError::InvalidTransition(
                "batch admission requires one dispatch covering the entire group",
            ))
        }
        RouteExecutionMode::Parallel
            if dispatches
                .iter()
                .any(|dispatch| dispatch.stage_ids.len() != 1) =>
        {
            Err(GraphRouterError::InvalidTransition(
                "parallel admission requires one dispatch per selected ready stage",
            ))
        }
        _ => Ok(()),
    }
}
