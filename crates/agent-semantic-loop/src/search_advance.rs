// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

use std::collections::BTreeMap;
use std::collections::BTreeSet;

use agent_semantic_context_product::ActiveProgram;
use agent_semantic_context_product::Digest;
use agent_semantic_context_product::EffectClass;
use agent_semantic_context_product::ExecutionAuthority;
use agent_semantic_context_product::ProtocolId;
use agent_semantic_context_product::ValidationError;

use crate::AdmitSearchLoopDirectiveRequest;
use crate::ConsumeExecutionGroupRequest;
use crate::ExecutionConsumptionSpec;
use crate::ExecutionDispatchAdmission;
use crate::ExecutionGrantSpec;
use crate::ExecutionStartSpec;
use crate::GraphRouter;
use crate::GraphRouterError;
use crate::IssueExecutionGroupGrantsRequest;
use crate::JoinExecutionGroupRequest;
use crate::ProofResolver;
use crate::ProviderExecutionDispatch;
use crate::RunCommitStore;
use crate::SearchExecutionDriver;
use crate::StartExecutionGroupRequest;
use crate::TrustedClock;
use crate::ValidatedContextProductStateV1;
use crate::search_loop::SearchLoopDirective;

#[derive(Clone, Debug)]
pub struct SearchLoopAdvanceDispatch {
    pub stage_ids: Vec<ProtocolId>,
    pub input_facts_digest: Digest,
    pub effect_class: EffectClass,
    pub lease_duration_ms: u64,
    pub admission_event_id: ProtocolId,
    pub grant_event_id: ProtocolId,
    pub start_event_id: ProtocolId,
    pub consume_event_id: ProtocolId,
}

#[derive(Clone, Debug)]
pub struct SearchLoopAdvanceRequest {
    pub run_id: ProtocolId,
    pub expected_revision: u64,
    pub expected_state_digest: Digest,
    pub expected_context_binding_digest: Digest,
    pub directive: SearchLoopDirective,
    pub dispatches: Vec<SearchLoopAdvanceDispatch>,
    pub join_event_id: ProtocolId,
}

#[derive(Clone, Debug)]
pub struct SearchLoopPollDispatch {
    pub stage_ids: Vec<ProtocolId>,
    pub consume_event_id: ProtocolId,
}

#[derive(Clone, Debug)]
pub struct SearchLoopPollRequest {
    pub run_id: ProtocolId,
    pub expected_revision: u64,
    pub expected_state_digest: Digest,
    pub expected_context_binding_digest: Digest,
    pub execution_group_id: ProtocolId,
    pub dispatches: Vec<SearchLoopPollDispatch>,
    pub join_event_id: ProtocolId,
}

