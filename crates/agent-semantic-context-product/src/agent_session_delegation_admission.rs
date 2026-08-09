use serde::{Deserialize, Serialize};

pub const AGENT_SESSION_DELEGATION_ADMISSION_SCHEMA_ID: &str =
    "agent.semantic-protocols.agent-session-delegation-admission";
pub const AGENT_SESSION_DELEGATION_ADMISSION_SCHEMA_VERSION: &str = "1";

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum AgentSessionDelegationCapability {
    Standard,
    FocusedLeaf,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum AgentSessionDelegationDecision {
    Accepted,
    Denied,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentSessionDelegationAdmissionReceipt {
    pub schema_id: String,
    pub schema_version: String,
    pub project_id: String,
    pub root_session_id: String,
    pub current_session_id: String,
    pub proposed_child_session_id: String,
    pub capability: AgentSessionDelegationCapability,
    pub decision: AgentSessionDelegationDecision,
    pub reason_kind: Option<String>,
    pub pre_state_digest: String,
    pub post_state_digest: String,
    pub delegation_generation: Option<u64>,
    pub evidence_refs: Vec<String>,
}

pub struct AgentSessionDelegationAdmissionInput {
    pub project_id: String,
    pub root_session_id: String,
    pub current_session_id: String,
    pub proposed_child_session_id: String,
    pub capability: AgentSessionDelegationCapability,
    pub decision: AgentSessionDelegationDecision,
    pub reason_kind: Option<String>,
    pub pre_state_digest: String,
    pub post_state_digest: String,
    pub delegation_generation: Option<u64>,
    pub evidence_refs: Vec<String>,
}

impl AgentSessionDelegationAdmissionReceipt {
    pub fn focused_leaf_denied(
        project_id: impl Into<String>,
        root_session_id: impl Into<String>,
        current_session_id: impl Into<String>,
        proposed_child_session_id: impl Into<String>,
        state_digest: impl Into<String>,
        evidence_refs: Vec<String>,
    ) -> Result<Self, String> {
        let state_digest = state_digest.into();
        Self::new(AgentSessionDelegationAdmissionInput {
            project_id: project_id.into(),
            root_session_id: root_session_id.into(),
            current_session_id: current_session_id.into(),
            proposed_child_session_id: proposed_child_session_id.into(),
            capability: AgentSessionDelegationCapability::FocusedLeaf,
            decision: AgentSessionDelegationDecision::Denied,
            reason_kind: Some("focused-agent-delegation-denied".to_owned()),
            pre_state_digest: state_digest.clone(),
            post_state_digest: state_digest,
            delegation_generation: None,
            evidence_refs,
        })
    }

    pub fn new(input: AgentSessionDelegationAdmissionInput) -> Result<Self, String> {
        let receipt = Self {
            schema_id: AGENT_SESSION_DELEGATION_ADMISSION_SCHEMA_ID.to_owned(),
            schema_version: AGENT_SESSION_DELEGATION_ADMISSION_SCHEMA_VERSION.to_owned(),
            project_id: input.project_id,
            root_session_id: input.root_session_id,
            current_session_id: input.current_session_id,
            proposed_child_session_id: input.proposed_child_session_id,
            capability: input.capability,
            decision: input.decision,
            reason_kind: input.reason_kind,
            pre_state_digest: input.pre_state_digest,
            post_state_digest: input.post_state_digest,
            delegation_generation: input.delegation_generation,
            evidence_refs: input.evidence_refs,
        };
        receipt.validate()?;
        Ok(receipt)
    }

    pub fn validate(&self) -> Result<(), String> {
        for (field, value) in [
            ("projectId", self.project_id.as_str()),
            ("rootSessionId", self.root_session_id.as_str()),
            ("currentSessionId", self.current_session_id.as_str()),
            (
                "proposedChildSessionId",
                self.proposed_child_session_id.as_str(),
            ),
            ("preStateDigest", self.pre_state_digest.as_str()),
            ("postStateDigest", self.post_state_digest.as_str()),
        ] {
            if value.trim().is_empty() {
                return Err(format!("{field} must be non-empty text"));
            }
        }

        if self
            .evidence_refs
            .iter()
            .any(|value| value.trim().is_empty())
        {
            return Err("evidenceRefs must contain only non-empty text".to_owned());
        }

        match self.decision {
            AgentSessionDelegationDecision::Accepted => {
                if self.reason_kind.is_some() {
                    return Err("accepted delegation cannot carry reasonKind".to_owned());
                }
                if self.delegation_generation.is_none() {
                    return Err("accepted delegation requires delegationGeneration".to_owned());
                }
            }
            AgentSessionDelegationDecision::Denied => {
                if self.reason_kind.as_deref().is_none_or(str::is_empty) {
                    return Err("denied delegation requires reasonKind".to_owned());
                }
                if self.delegation_generation.is_some() {
                    return Err("denied delegation cannot carry delegationGeneration".to_owned());
                }
                if self.pre_state_digest != self.post_state_digest {
                    return Err("denied delegation must preserve control-plane state".to_owned());
                }
                if self.capability == AgentSessionDelegationCapability::FocusedLeaf
                    && self.reason_kind.as_deref() != Some("focused-agent-delegation-denied")
                {
                    return Err(
                        "focused-leaf denial requires focused-agent-delegation-denied".to_owned(),
                    );
                }
            }
        }
        Ok(())
    }
}
