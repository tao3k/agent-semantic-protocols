use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

use crate::agent_session_delegation_admission::AgentSessionDelegationCapability;

pub const AGENT_SESSION_DELEGATION_INTENT_SCHEMA_ID: &str =
    "agent.semantic-protocols.agent-session-delegation-intent";
pub const AGENT_SESSION_DELEGATION_INTENT_SCHEMA_VERSION: &str = "1";

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentSessionDelegationIntent {
    pub schema_id: String,
    pub schema_version: String,
    pub event_id: String,
    pub project_id: String,
    pub root_session_id: String,
    pub current_session_id: String,
    pub proposed_child_session_id: String,
    pub proposed_child_resident_name: String,
    pub proposed_child_capability: AgentSessionDelegationCapability,
    pub evidence_refs: Vec<String>,
    pub observed_at_ms: i64,
}

impl AgentSessionDelegationIntent {
    pub fn validate(&self) -> Result<(), String> {
        if self.schema_id != AGENT_SESSION_DELEGATION_INTENT_SCHEMA_ID {
            return Err("delegation intent schemaId mismatch".to_owned());
        }
        if self.schema_version != AGENT_SESSION_DELEGATION_INTENT_SCHEMA_VERSION {
            return Err("delegation intent schemaVersion mismatch".to_owned());
        }
        for (field, value) in [
            ("eventId", self.event_id.as_str()),
            ("projectId", self.project_id.as_str()),
            ("rootSessionId", self.root_session_id.as_str()),
            ("currentSessionId", self.current_session_id.as_str()),
            (
                "proposedChildSessionId",
                self.proposed_child_session_id.as_str(),
            ),
            (
                "proposedChildResidentName",
                self.proposed_child_resident_name.as_str(),
            ),
        ] {
            if value.trim().is_empty() {
                return Err(format!("delegation intent {field} must be non-empty"));
            }
        }
        if self.observed_at_ms < 0 {
            return Err("delegation intent observedAtMs must be non-negative".to_owned());
        }
        if self
            .evidence_refs
            .iter()
            .any(|evidence| evidence.trim().is_empty())
        {
            return Err("delegation intent evidenceRefs must be non-empty strings".to_owned());
        }
        let unique_evidence = self.evidence_refs.iter().collect::<BTreeSet<_>>();
        if unique_evidence.len() != self.evidence_refs.len() {
            return Err("delegation intent evidenceRefs must be unique".to_owned());
        }
        Ok(())
    }
}
