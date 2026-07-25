use super::{
    ClosureDisposition, ContextBinding, DecisionRequirement, Digest, Obligation,
    ObligationDisposition, ProofReuse, ProofReuseMode, ProtocolId, UncheckedContextProductStateV1,
    ValidationError,
};
use serde::Serialize;
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};

use super::model::{CONTEXT_PRODUCT_SCHEMA_ID, CONTEXT_PRODUCT_SCHEMA_VERSION};

impl ContextBinding {
    pub fn recompute_binding_digest(&self) -> Digest {
        canonical_digest_without_field(self, "bindingDigest")
    }
}

impl super::model::FrontierAntichain {
    pub fn recompute_frontier_digest(&self) -> Digest {
        canonical_digest_without_field(self, "frontierDigest")
    }
}

impl super::StateAuthorityReceipt {
    pub fn recompute_receipt_digest(&self) -> Digest {
        canonical_digest_without_field(self, "receiptDigest")
    }

    pub fn validate_for_state(
        &self,
        state: &UncheckedContextProductStateV1,
    ) -> Result<(), ValidationError> {
        if self.run_id != state.run_id
            || self.revision != state.revision
            || self.state_digest != state.state_digest
            || self.event_log_digest != state.event_log_digest
            || self.receipt_id != state.authority_receipt_ref
        {
            return Err(ValidationError::StateChainMismatch);
        }
        if self.issued_at_ms > super::JSON_SAFE_INTEGER_MAX {
            return Err(ValidationError::UnsafeInteger("issuedAtMs"));
        }
        if self.receipt_digest != self.recompute_receipt_digest() {
            return Err(ValidationError::DigestMismatch(
                "authorityReceipt.receiptDigest",
            ));
        }
        Ok(())
    }
}

impl super::RouteProposal {
    pub fn recompute_proposal_digest(&self) -> Digest {
        canonical_digest_without_field(self, "proposalDigest")
    }
}

impl super::RouteProgram {
    pub fn recompute_graph_digest(&self) -> Digest {
        let mut stages = self.stages.clone();
        stages.sort_by(|left, right| left.stage_id.cmp(&right.stage_id));
        for stage in &mut stages {
            stage.covers_obligation_ids.sort();
            stage.evidence_predicate.accepted_schema_ids.sort();
            stage.evidence_predicate.required_fields.sort();
        }
        let mut edges = self.edges.clone();
        edges.sort_by(|left, right| (&left.from, &left.to).cmp(&(&right.from, &right.to)));
        let mut joins = self.joins.clone();
        joins.sort_by(|left, right| left.join_id.cmp(&right.join_id));
        for join in &mut joins {
            join.required_stage_ids.sort();
        }
        canonical_digest(&GraphDigestProjection {
            stages: &stages,
            edges: &edges,
            joins: &joins,
        })
    }

    pub fn recompute_program_digest(&self) -> Digest {
        canonical_digest_without_field(self, "programDigest")
    }
}

impl super::RouteProgramAdmitted {
    pub fn recompute_event_digest(&self) -> Digest {
        canonical_digest_without_field(self, "eventDigest")
    }
}

impl super::ActionAdmitted {
    pub fn recompute_admission_digest(&self) -> Digest {
        canonical_digest_without_field(self, "admissionDigest")
    }
}

impl super::ExecutionGrantIssued {
    pub fn recompute_grant_digest(&self) -> Digest {
        canonical_digest_without_field(self, "grantDigest")
    }
}

impl super::ExecutionStarted {
    pub fn recompute_event_digest(&self) -> Digest {
        canonical_digest_without_field(self, "eventDigest")
    }
}

impl super::ExecutionConsumed {
    pub fn recompute_event_digest(&self) -> Digest {
        canonical_digest_without_field(self, "eventDigest")
    }
}

impl super::ExecutionRevoked {
    pub fn recompute_event_digest(&self) -> Digest {
        canonical_digest_without_field(self, "eventDigest")
    }
}

impl super::SearchClosureReceipt {
    pub fn recompute_receipt_digest(&self) -> Digest {
        canonical_digest_without_field(self, "receiptDigest")
    }
}