impl<S, P, C> GraphRouter<S, P, C>
where
    S: RunCommitStore,
    P: ProofResolver,
    C: TrustedClock,
{
    pub async fn advance_search_loop<D: SearchExecutionDriver>(
        &self,
        request: SearchLoopAdvanceRequest,
        driver: &D,
    ) -> Result<ValidatedContextProductStateV1, GraphRouterError> {
        let execution_group_id = directive_group_id(&request.directive)?.clone();
        let dispatch_by_stage_set = dispatch_material_by_stage_set(&request.dispatches)?;
        let admitted = self
            .admit_search_loop_directive(AdmitSearchLoopDirectiveRequest {
                run_id: request.run_id,
                expected_revision: request.expected_revision,
                expected_state_digest: request.expected_state_digest,
                expected_context_binding_digest: request.expected_context_binding_digest,
                directive: request.directive,
                dispatches: request
                    .dispatches
                    .iter()
                    .map(|dispatch| ExecutionDispatchAdmission {
                        event_id: dispatch.admission_event_id.clone(),
                        stage_ids: dispatch.stage_ids.clone(),
                        input_facts_digest: dispatch.input_facts_digest.clone(),
                    })
                    .collect(),
            })
            .await?;
        let admitted_authorities = group_authorities(&admitted, &execution_group_id, |execution| {
            matches!(execution, ExecutionAuthority::Admitted(_))
        });
        ensure_phase_cardinality(admitted_authorities.len(), request.dispatches.len())?;

        let granted = self
            .issue_execution_group_grants(IssueExecutionGroupGrantsRequest {
                run_id: admitted.wire().run_id.clone(),
                expected_revision: admitted.revision(),
                expected_state_digest: admitted.state_digest().clone(),
                expected_context_binding_digest: admitted.context_binding_digest().clone(),
                execution_group_id: execution_group_id.clone(),
                grants: admitted_authorities
                    .iter()
                    .map(|authority| {
                        let material = dispatch_material(authority, &dispatch_by_stage_set)?;
                        Ok(ExecutionGrantSpec {
                            event_id: material.grant_event_id.clone(),
                            admission_id: authority
                                .admission_id()
                                .expect("admitted authority has admission id")
                                .clone(),
                            effect_class: material.effect_class,
                            lease_duration_ms: material.lease_duration_ms,
                        })
                    })
                    .collect::<Result<Vec<_>, GraphRouterError>>()?,
            })
            .await?;
        let granted_authorities = group_authorities(&granted, &execution_group_id, |execution| {
            matches!(execution, ExecutionAuthority::Granted(_))
        });
        ensure_phase_cardinality(granted_authorities.len(), request.dispatches.len())?;

        let started = self
            .start_execution_group(StartExecutionGroupRequest {
                run_id: granted.wire().run_id.clone(),
                expected_revision: granted.revision(),
                expected_state_digest: granted.state_digest().clone(),
                expected_context_binding_digest: granted.context_binding_digest().clone(),
                execution_group_id: execution_group_id.clone(),
                starts: granted_authorities
                    .iter()
                    .map(|authority| {
                        let material = dispatch_material(authority, &dispatch_by_stage_set)?;
                        Ok(ExecutionStartSpec {
                            event_id: material.start_event_id.clone(),
                            grant_id: authority
                                .grant_id()
                                .expect("granted authority has grant id")
                                .clone(),
                        })
                    })
                    .collect::<Result<Vec<_>, GraphRouterError>>()?,
            })
            .await?;
        let in_flight = group_authorities(&started, &execution_group_id, |execution| {
            matches!(execution, ExecutionAuthority::InFlight(_))
                && dispatch_by_stage_set.contains_key(&canonical_stage_set(execution.stage_ids()))
        });
        ensure_phase_cardinality(in_flight.len(), request.dispatches.len())?;
        let provider_dispatches = in_flight
            .iter()
            .map(|execution| provider_dispatch(execution))
            .collect::<Result<Vec<_>, _>>()?;
        let results = driver
            .execute_group(&execution_group_id, provider_dispatches)
            .await
            .map_err(|error| GraphRouterError::Provider(error.to_string()))?;
        let result_by_attempt = validate_provider_results(&in_flight, results)?;

        let consumed = self
            .consume_execution_group(ConsumeExecutionGroupRequest {
                run_id: started.wire().run_id.clone(),
                expected_revision: started.revision(),
                expected_state_digest: started.state_digest().clone(),
                expected_context_binding_digest: started.context_binding_digest().clone(),
                execution_group_id: execution_group_id.clone(),
                consumptions: in_flight
                    .iter()
                    .map(|authority| {
                        let material = dispatch_material(authority, &dispatch_by_stage_set)?;
                        let attempt_id = authority
                            .attempt_id()
                            .expect("phase filter returned only in-flight authorities");
                        let result = result_by_attempt.get(attempt_id).ok_or(
                            GraphRouterError::InvalidTransition(
                                "provider result set does not cover every group attempt",
                            ),
                        )?;
                        Ok(ExecutionConsumptionSpec {
                            event_id: material.consume_event_id.clone(),
                            attempt_id: attempt_id.clone(),
                            result_receipt_ref: result.result_receipt_ref.clone(),
                            result_digest: result.result_digest.clone(),
                        })
                    })
                    .collect::<Result<Vec<_>, GraphRouterError>>()?,
            })
            .await?;

        self.join_if_complete(consumed, execution_group_id, request.join_event_id)
            .await
    }

    pub async fn poll_search_loop<D: SearchExecutionDriver>(
        &self,
        request: SearchLoopPollRequest,
        driver: &D,
    ) -> Result<ValidatedContextProductStateV1, GraphRouterError> {
        let current = self.load(&request.run_id).await?;
        if current.revision() != request.expected_revision {
            return Err(GraphRouterError::Validation(
                ValidationError::RevisionMismatch,
            ));
        }
        if current.state_digest() != &request.expected_state_digest {
            return Err(GraphRouterError::Validation(
                ValidationError::StateChainMismatch,
            ));
        }
        if current.context_binding_digest() != &request.expected_context_binding_digest {
            return Err(GraphRouterError::Validation(
                ValidationError::StaleContextBinding,
            ));
        }
        let mut consume_event_by_stage_set = BTreeMap::new();
        for dispatch in &request.dispatches {
            if dispatch.stage_ids.is_empty()
                || consume_event_by_stage_set
                    .insert(
                        canonical_stage_set(&dispatch.stage_ids),
                        &dispatch.consume_event_id,
                    )
                    .is_some()
            {
                return Err(GraphRouterError::InvalidTransition(
                    "search-loop poll dispatch stage sets must be non-empty and unique",
                ));
            }
        }
        let in_flight = group_authorities(&current, &request.execution_group_id, |execution| {
            matches!(execution, ExecutionAuthority::InFlight(_))
                && consume_event_by_stage_set
                    .contains_key(&canonical_stage_set(execution.stage_ids()))
        });
        ensure_phase_cardinality(in_flight.len(), request.dispatches.len())?;
        let provider_dispatches = in_flight
            .iter()
            .map(|execution| provider_dispatch(execution))
            .collect::<Result<Vec<_>, _>>()?;
        let results = driver
            .execute_group(&request.execution_group_id, provider_dispatches)
            .await
            .map_err(|error| GraphRouterError::Provider(error.to_string()))?;
        let result_by_attempt = validate_provider_results(&in_flight, results)?;
        let consumed = self
            .consume_execution_group(ConsumeExecutionGroupRequest {
                run_id: current.wire().run_id.clone(),
                expected_revision: current.revision(),
                expected_state_digest: current.state_digest().clone(),
                expected_context_binding_digest: current.context_binding_digest().clone(),
                execution_group_id: request.execution_group_id.clone(),
                consumptions: in_flight
                    .iter()
                    .map(|authority| {
                        let consume_event_id = consume_event_by_stage_set
                            .get(&canonical_stage_set(authority.stage_ids()))
                            .ok_or(GraphRouterError::InvalidTransition(
                                "poll material does not match in-flight authority stage set",
                            ))?;
                        let attempt_id = authority
                            .attempt_id()
                            .expect("phase filter returned only in-flight authorities");
                        let result = result_by_attempt.get(attempt_id).ok_or(
                            GraphRouterError::InvalidTransition(
                                "provider result set does not cover every group attempt",
                            ),
                        )?;
                        Ok(ExecutionConsumptionSpec {
                            event_id: (*consume_event_id).clone(),
                            attempt_id: attempt_id.clone(),
                            result_receipt_ref: result.result_receipt_ref.clone(),
                            result_digest: result.result_digest.clone(),
                        })
                    })
                    .collect::<Result<Vec<_>, GraphRouterError>>()?,
            })
            .await?;
        self.join_if_complete(consumed, request.execution_group_id, request.join_event_id)
            .await
    }

    async fn join_if_complete(
        &self,
        state: ValidatedContextProductStateV1,
        execution_group_id: ProtocolId,
        join_event_id: ProtocolId,
    ) -> Result<ValidatedContextProductStateV1, GraphRouterError> {
        if !execution_group_complete(&state, &execution_group_id)? {
            return Ok(state);
        }
        self.join_execution_group(JoinExecutionGroupRequest {
            run_id: state.wire().run_id.clone(),
            expected_revision: state.revision(),
            expected_state_digest: state.state_digest().clone(),
            expected_context_binding_digest: state.context_binding_digest().clone(),
            event_id: join_event_id,
            execution_group_id,
        })
        .await
    }
}

