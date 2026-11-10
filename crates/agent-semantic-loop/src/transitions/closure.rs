// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

use agent_semantic_context_product::ActiveProgram;
use agent_semantic_context_product::ClosureDisposition;
use agent_semantic_context_product::ClosureFinalizedEventType;
use agent_semantic_context_product::ClosureProof;
use agent_semantic_context_product::ContextProductEvent;
use agent_semantic_context_product::DecisionRequirement;
use agent_semantic_context_product::Digest;
use agent_semantic_context_product::ExecutionAuthority;
use agent_semantic_context_product::ObligationDisposition;
use agent_semantic_context_product::ProtocolId;
use agent_semantic_context_product::SearchClosureReceipt;
use agent_semantic_context_product::UncheckedContextProductStateV1;
use agent_semantic_context_product::chained_event_log_digest;

use crate::GraphRouter;
use crate::GraphRouterError;
use crate::ProofResolver;
use crate::RunCommitStore;
use crate::TrustedClock;
use crate::ValidatedContextProductStateV1;
use crate::graph_router::authority_receipt_id;

use super::transition_core::canonical_digest;
use super::transition_core::commit_state;
use super::transition_core::derived_id;
use super::transition_core::next_clock;
use super::transition_core::require_expected_head;

#[derive(Clone, Debug)]
pub struct FinalizeClosureRequest {
    pub run_id: ProtocolId,
    pub expected_revision: u64,
    pub expected_state_digest: Digest,
    pub expected_context_binding_digest: Digest,
    pub closed_obligations: Vec<ClosureProof>,
}