impl UncheckedContextProductStateV1 {
    pub fn recompute_state_digest(&self) -> Digest {
        canonical_digest_without_field(self, "stateDigest")
    }

    pub fn validate(&self) -> Result<(), ValidationError> {
        if self.schema_id != CONTEXT_PRODUCT_SCHEMA_ID {
            return Err(ValidationError::InvalidSchemaId(self.schema_id.clone()));
        }
        if self.schema_version != CONTEXT_PRODUCT_SCHEMA_VERSION {
            return Err(ValidationError::InvalidSchemaVersion(self.schema_version));
        }
        if self.canonicalization_profile != super::CONTEXT_PRODUCT_CANONICALIZATION_PROFILE {
            return Err(ValidationError::InvalidCanonicalizationProfile(
                self.canonicalization_profile.clone(),
            ));
        }
        if self.revision > super::JSON_SAFE_INTEGER_MAX {
            return Err(ValidationError::UnsafeInteger("revision"));
        }
        if self.last_event_sequence > super::JSON_SAFE_INTEGER_MAX {
            return Err(ValidationError::UnsafeInteger("lastEventSequence"));
        }
        if (self.revision == 0) != self.previous_state_digest.is_none() {
            return Err(ValidationError::StateChainMismatch);
        }
        match &self.active_program {
            super::ActiveProgram::None => {}
            super::ActiveProgram::Admitted {
                admitted_at_revision,
                context_binding_digest,
                ..
            } if *admitted_at_revision <= self.revision
                && *admitted_at_revision <= super::JSON_SAFE_INTEGER_MAX
                && context_binding_digest == &self.context.binding_digest => {}
            super::ActiveProgram::Admitted { .. } => {
                return Err(ValidationError::InvalidActiveProgram);
            }
        }
        if self
            .spent_action_keys
            .windows(2)
            .any(|pair| pair[0] >= pair[1])
        {
            return Err(ValidationError::DuplicateActionIdentity);
        }
        if self.spent_action_ledger_digest != self.recompute_spent_action_ledger_digest() {
            return Err(ValidationError::DigestMismatch("spentActionLedgerDigest"));
        }
        if self.context.binding_digest != self.context.recompute_binding_digest() {
            return Err(ValidationError::DigestMismatch("context.bindingDigest"));
        }
        if self.frontier.frontier_digest != self.frontier.recompute_frontier_digest() {
            return Err(ValidationError::DigestMismatch("frontier.frontierDigest"));
        }
        if self.state_digest != self.recompute_state_digest() {
            return Err(ValidationError::DigestMismatch("stateDigest"));
        }
        self.validate_obligations()?;
        self.validate_proof_reuse()?;
        self.validate_frontier()?;
        self.validate_decision()?;
        self.validate_execution()?;
        self.validate_closure()
    }

    pub fn recompute_spent_action_ledger_digest(&self) -> Digest {
        canonical_digest(&self.spent_action_keys)
    }

    fn validate_execution(&self) -> Result<(), ValidationError> {
        match &self.execution {
            super::ExecutionAuthority::None
            | super::ExecutionAuthority::Admitted { .. }
            | super::ExecutionAuthority::Revoked { .. } => {}
            super::ExecutionAuthority::Granted {
                lease_fence,
                expires_at_ms,
                ..
            }
            | super::ExecutionAuthority::InFlight {
                lease_fence,
                expires_at_ms,
                ..
            } => {
                if *lease_fence == 0
                    || *lease_fence > super::JSON_SAFE_INTEGER_MAX
                    || *expires_at_ms > super::JSON_SAFE_INTEGER_MAX
                {
                    return Err(ValidationError::InvalidExecutionState);
                }
            }
            super::ExecutionAuthority::Consumed { lease_fence, .. } => {
                if *lease_fence == 0 || *lease_fence > super::JSON_SAFE_INTEGER_MAX {
                    return Err(ValidationError::InvalidExecutionState);
                }
            }
        }
        Ok(())
    }

