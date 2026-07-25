use super::{Digest, ProtocolId};
use serde::{Deserialize, Serialize};

pub const CONTEXT_PRODUCT_SCHEMA_ID: &str = "asp.context-product-state";
pub const CONTEXT_PRODUCT_SCHEMA_VERSION: u32 = 1;
pub const CONTEXT_PRODUCT_CANONICALIZATION_PROFILE: &str = "jcs-rfc8785-blake3-256-v1";
pub const JSON_SAFE_INTEGER_MAX: u64 = 9_007_199_254_740_991;

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ContextBinding {
    pub workspace_id: ProtocolId,
    pub workspace_root_digest: Digest,
    pub base_source_digest: Digest,
    pub dirty_overlay_digest: Digest,
    pub source_snapshot_digest: Digest,
    pub provider_digest: Digest,
    pub parser_digest: Digest,
    pub query_pack_digest: Digest,
    pub fact_schema_digest: Digest,
    pub projection_schema_digest: Digest,
    pub policy_digest: Digest,
    pub graph_revision_digest: Digest,
    pub context_epoch: u64,
    pub binding_digest: Digest,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum ClaimClass {
    Identity,
    Enumeration,
    Absence,
    CausalFlow,
    Handoff,
    Quality,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum EvidenceVerdict {
    Supported,
    Refuted,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(
    tag = "kind",
    rename_all = "kebab-case",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum ObligationDisposition {
    Open {
        reason_code: ProtocolId,
    },
    Resolved {
        verdict: EvidenceVerdict,
        proof_refs: Vec<ProtocolId>,
    },
    RefreshRequired {
        reason_code: ProtocolId,
        invalidated_proof_refs: Vec<ProtocolId>,
    },
    Blocked {
        blocker_code: ProtocolId,
        retryable: bool,
    },
}

impl ObligationDisposition {
    pub(crate) fn is_resolved(&self) -> bool {
        matches!(self, Self::Resolved { .. })
    }

    pub(crate) fn is_open_for_closure(&self) -> bool {
        !self.is_resolved()
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Obligation {
    pub obligation_id: ProtocolId,
    pub claim_class: ClaimClass,
    pub claim_digest: Digest,
    pub required: bool,
    pub depends_on: Vec<ProtocolId>,
    pub disposition: ObligationDisposition,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum ProofReuseMode {
    Direct,
    Rebased,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RetainedProof {
    pub proof_ref: ProtocolId,
    pub mode: ProofReuseMode,
    pub context_binding_digest: Digest,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rebase_certificate_ref: Option<ProtocolId>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(
    tag = "kind",
    rename_all = "kebab-case",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum ProofReuse {
    None {
        retained: Vec<RetainedProof>,
        invalidated_proof_refs: Vec<ProtocolId>,
    },
    Partial {
        retained: Vec<RetainedProof>,
        invalidated_proof_refs: Vec<ProtocolId>,
    },
    Complete {
        retained: Vec<RetainedProof>,
        invalidated_proof_refs: Vec<ProtocolId>,
    },
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct FrontierNode {
    pub node_id: ProtocolId,
    pub program_id: ProtocolId,
    pub stage_id: ProtocolId,
    pub depth: u64,
    pub predecessor_ids: Vec<ProtocolId>,
    pub ancestor_node_ids: Vec<ProtocolId>,
    pub open_obligation_ids: Vec<ProtocolId>,
    pub proof_refs: Vec<ProtocolId>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct FrontierAntichain {
    pub frontier_id: ProtocolId,
    pub graph_digest: Digest,
    pub context_binding_digest: Digest,
    pub nodes: Vec<FrontierNode>,
    pub frontier_digest: Digest,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SearchBudget {
    pub max_commands: u64,
    pub max_elapsed_ms: u64,
    pub max_packet_bytes: u64,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(
    tag = "kind",
    rename_all = "kebab-case",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum DecisionRequirement {
    None,
    Deterministic {
        next_stage_id: ProtocolId,
    },
    Search {
        open_obligation_ids: Vec<ProtocolId>,
        evidence_plan_ref: ProtocolId,
        budget: SearchBudget,
    },
    Reason {
        checkpoint_id: ProtocolId,
        choice_ids: Vec<ProtocolId>,
    },
    Clarify {
        question_id: ProtocolId,
        input_schema_ref: ProtocolId,
    },
    Blocked {
        blocker_code: ProtocolId,
        retryable: bool,
    },
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(
    tag = "kind",
    rename_all = "kebab-case",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum ActiveProgram {
    None,
    Admitted {
        proposal_id: ProtocolId,
        proposal_digest: Digest,
        intent_digest: Digest,
        program_id: ProtocolId,
        program_digest: Digest,
        program: super::RouteProgram,
        graph_digest: Digest,
        admitted_at_revision: u64,
        context_binding_digest: Digest,
    },
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(
    tag = "kind",
    rename_all = "kebab-case",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum ExecutionAuthority {
    None,
    Admitted {
        admission_id: ProtocolId,
        admission_digest: Digest,
        action_key: Digest,
        program_digest: Digest,
        stage_id: ProtocolId,
        provider_id: ProtocolId,
        operation: ProtocolId,
        resolved_input_digest: Digest,
        policy_digest: Digest,
        dependency_digest: Digest,
        budget_reservation_id: ProtocolId,
        budget_charge_key: Digest,
    },
    Granted {
        admission_id: ProtocolId,
        admission_digest: Digest,
        grant_id: ProtocolId,
        grant_digest: Digest,
        action_key: Digest,
        program_digest: Digest,
        stage_id: ProtocolId,
        provider_id: ProtocolId,
        operation: ProtocolId,
        resolved_input_digest: Digest,
        policy_digest: Digest,
        dependency_digest: Digest,
        budget_reservation_id: ProtocolId,
        budget_charge_key: Digest,
        lease_fence: u64,
        expires_at_ms: u64,
        effect_class: super::EffectClass,
        provider_idempotency_key: ProtocolId,
    },
    InFlight {
        admission_id: ProtocolId,
        admission_digest: Digest,
        grant_id: ProtocolId,
        grant_digest: Digest,
        action_key: Digest,
        attempt_id: ProtocolId,
        attempt_digest: Digest,
        provider_id: ProtocolId,
        operation: ProtocolId,
        program_digest: Digest,
        stage_id: ProtocolId,
        resolved_input_digest: Digest,
        policy_digest: Digest,
        dependency_digest: Digest,
        budget_reservation_id: ProtocolId,
        budget_charge_key: Digest,
        effect_class: super::EffectClass,
        lease_fence: u64,
        expires_at_ms: u64,
        provider_idempotency_key: ProtocolId,
    },
    Consumed {
        admission_id: ProtocolId,
        grant_id: ProtocolId,
        grant_digest: Digest,
        action_key: Digest,
        attempt_id: ProtocolId,
        result_receipt_ref: ProtocolId,
        result_digest: Digest,
        lease_fence: u64,
    },
    Revoked {
        reason_code: ProtocolId,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        admission_id: Option<ProtocolId>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        grant_id: Option<ProtocolId>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        action_key: Option<Digest>,
    },
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(
    tag = "kind",
    rename_all = "kebab-case",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum ClosureDisposition {
    Open {
        open_obligation_ids: Vec<ProtocolId>,
    },
    Partial {
        closed_obligation_ids: Vec<ProtocolId>,
        open_obligation_ids: Vec<ProtocolId>,
    },
    Finalized {
        receipt_id: ProtocolId,
        receipt_digest: Digest,
    },
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct UncheckedContextProductStateV1 {
    pub schema_id: String,
    pub schema_version: u32,
    pub run_id: ProtocolId,
    pub revision: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub previous_state_digest: Option<Digest>,
    pub last_event_sequence: u64,
    pub event_log_digest: Digest,
    pub active_program: ActiveProgram,
    pub spent_action_keys: Vec<Digest>,
    pub spent_action_ledger_digest: Digest,
    pub authority_receipt_ref: ProtocolId,
    pub canonicalization_profile: String,
    pub context: ContextBinding,
    pub obligations: Vec<Obligation>,
    pub proof_reuse: ProofReuse,
    pub frontier: FrontierAntichain,
    pub decision: DecisionRequirement,
    pub execution: ExecutionAuthority,
    pub closure: ClosureDisposition,
    pub state_digest: Digest,
}
