use super::{
    ClaimClass, Digest, EvidenceVerdict, ProtocolId, SearchBudget, UncheckedContextProductStateV1,
};
use serde::{Deserialize, Serialize};
use std::fmt;

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct EvidencePredicate {
    pub predicate_id: ProtocolId,
    pub claim_class: ClaimClass,
    pub scope_digest: Digest,
    pub accepted_schema_ids: Vec<ProtocolId>,
    pub required_fields: Vec<ProtocolId>,
    pub completeness: EvidenceCompleteness,
    pub requires_fresh_binding: bool,
    pub requires_untruncated: bool,
    pub max_results: u64,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum EvidenceCompleteness {
    IdentityOnly,
    CompleteScope,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum RouteActionClass {
    ProviderSearch,
    ProviderQuery,
    ReasoningCheckpoint,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum RequiredClosure {
    Discovery,
    Existential,
    Enumeration,
    NegativeCompleteness,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RouteNode {
    pub node_id: ProtocolId,
    pub action_class: RouteActionClass,
    pub provider_id: ProtocolId,
    pub language_id: ProtocolId,
    pub catalog_id: ProtocolId,
    pub input_template_digest: Digest,
    pub covers_obligation_ids: Vec<ProtocolId>,
    pub depends_on_node_ids: Vec<ProtocolId>,
    pub required_closure: RequiredClosure,
    pub evidence_predicate: EvidencePredicate,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RouteEdge {
    pub from: ProtocolId,
    pub to: ProtocolId,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RouteProposal {
    pub proposal_id: ProtocolId,
    pub run_id: ProtocolId,
    pub based_on_revision: u64,
    pub context_binding_digest: Digest,
    pub intent_digest: Digest,
    pub catalog_digest: Digest,
    pub proposed_by: ProtocolId,
    pub agent_reasoning_receipt_ref: ProtocolId,
    pub graph_derivation_receipt_ref: ProtocolId,
    pub obligation_ids: Vec<ProtocolId>,
    pub nodes: Vec<RouteNode>,
    pub edges: Vec<RouteEdge>,
    pub execution_groups: Vec<RouteProposalExecutionGroup>,
    pub joins: Vec<RouteProposalJoin>,
    pub estimate_receipt_refs: Vec<ProtocolId>,
    pub budget_proposal: SearchBudget,
    pub proposal_digest: Digest,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum RouteExecutionMode {
    Batch,
    Serial,
    Parallel,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RouteProposalExecutionGroup {
    pub group_id: ProtocolId,
    pub mode: RouteExecutionMode,
    pub node_ids: Vec<ProtocolId>,
    pub join_policy: JoinPolicy,
    pub max_parallel: u64,
    pub derivation_receipt_ref: ProtocolId,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub batch_capability_ref: Option<ProtocolId>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub independence_proof_ref: Option<ProtocolId>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RouteProposalJoin {
    pub join_id: ProtocolId,
    pub required_node_ids: Vec<ProtocolId>,
    pub continuation_node_id: ProtocolId,
    pub policy: JoinPolicy,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RouteStage {
    pub stage_id: ProtocolId,
    pub proposal_node_id: ProtocolId,
    pub provider_id: ProtocolId,
    pub catalog_id: ProtocolId,
    pub input_template_digest: Digest,
    pub covers_obligation_ids: Vec<ProtocolId>,
    pub required_closure: RequiredClosure,
    pub evidence_predicate: EvidencePredicate,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum JoinPolicy {
    AllRequired,
    FirstExact,
    CompleteEnumeration,
    DisambiguateAll,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RouteExecutionGroup {
    pub group_id: ProtocolId,
    pub mode: RouteExecutionMode,
    pub stage_ids: Vec<ProtocolId>,
    pub join_policy: JoinPolicy,
    pub max_parallel: u64,
    pub derivation_receipt_ref: ProtocolId,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub batch_capability_ref: Option<ProtocolId>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub independence_proof_ref: Option<ProtocolId>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RouteJoin {
    pub join_id: ProtocolId,
    pub required_stage_ids: Vec<ProtocolId>,
    pub continuation_stage_id: ProtocolId,
    pub policy: JoinPolicy,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RouteProgram {
    pub program_id: ProtocolId,
    pub proposal_id: ProtocolId,
    pub run_id: ProtocolId,
    pub admitted_at_revision: u64,
    pub context_binding_digest: Digest,
    pub graph_digest: Digest,
    pub catalog_digest: Digest,
    pub stages: Vec<RouteStage>,
    pub execution_groups: Vec<RouteExecutionGroup>,
    pub edges: Vec<RouteEdge>,
    pub joins: Vec<RouteJoin>,
    pub budget_limit: SearchBudget,
    pub program_digest: Digest,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum ActionAdmittedEventType {
    #[serde(rename = "ActionAdmitted")]
    ActionAdmitted,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RouteProgramAdmitted {
    pub event_type: RouteProgramAdmittedEventType,
    pub event_id: ProtocolId,
    pub run_id: ProtocolId,
    pub sequence: u64,
    pub state_revision: u64,
    pub pre_state_digest: Digest,
    pub proposal_digest: Digest,
    pub program_digest: Digest,
    pub context_binding_digest: Digest,
    pub event_digest: Digest,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum RouteProgramAdmittedEventType {
    #[serde(rename = "RouteProgramAdmitted")]
    RouteProgramAdmitted,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ActionAdmitted {
    pub event_type: ActionAdmittedEventType,
    pub event_id: ProtocolId,
    pub run_id: ProtocolId,
    pub sequence: u64,
    pub state_revision: u64,
    pub pre_state_digest: Digest,
    pub admission_id: ProtocolId,
    pub program_id: ProtocolId,
    pub execution_group_id: ProtocolId,
    pub stage_ids: Vec<ProtocolId>,
    pub context_binding_digest: Digest,
    pub resolved_input_digest: Digest,
    pub action_key: Digest,
    pub policy_digest: Digest,
    pub dependency_digest: Digest,
    pub budget_reservation_id: ProtocolId,
    pub budget_charge_key: Digest,
    pub admission_digest: Digest,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum EffectClass {
    ReadOnly,
    IdempotentKeyed,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum ExecutionGrantIssuedEventType {
    #[serde(rename = "ExecutionGrantIssued")]
    ExecutionGrantIssued,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ExecutionGrantIssued {
    pub event_type: ExecutionGrantIssuedEventType,
    pub event_id: ProtocolId,
    pub run_id: ProtocolId,
    pub sequence: u64,
    pub state_revision: u64,
    pub pre_state_digest: Digest,
    pub grant_id: ProtocolId,
    pub admission_id: ProtocolId,
    pub admission_digest: Digest,
    pub program_id: ProtocolId,
    pub program_digest: Digest,
    pub execution_group_id: ProtocolId,
    pub stage_ids: Vec<ProtocolId>,
    pub action_key: Digest,
    pub context_binding_digest: Digest,
    pub resolved_input_digest: Digest,
    pub provider_id: ProtocolId,
    pub operation: ProtocolId,
    pub lease_fence: u64,
    pub issued_at_ms: u64,
    pub expires_at_ms: u64,
    pub effect_class: EffectClass,
    pub single_use: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider_idempotency_key: Option<ProtocolId>,
    pub grant_digest: Digest,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct EvidenceReceipt {
    pub proof_ref: ProtocolId,
    pub claim_class: ClaimClass,
    pub claim_digest: Digest,
    pub verdict: EvidenceVerdict,
    pub context_binding_digest: Digest,
    pub source_snapshot_digest: Digest,
    pub provider_digest: Digest,
    pub parser_digest: Digest,
    pub query_pack_digest: Digest,
    pub scope_digest: Digest,
    pub support_closure_digest: Digest,
    pub freshness_receipt_ref: ProtocolId,
    pub packet_digest: Digest,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct EvidenceScope {
    pub scope_digest: Digest,
    pub complete: bool,
    pub truncated: bool,
    pub snapshot_continuous: bool,
    pub provider_admitted: bool,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ClosureProof {
    pub obligation_id: ProtocolId,
    pub claim_class: ClaimClass,
    pub verdict: EvidenceVerdict,
    pub proof_refs: Vec<ProtocolId>,
    pub support_paths: Vec<String>,
    pub evidence_scope: EvidenceScope,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum ClosureFinalizedEventType {
    #[serde(rename = "ClosureFinalized")]
    ClosureFinalized,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SearchClosureReceipt {
    pub event_type: ClosureFinalizedEventType,
    pub receipt_id: ProtocolId,
    pub event_id: ProtocolId,
    pub run_id: ProtocolId,
    pub sequence: u64,
    pub state_revision: u64,
    pub pre_state_digest: Digest,
    pub context_binding_digest: Digest,
    pub intent_digest: Digest,
    pub route_program_id: ProtocolId,
    pub program_digest: Digest,
    pub closed_obligations: Vec<ClosureProof>,
    pub finalized_at_ms: u64,
    pub receipt_digest: Digest,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct StateAuthorityReceipt {
    pub receipt_id: ProtocolId,
    pub authority_id: ProtocolId,
    pub run_id: ProtocolId,
    pub revision: u64,
    pub state_digest: Digest,
    pub event_log_digest: Digest,
    pub issued_at_ms: u64,
    pub receipt_digest: Digest,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ExecutionStarted {
    pub event_type: ExecutionStartedEventType,
    pub event_id: ProtocolId,
    pub run_id: ProtocolId,
    pub sequence: u64,
    pub state_revision: u64,
    pub pre_state_digest: Digest,
    pub grant_id: ProtocolId,
    pub grant_digest: Digest,
    pub attempt_id: ProtocolId,
    pub attempt_digest: Digest,
    pub execution_group_id: ProtocolId,
    pub stage_ids: Vec<ProtocolId>,
    pub provider_id: ProtocolId,
    pub operation: ProtocolId,
    pub lease_fence: u64,
    pub expires_at_ms: u64,
    pub provider_idempotency_key: ProtocolId,
    pub event_digest: Digest,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum ExecutionStartedEventType {
    #[serde(rename = "ExecutionStarted")]
    ExecutionStarted,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ExecutionConsumed {
    pub event_type: ExecutionConsumedEventType,
    pub event_id: ProtocolId,
    pub run_id: ProtocolId,
    pub sequence: u64,
    pub state_revision: u64,
    pub pre_state_digest: Digest,
    pub grant_id: ProtocolId,
    pub grant_digest: Digest,
    pub attempt_id: ProtocolId,
    pub execution_group_id: ProtocolId,
    pub stage_ids: Vec<ProtocolId>,
    pub provider_id: ProtocolId,
    pub operation: ProtocolId,
    pub result_receipt_ref: ProtocolId,
    pub result_digest: Digest,
    pub lease_fence: u64,
    pub event_digest: Digest,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum ExecutionConsumedEventType {
    #[serde(rename = "ExecutionConsumed")]
    ExecutionConsumed,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ExecutionRevoked {
    pub event_type: ExecutionRevokedEventType,
    pub event_id: ProtocolId,
    pub run_id: ProtocolId,
    pub sequence: u64,
    pub state_revision: u64,
    pub pre_state_digest: Digest,
    pub reason_code: ProtocolId,
    pub execution_group_id: ProtocolId,
    pub stage_ids: Vec<ProtocolId>,
    pub provider_id: ProtocolId,
    pub operation: ProtocolId,
    pub admission_id: Option<ProtocolId>,
    pub grant_id: Option<ProtocolId>,
    pub action_key: Option<Digest>,
    pub event_digest: Digest,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum ExecutionRevokedEventType {
    #[serde(rename = "ExecutionRevoked")]
    ExecutionRevoked,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum ExecutionGroupJoinedEventType {
    #[serde(rename = "ExecutionGroupJoined")]
    ExecutionGroupJoined,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ExecutionGroupJoined {
    pub event_type: ExecutionGroupJoinedEventType,
    pub event_id: ProtocolId,
    pub run_id: ProtocolId,
    pub sequence: u64,
    pub state_revision: u64,
    pub pre_state_digest: Digest,
    pub program_digest: Digest,
    pub execution_group_id: ProtocolId,
    pub policy: JoinPolicy,
    pub result_receipt_refs: Vec<ProtocolId>,
    pub joined_group_digest: Digest,
    pub event_digest: Digest,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(untagged)]
pub enum ContextProductEvent {
    RouteProgramAdmitted(RouteProgramAdmitted),
    ActionAdmitted(ActionAdmitted),
    ExecutionGrantIssued(ExecutionGrantIssued),
    ExecutionStarted(ExecutionStarted),
    ExecutionConsumed(ExecutionConsumed),
    ExecutionRevoked(ExecutionRevoked),
    ExecutionGroupJoined(ExecutionGroupJoined),
    ClosureFinalized(SearchClosureReceipt),
}

impl ContextProductEvent {
    pub fn run_id(&self) -> &ProtocolId {
        match self {
            Self::RouteProgramAdmitted(event) => &event.run_id,
            Self::ActionAdmitted(event) => &event.run_id,
            Self::ExecutionGrantIssued(event) => &event.run_id,
            Self::ExecutionStarted(event) => &event.run_id,
            Self::ExecutionConsumed(event) => &event.run_id,
            Self::ExecutionRevoked(event) => &event.run_id,
            Self::ExecutionGroupJoined(event) => &event.run_id,
            Self::ClosureFinalized(event) => &event.run_id,
        }
    }

    pub fn event_id(&self) -> &ProtocolId {
        match self {
            Self::RouteProgramAdmitted(event) => &event.event_id,
            Self::ActionAdmitted(event) => &event.event_id,
            Self::ExecutionGrantIssued(event) => &event.event_id,
            Self::ExecutionStarted(event) => &event.event_id,
            Self::ExecutionConsumed(event) => &event.event_id,
            Self::ExecutionRevoked(event) => &event.event_id,
            Self::ExecutionGroupJoined(event) => &event.event_id,
            Self::ClosureFinalized(event) => &event.event_id,
        }
    }

    pub fn sequence(&self) -> u64 {
        match self {
            Self::RouteProgramAdmitted(event) => event.sequence,
            Self::ActionAdmitted(event) => event.sequence,
            Self::ExecutionGrantIssued(event) => event.sequence,
            Self::ExecutionStarted(event) => event.sequence,
            Self::ExecutionConsumed(event) => event.sequence,
            Self::ExecutionRevoked(event) => event.sequence,
            Self::ExecutionGroupJoined(event) => event.sequence,
            Self::ClosureFinalized(event) => event.sequence,
        }
    }

    pub fn digest(&self) -> &Digest {
        match self {
            Self::RouteProgramAdmitted(event) => &event.event_digest,
            Self::ActionAdmitted(event) => &event.admission_digest,
            Self::ExecutionGrantIssued(event) => &event.grant_digest,
            Self::ExecutionStarted(event) => &event.event_digest,
            Self::ExecutionConsumed(event) => &event.event_digest,
            Self::ExecutionRevoked(event) => &event.event_digest,
            Self::ExecutionGroupJoined(event) => &event.event_digest,
            Self::ClosureFinalized(event) => &event.receipt_digest,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RecommendedNextCandidate {
    pub language_id: ProtocolId,
    pub operation: ProtocolId,
    pub argv: Vec<String>,
    pub action_identity: Digest,
    pub context_binding_digest: Digest,
    pub covers_obligation_ids: Vec<ProtocolId>,
}

pub trait ParserOwnedCommandAdmission {
    type Error: fmt::Display;

    fn validate(
        &self,
        state: &UncheckedContextProductStateV1,
        candidate: &RecommendedNextCandidate,
    ) -> Result<(), Self::Error>;
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RecommendedNextAdmission {
    pub candidate: RecommendedNextCandidate,
}

#[cfg(test)]
#[path = "../tests/unit/protocol.rs"]
mod tests;