fn directive_group_id(directive: &SearchLoopDirective) -> Result<&ProtocolId, GraphRouterError> {
    match directive {
        SearchLoopDirective::AdmitSerial { group_id, .. }
        | SearchLoopDirective::AdmitBatch { group_id, .. }
        | SearchLoopDirective::AdmitParallel { group_id, .. } => Ok(group_id),
        _ => Err(GraphRouterError::InvalidTransition(
            "search-loop advance requires an admission directive",
        )),
    }
}

fn canonical_stage_set(stage_ids: &[ProtocolId]) -> Vec<ProtocolId> {
    let mut stage_ids = stage_ids.to_vec();
    stage_ids.sort();
    stage_ids
}

fn dispatch_material_by_stage_set(
    dispatches: &[SearchLoopAdvanceDispatch],
) -> Result<BTreeMap<Vec<ProtocolId>, &SearchLoopAdvanceDispatch>, GraphRouterError> {
    let mut by_stage_set = BTreeMap::new();
    for dispatch in dispatches {
        let stage_set = canonical_stage_set(&dispatch.stage_ids);
        if stage_set.is_empty() || by_stage_set.insert(stage_set, dispatch).is_some() {
            return Err(GraphRouterError::InvalidTransition(
                "search-loop advance dispatch stage sets must be non-empty and unique",
            ));
        }
    }
    Ok(by_stage_set)
}