    pub(crate) fn obligation_map(
        &self,
    ) -> Result<BTreeMap<&ProtocolId, &Obligation>, ValidationError> {
        let mut obligations = BTreeMap::new();
        for obligation in &self.obligations {
            if obligations
                .insert(&obligation.obligation_id, obligation)
                .is_some()
            {
                return Err(ValidationError::DuplicateId(
                    obligation.obligation_id.clone(),
                ));
            }
        }
        Ok(obligations)
    }

    fn validate_obligations(&self) -> Result<(), ValidationError> {
        let obligations = self.obligation_map()?;
        for obligation in &self.obligations {
            for dependency in &obligation.depends_on {
                if !obligations.contains_key(dependency) {
                    return Err(ValidationError::MissingReference {
                        kind: "obligation dependency",
                        id: dependency.clone(),
                    });
                }
            }
            match &obligation.disposition {
                ObligationDisposition::Resolved { proof_refs, .. } if proof_refs.is_empty() => {
                    return Err(ValidationError::EmptyRequiredCollection("proofRefs"));
                }
                ObligationDisposition::RefreshRequired {
                    invalidated_proof_refs,
                    ..
                } if invalidated_proof_refs.is_empty() => {
                    return Err(ValidationError::EmptyRequiredCollection(
                        "invalidatedProofRefs",
                    ));
                }
                _ => {}
            }
        }
        let mut visiting = BTreeSet::new();
        let mut visited = BTreeSet::new();
        for obligation in &self.obligations {
            visit_obligation(
                &obligation.obligation_id,
                &obligations,
                &mut visiting,
                &mut visited,
            )?;
        }
        Ok(())
    }

    fn validate_proof_reuse(&self) -> Result<(), ValidationError> {
        let current_binding = &self.context.binding_digest;
        let validate_retained = |retained: &[super::RetainedProof]| {
            for proof in retained {
                match proof.mode {
                    ProofReuseMode::Direct => {
                        if proof.context_binding_digest != *current_binding {
                            return Err(ValidationError::StaleContextBinding);
                        }
                        if proof.rebase_certificate_ref.is_some() {
                            return Err(ValidationError::UnexpectedField("rebaseCertificateRef"));
                        }
                    }
                    ProofReuseMode::Rebased => {
                        if proof.rebase_certificate_ref.is_none() {
                            return Err(ValidationError::MissingField("rebaseCertificateRef"));
                        }
                    }
                }
            }
            Ok(())
        };
        match &self.proof_reuse {
            ProofReuse::None {
                retained,
                invalidated_proof_refs: _,
            } => {
                if !retained.is_empty() {
                    return Err(ValidationError::UnexpectedNonEmptyCollection("retained"));
                }
            }
            ProofReuse::Partial {
                retained,
                invalidated_proof_refs,
            } => {
                if retained.is_empty() {
                    return Err(ValidationError::EmptyRequiredCollection("retained"));
                }
                if invalidated_proof_refs.is_empty() {
                    return Err(ValidationError::EmptyRequiredCollection(
                        "invalidatedProofRefs",
                    ));
                }
                validate_retained(retained)?;
            }
            ProofReuse::Complete {
                retained,
                invalidated_proof_refs,
            } => {
                if retained.is_empty() {
                    return Err(ValidationError::EmptyRequiredCollection("retained"));
                }
                if !invalidated_proof_refs.is_empty() {
                    return Err(ValidationError::UnexpectedNonEmptyCollection(
                        "invalidatedProofRefs",
                    ));
                }
                if self
                    .obligations
                    .iter()
                    .any(|obligation| obligation.required && !obligation.disposition.is_resolved())
                {
                    return Err(ValidationError::IncompleteProofReuse);
                }
                validate_retained(retained)?;
            }
        }
        Ok(())
    }

