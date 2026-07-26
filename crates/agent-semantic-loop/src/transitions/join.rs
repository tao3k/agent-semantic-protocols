use agent_semantic_context_product::{
    ActiveProgram, ContextProductEvent, Digest, ExecutionAuthority, ExecutionGroupJoined,
    ExecutionGroupJoinedEventType, JoinedExecutionGroup, ProtocolId,
    UncheckedContextProductStateV1, ValidationError, chained_event_log_digest,
};

use super::transition_core::{commit_state, next_clock, require_expected_head};
use crate::{
    GraphRouter, GraphRouterError, ProofResolver, RunCommitStore, TrustedClock,
    ValidatedContextProductStateV1, graph_router::authority_receipt_id,
};

#[derive(Clone, Debug)]
pub struct JoinExecutionGroupRequest {
    pub run_id: ProtocolId,
    pub expected_revision: u64,
    pub expected_state_digest: Digest,
    pub expected_context_binding_digest: Digest,
    pub event_id: ProtocolId,
    pub execution_group_id: ProtocolId,
}

impl<S, P, C> GraphRouter<S, P, C>
where
    S: RunCommitStore,
    P: ProofResolver,
    C: TrustedClock,
{
    pub async fn join_execution_group(
        &self,
        request: JoinExecutionGroupRequest,
    ) -> Result<ValidatedContextProductStateV1, GraphRouterError> {
        let current = self.load(&request.run_id).await?;
        require_expected_head(
            &current,
            request.expected_revision,
            &request.expected_state_digest,
            &request.expected_context_binding_digest,
        )?;
        let ActiveProgram::Admitted {
            program_digest,
            program,
            ..
        } = current.active_program()
        else {
            return Err(GraphRouterError::InvalidTransition(
                "execution-group join requires an admitted route program",
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
        if current
            .wire()
            .joined_execution_groups
            .iter()
            .any(|joined| joined.execution_group_id == group.group_id)
        {
            return Err(GraphRouterError::InvalidTransition(
                "execution group is already joined",
            ));
        }
        let consumed = current
            .executions()
            .iter()
            .filter(|execution| {
                execution.execution_group_id() == &group.group_id
                    && matches!(execution, ExecutionAuthority::Consumed(_))
            })
            .collect::<Vec<_>>();
        if group.stage_ids.iter().any(|stage_id| {
            !consumed
                .iter()
                .any(|execution| execution.stage_ids().contains(stage_id))
        }) {
            return Err(GraphRouterError::InvalidTransition(
                "execution-group join requires every group stage consumed",
            ));
        }
        let mut result_receipt_refs = consumed
            .iter()
            .filter_map(|execution| execution.result_receipt_ref().cloned())
            .collect::<Vec<_>>();
        result_receipt_refs.sort();
        result_receipt_refs.dedup();
        if result_receipt_refs.is_empty() {
            return Err(GraphRouterError::InvalidTransition(
                "execution-group join requires provider result receipts",
            ));
        }

        let (joined_at_ms, next_revision, next_sequence) = next_clock(&self.clock, &current)?;
        let mut joined = JoinedExecutionGroup {
            execution_group_id: group.group_id.clone(),
            policy: group.join_policy,
            result_receipt_refs,
            join_digest: Digest::from_bytes(b"pending-joined-execution-group"),
        };
        joined.join_digest = joined.recompute_join_digest();
        let mut event = ExecutionGroupJoined {
            event_type: ExecutionGroupJoinedEventType::ExecutionGroupJoined,
            event_id: request.event_id,
            run_id: request.run_id,
            sequence: next_sequence,
            state_revision: current.revision(),
            pre_state_digest: current.state_digest().clone(),
            program_digest: program_digest.clone(),
            execution_group_id: group.group_id.clone(),
            policy: group.join_policy,
            result_receipt_refs: joined.result_receipt_refs.clone(),
            joined_group_digest: joined.join_digest.clone(),
            event_digest: Digest::from_bytes(b"pending-execution-group-joined-event"),
        };
        event.event_digest = event.recompute_event_digest();
        let next_event_log_digest =
            chained_event_log_digest(&current.wire().event_log_digest, &event.event_digest);
        let expected = current.head();
        let mut next_state: UncheckedContextProductStateV1 = current.wire().clone();
        next_state.revision = next_revision;
        next_state.previous_state_digest = Some(current.state_digest().clone());
        next_state.last_event_sequence = next_sequence;
        next_state.event_log_digest = next_event_log_digest;
        next_state.joined_execution_groups.push(joined);
        next_state
            .joined_execution_groups
            .sort_by(|left, right| left.execution_group_id.cmp(&right.execution_group_id));
        next_state.authority_receipt_ref =
            authority_receipt_id(&expected, next_revision, &next_state.event_log_digest)?;
        next_state.state_digest = next_state.recompute_state_digest();
        next_state
            .validate()
            .map_err(GraphRouterError::Validation)?;
        commit_state(
            &self.store,
            expected,
            ContextProductEvent::ExecutionGroupJoined(event),
            next_state,
            current.search_loop_capabilities().to_vec(),
            current.search_loop_runtime().cloned(),
            joined_at_ms,
        )
        .await
    }
}