fn dispatch_material<'a>(
    authority: &ExecutionAuthority,
    dispatches: &'a BTreeMap<Vec<ProtocolId>, &SearchLoopAdvanceDispatch>,
) -> Result<&'a SearchLoopAdvanceDispatch, GraphRouterError> {
    dispatches
        .get(&canonical_stage_set(authority.stage_ids()))
        .copied()
        .ok_or(GraphRouterError::InvalidTransition(
            "execution authority does not match search-loop advance dispatch material",
        ))
}

fn group_authorities<'a>(
    state: &'a ValidatedContextProductStateV1,
    group_id: &ProtocolId,
    predicate: impl Fn(&ExecutionAuthority) -> bool,
) -> Vec<&'a ExecutionAuthority> {
    state
        .executions()
        .iter()
        .filter(|execution| execution.execution_group_id() == group_id && predicate(execution))
        .collect()
}

fn ensure_phase_cardinality(actual: usize, expected: usize) -> Result<(), GraphRouterError> {
    if actual != expected {
        return Err(GraphRouterError::InvalidTransition(
            "search-loop execution phase does not cover the complete dispatch set",
        ));
    }
    Ok(())
}

fn execution_group_complete(
    state: &ValidatedContextProductStateV1,
    execution_group_id: &ProtocolId,
) -> Result<bool, GraphRouterError> {
    let ActiveProgram::Admitted { program, .. } = state.active_program() else {
        return Err(GraphRouterError::InvalidTransition(
            "search-loop execution requires an admitted route program",
        ));
    };
    let group = program
        .execution_groups
        .iter()
        .find(|group| &group.group_id == execution_group_id)
        .ok_or(GraphRouterError::InvalidTransition(
            "search-loop execution group is not in the admitted route program",
        ))?;
    Ok(group.stage_ids.iter().all(|stage_id| {
        state.executions().iter().any(|execution| {
            execution.execution_group_id() == execution_group_id
                && execution.stage_ids().contains(stage_id)
                && matches!(execution, ExecutionAuthority::Consumed(_))
        })
    }))
}

fn provider_dispatch(
    execution: &ExecutionAuthority,
) -> Result<ProviderExecutionDispatch, GraphRouterError> {
    if !matches!(execution, ExecutionAuthority::InFlight(_)) {
        return Err(GraphRouterError::InvalidTransition(
            "provider dispatch requires in-flight execution authority",
        ));
    }
    Ok(ProviderExecutionDispatch {
        execution_group_id: execution.execution_group_id().clone(),
        stage_ids: execution.stage_ids().to_vec(),
        attempt_id: execution
            .attempt_id()
            .expect("in-flight authority has attempt id")
            .clone(),
        grant_id: execution
            .grant_id()
            .expect("in-flight authority has grant id")
            .clone(),
        grant_digest: execution
            .grant_digest()
            .expect("in-flight authority has grant digest")
            .clone(),
        provider_id: execution.provider_id().clone(),
        operation: execution.operation().clone(),
        resolved_input_digest: execution
            .resolved_input_digest()
            .expect("in-flight authority has resolved input digest")
            .clone(),
        effect_class: execution
            .effect_class()
            .expect("in-flight authority has effect class"),
        lease_fence: execution
            .lease_fence()
            .expect("in-flight authority has lease fence"),
        expires_at_ms: execution
            .expires_at_ms()
            .expect("in-flight authority has expiry"),
        provider_idempotency_key: execution
            .provider_idempotency_key()
            .expect("in-flight authority has provider idempotency key")
            .clone(),
    })
}

fn validate_provider_results(
    in_flight: &[&ExecutionAuthority],
    results: Vec<crate::ProviderExecutionResult>,
) -> Result<BTreeMap<ProtocolId, crate::ProviderExecutionResult>, GraphRouterError> {
    let expected_attempts = in_flight
        .iter()
        .filter_map(|execution| execution.attempt_id().cloned())
        .collect::<BTreeSet<_>>();
    let mut by_attempt = BTreeMap::new();
    for result in results {
        if !expected_attempts.contains(&result.attempt_id)
            || by_attempt
                .insert(result.attempt_id.clone(), result)
                .is_some()
        {
            return Err(GraphRouterError::InvalidTransition(
                "provider returned an unknown or duplicate execution attempt",
            ));
        }
    }
    if by_attempt.len() != expected_attempts.len() {
        return Err(GraphRouterError::InvalidTransition(
            "provider result set does not cover every group attempt",
        ));
    }
    Ok(by_attempt)
}