    fn validate_frontier(&self) -> Result<(), ValidationError> {
        if self.frontier.context_binding_digest != self.context.binding_digest {
            return Err(ValidationError::StaleContextBinding);
        }
        let obligations = self.obligation_map()?;
        let mut node_ids = BTreeSet::new();
        for node in &self.frontier.nodes {
            if !node_ids.insert(&node.node_id) {
                return Err(ValidationError::DuplicateId(node.node_id.clone()));
            }
            for obligation_id in &node.open_obligation_ids {
                let obligation = obligations.get(obligation_id).ok_or_else(|| {
                    ValidationError::MissingReference {
                        kind: "frontier obligation",
                        id: obligation_id.clone(),
                    }
                })?;
                if !obligation.disposition.is_open_for_closure() {
                    return Err(ValidationError::ClosedObligationInFrontier(
                        obligation_id.clone(),
                    ));
                }
            }
        }
        for node in &self.frontier.nodes {
            if node
                .predecessor_ids
                .iter()
                .any(|predecessor| !node.ancestor_node_ids.contains(predecessor))
                || node
                    .ancestor_node_ids
                    .iter()
                    .any(|ancestor| node_ids.contains(ancestor))
            {
                return Err(ValidationError::FrontierNotAntichain);
            }
        }
        Ok(())
    }

    fn validate_decision(&self) -> Result<(), ValidationError> {
        let obligations = self.obligation_map()?;
        match &self.decision {
            DecisionRequirement::None
                if self.obligations.iter().any(|obligation| {
                    obligation.required && obligation.disposition.is_open_for_closure()
                }) =>
            {
                return Err(ValidationError::OpenObligationsWithoutDecision);
            }
            DecisionRequirement::Search {
                open_obligation_ids,
                budget,
                ..
            } => {
                if open_obligation_ids.is_empty() {
                    return Err(ValidationError::EmptyRequiredCollection(
                        "openObligationIds",
                    ));
                }
                if budget.max_commands == 0
                    || budget.max_elapsed_ms == 0
                    || budget.max_packet_bytes == 0
                    || budget.max_commands > super::JSON_SAFE_INTEGER_MAX
                    || budget.max_elapsed_ms > super::JSON_SAFE_INTEGER_MAX
                    || budget.max_packet_bytes > super::JSON_SAFE_INTEGER_MAX
                {
                    return Err(ValidationError::InvalidBudget);
                }
                for obligation_id in open_obligation_ids {
                    let obligation = obligations.get(obligation_id).ok_or_else(|| {
                        ValidationError::MissingReference {
                            kind: "decision obligation",
                            id: obligation_id.clone(),
                        }
                    })?;
                    if !obligation.disposition.is_open_for_closure() {
                        return Err(ValidationError::ClosedObligationInDecision(
                            obligation_id.clone(),
                        ));
                    }
                }
                let unresolved_required = self
                    .obligations
                    .iter()
                    .filter(|obligation| {
                        obligation.required && obligation.disposition.is_open_for_closure()
                    })
                    .collect::<Vec<_>>();
                if open_obligation_ids.len() != unresolved_required.len()
                    || unresolved_required
                        .iter()
                        .any(|obligation| !open_obligation_ids.contains(&obligation.obligation_id))
                {
                    return Err(ValidationError::DecisionObligationSetMismatch);
                }
            }
            DecisionRequirement::Reason { choice_ids, .. } if choice_ids.is_empty() => {
                return Err(ValidationError::EmptyRequiredCollection("choiceIds"));
            }
            _ => {}
        }
        Ok(())
    }