impl<S, P, C> GraphRouter<S, P, C>
where
    S: RunCommitStore,
    P: ProofResolver,
    C: TrustedClock,
{
    pub async fn finalize_closure(
        &self,
        request: FinalizeClosureRequest,
    ) -> Result<ValidatedContextProductStateV1, GraphRouterError> {
        let current = self.load(&request.run_id).await?;
        require_expected_head(
            &current,
            request.expected_revision,
            &request.expected_state_digest,
            &request.expected_context_binding_digest,
        )?;

        if matches!(current.wire().closure, ClosureDisposition::Finalized { .. }) {
            return Err(GraphRouterError::InvalidTransition(
                "search closure is already finalized",
            ));
        }
        let decision_open_obligation_ids = match &current.wire().decision {
            DecisionRequirement::Search {
                open_obligation_ids,
                ..
            } => Some(open_obligation_ids),
            DecisionRequirement::None => None,
            _ => {
                return Err(GraphRouterError::InvalidTransition(
                    "search closure cannot consume a non-search decision",
                ));
            }
        };
        if current.wire().executions.iter().any(|execution| {
            !matches!(
                execution,
                ExecutionAuthority::Consumed { .. } | ExecutionAuthority::Revoked { .. }
            )
        }) {
            return Err(GraphRouterError::InvalidTransition(
                "search closure requires no active execution authority",
            ));
        }

        let ActiveProgram::Admitted {
            intent_digest,
            program_id,
            program_digest,
            program,
            ..
        } = &current.wire().active_program
        else {
            return Err(GraphRouterError::InvalidTransition(
                "search closure requires an admitted route program",
            ));
        };
        if program.execution_groups.iter().any(|group| {
            !current
                .wire()
                .joined_execution_groups
                .iter()
                .any(|joined| joined.execution_group_id == group.group_id)
        }) {
            return Err(GraphRouterError::InvalidTransition(
                "search closure requires every execution group joined",
            ));
        }

        let required_count = current
            .wire()
            .obligations
            .iter()
            .filter(|obligation| obligation.required)
            .count();
        let unresolved_required_ids = current
            .wire()
            .obligations
            .iter()
            .filter(|obligation| {
                obligation.required
                    && !matches!(
                        obligation.disposition,
                        ObligationDisposition::Resolved { .. }
                    )
            })
            .map(|obligation| &obligation.obligation_id)
            .collect::<Vec<_>>();
        match decision_open_obligation_ids {
            Some(open_ids)
                if open_ids.len() == unresolved_required_ids.len()
                    && unresolved_required_ids
                        .iter()
                        .all(|obligation_id| open_ids.contains(obligation_id)) => {}
            None if unresolved_required_ids.is_empty() => {}
            _ => {
                return Err(GraphRouterError::InvalidTransition(
                    "search decision does not match unresolved required obligations",
                ));
            }
        }
        if request.closed_obligations.len() != required_count {
            return Err(GraphRouterError::InvalidTransition(
                "closure proofs must cover every required obligation exactly once",
            ));
        }

        let mut ordered_proofs = Vec::with_capacity(required_count);
        for obligation in current
            .wire()
            .obligations
            .iter()
            .filter(|obligation| obligation.required)
        {
            let mut matches = request
                .closed_obligations
                .iter()
                .filter(|proof| proof.obligation_id == obligation.obligation_id);
            let proof = matches.next().ok_or(GraphRouterError::InvalidTransition(
                "missing closure proof for required obligation",
            ))?;
            if matches.next().is_some() {
                return Err(GraphRouterError::InvalidTransition(
                    "duplicate closure proof for required obligation",
                ));
            }
            if proof.claim_class != obligation.claim_class {
                return Err(GraphRouterError::InvalidTransition(
                    "closure proof claim class does not match obligation",
                ));
            }
            if proof.proof_refs.is_empty() {
                return Err(GraphRouterError::InvalidTransition(
                    "closure proof must contain at least one proof reference",
                ));
            }
            if !proof.evidence_scope.complete
                || proof.evidence_scope.truncated
                || !proof.evidence_scope.snapshot_continuous
                || !proof.evidence_scope.provider_admitted
            {
                return Err(GraphRouterError::InvalidTransition(
                    "closure proof evidence scope is not complete and admitted",
                ));
            }

            for (index, proof_ref) in proof.proof_refs.iter().enumerate() {
                if proof.proof_refs[..index].contains(proof_ref) {
                    return Err(GraphRouterError::InvalidTransition(
                        "closure proof contains duplicate proof references",
                    ));
                }
                let receipt = self
                    .proof_resolver
                    .resolve(proof_ref)
                    .await
                    .map_err(|error| GraphRouterError::Proof(error.to_string()))?;
                if receipt.proof_ref != *proof_ref
                    || receipt.claim_class != obligation.claim_class
                    || receipt.claim_digest != obligation.claim_digest
                    || receipt.verdict != proof.verdict
                    || receipt.context_binding_digest != current.wire().context.binding_digest
                    || receipt.source_snapshot_digest
                        != current.wire().context.source_snapshot_digest
                    || receipt.provider_digest != current.wire().context.provider_digest
                    || receipt.parser_digest != current.wire().context.parser_digest
                    || receipt.query_pack_digest != current.wire().context.query_pack_digest
                    || receipt.scope_digest != proof.evidence_scope.scope_digest
                    || receipt.support_closure_digest != canonical_digest(&proof.support_paths)
                {
                    return Err(GraphRouterError::InvalidTransition(
                        "resolved evidence receipt does not match closure proof and context",
                    ));
                }
            }
            ordered_proofs.push(proof.clone());
        }

        let (committed_at_ms, next_revision, next_sequence) = next_clock(&self.clock, &current)?;
        let expected = current.head();
        let mut next_state: UncheckedContextProductStateV1 = current.wire().clone();
        for proof in &ordered_proofs {
            let obligation = next_state
                .obligations
                .iter_mut()
                .find(|obligation| obligation.obligation_id == proof.obligation_id)
                .ok_or(GraphRouterError::InvalidTransition(
                    "closure proof obligation disappeared during reduction",
                ))?;
            obligation.disposition = ObligationDisposition::Resolved {
                verdict: proof.verdict,
                proof_refs: proof.proof_refs.clone(),
            };
        }

        let receipt_seed = canonical_digest(&serde_json::json!({
            "runId": request.run_id,
            "nextRevision": next_revision,
            "programDigest": program_digest,
            "closedObligations": ordered_proofs,
        }));
        let receipt_id = derived_id("closure-receipt", &receipt_seed)?;
        let event_id = derived_id("closure-finalized-event", &receipt_seed)?;
        let mut receipt = SearchClosureReceipt {
            event_type: ClosureFinalizedEventType::ClosureFinalized,
            receipt_id: receipt_id.clone(),
            event_id,
            run_id: request.run_id,
            sequence: next_sequence,
            state_revision: next_revision,
            pre_state_digest: current.wire().state_digest.clone(),
            context_binding_digest: current.wire().context.binding_digest.clone(),
            intent_digest: intent_digest.clone(),
            route_program_id: program_id.clone(),
            program_digest: program_digest.clone(),
            closed_obligations: ordered_proofs,
            finalized_at_ms: committed_at_ms,
            receipt_digest: Digest::from_bytes(b"closure-receipt-pending"),
        };
        receipt.receipt_digest = receipt.recompute_receipt_digest();
        let next_event_log_digest =
            chained_event_log_digest(&current.wire().event_log_digest, &receipt.receipt_digest);
        let authority_receipt_ref =
            authority_receipt_id(&expected, next_revision, &next_event_log_digest)?;
        next_state.closure = ClosureDisposition::Finalized {
            receipt_id,
            receipt_digest: receipt.receipt_digest.clone(),
        };
        next_state.decision = DecisionRequirement::None;
        next_state.revision = next_revision;
        next_state.previous_state_digest = Some(current.wire().state_digest.clone());
        next_state.last_event_sequence = next_sequence;
        next_state.event_log_digest = next_event_log_digest;
        next_state.authority_receipt_ref = authority_receipt_ref;
        next_state.state_digest = next_state.recompute_state_digest();
        next_state
            .validate()
            .map_err(GraphRouterError::Validation)?;

        commit_state(
            &self.store,
            expected,
            ContextProductEvent::ClosureFinalized(receipt),
            next_state,
            current.search_loop_capabilities().to_vec(),
            current.search_loop_runtime().cloned(),
            committed_at_ms,
        )
        .await
    }
}