    fn validate_closure(&self) -> Result<(), ValidationError> {
        let required: BTreeSet<_> = self
            .obligations
            .iter()
            .filter(|obligation| obligation.required)
            .map(|obligation| obligation.obligation_id.clone())
            .collect();
        let unresolved: BTreeSet<_> = self
            .obligations
            .iter()
            .filter(|obligation| {
                obligation.required && obligation.disposition.is_open_for_closure()
            })
            .map(|obligation| obligation.obligation_id.clone())
            .collect();
        match &self.closure {
            ClosureDisposition::Open {
                open_obligation_ids,
            } => {
                let open = unique_set("openObligationIds", open_obligation_ids)?;
                if open.is_empty() {
                    return Err(ValidationError::EmptyRequiredCollection(
                        "openObligationIds",
                    ));
                }
                if open != unresolved {
                    return Err(ValidationError::ClosurePartitionMismatch);
                }
            }
            ClosureDisposition::Partial {
                closed_obligation_ids,
                open_obligation_ids,
            } => {
                let closed = unique_set("closedObligationIds", closed_obligation_ids)?;
                let open = unique_set("openObligationIds", open_obligation_ids)?;
                if closed.is_empty() || open.is_empty() || !closed.is_disjoint(&open) {
                    return Err(ValidationError::ClosurePartitionMismatch);
                }
                if closed.union(&open).cloned().collect::<BTreeSet<_>>() != required
                    || open != unresolved
                {
                    return Err(ValidationError::ClosurePartitionMismatch);
                }
            }
            ClosureDisposition::Finalized { .. } => {
                if !unresolved.is_empty() {
                    return Err(ValidationError::FinalizedWithOpenObligations);
                }
                if !matches!(self.decision, DecisionRequirement::None)
                    || !matches!(
                        self.execution,
                        super::ExecutionAuthority::None
                            | super::ExecutionAuthority::Consumed { .. }
                            | super::ExecutionAuthority::Revoked { .. }
                    )
                {
                    return Err(ValidationError::InvalidExecutionState);
                }
            }
        }
        Ok(())
    }
}

pub fn chained_event_log_digest(previous: &Digest, event: &Digest) -> Digest {
    canonical_digest(&serde_json::json!({
        "eventDigest": event,
        "previousEventLogDigest": previous,
        "profile": super::CONTEXT_PRODUCT_CANONICALIZATION_PROFILE,
    }))
}

pub(crate) fn canonical_digest_without_field<T: Serialize>(value: &T, field: &str) -> Digest {
    let mut value = serde_json::to_value(value).expect("protocol values serialize");
    if let Value::Object(object) = &mut value {
        object.remove(field);
    }
    canonical_json_digest(value)
}

fn canonical_json_digest(value: Value) -> Digest {
    let canonical = canonicalize_json(value);
    let bytes = serde_json::to_vec(&canonical).expect("canonical protocol JSON serializes");
    Digest::from_bytes(&bytes)
}

fn canonical_digest<T: Serialize>(value: &T) -> Digest {
    let value = serde_json::to_value(value).expect("protocol values serialize");
    canonical_json_digest(value)
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct GraphDigestProjection<'a> {
    stages: &'a [super::RouteStage],
    edges: &'a [super::RouteEdge],
    joins: &'a [super::RouteJoin],
}

fn canonicalize_json(value: Value) -> Value {
    match value {
        Value::Array(values) => Value::Array(values.into_iter().map(canonicalize_json).collect()),
        Value::Object(values) => {
            let ordered = values
                .into_iter()
                .map(|(key, value)| (key, canonicalize_json(value)))
                .collect::<BTreeMap<_, _>>();
            Value::Object(ordered.into_iter().collect())
        }
        scalar => scalar,
    }
}

pub(crate) fn unique_set(
    field: &'static str,
    values: &[ProtocolId],
) -> Result<BTreeSet<ProtocolId>, ValidationError> {
    let set = values.iter().cloned().collect::<BTreeSet<_>>();
    if set.len() != values.len() {
        return Err(ValidationError::InvalidRouteProgram(field));
    }
    Ok(set)
}

fn visit_obligation<'a>(
    id: &'a ProtocolId,
    obligations: &BTreeMap<&'a ProtocolId, &'a Obligation>,
    visiting: &mut BTreeSet<ProtocolId>,
    visited: &mut BTreeSet<ProtocolId>,
) -> Result<(), ValidationError> {
    if visited.contains(id) {
        return Ok(());
    }
    if !visiting.insert(id.clone()) {
        return Err(ValidationError::DependencyCycle(id.clone()));
    }
    let obligation = obligations
        .get(id)
        .expect("dependency references validated before cycle traversal");
    for dependency in &obligation.depends_on {
        visit_obligation(dependency, obligations, visiting, visited)?;
    }
    visiting.remove(id);
    visited.insert(id.clone());
    Ok(())
}
#[cfg(test)]
#[path = "../tests/unit/closure_digest.rs"]
mod closure_digest_tests;
