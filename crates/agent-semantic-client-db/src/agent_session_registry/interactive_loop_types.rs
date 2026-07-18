//! Serializable contract types for the resident lifecycle choice pane.

use agent_semantic_loop::{Choice, HostRequirement, LoopReceipt, TraceStep};
use serde::Serialize;

use super::types::AgentSessionRecord;

#[derive(Serialize)]
pub struct AgentSessionInteractiveMenu<'a> {
    #[serde(rename = "schemaId")]
    pub schema_id: &'a str,
    #[serde(rename = "schemaVersion")]
    pub schema_version: &'a str,
    pub owner: &'a str,
    pub state: AgentSessionLoopState,
    pub name: &'a str,
    #[serde(rename = "rootSessionId", skip_serializing_if = "Option::is_none")]
    pub root_session_id: Option<&'a str>,
    #[serde(rename = "expectedModel", skip_serializing_if = "Option::is_none")]
    pub expected_model: Option<&'a str>,
    #[serde(
        rename = "expectedReasoningEffort",
        skip_serializing_if = "Option::is_none"
    )]
    pub expected_reasoning_effort: Option<&'a str>,
    #[serde(
        rename = "rolloutHistoryStatus",
        skip_serializing_if = "Option::is_none"
    )]
    pub rollout_history_status: Option<&'a str>,
    #[serde(
        rename = "rolloutHistoryAction",
        skip_serializing_if = "Option::is_none"
    )]
    pub rollout_history_action: Option<&'a str>,
    #[serde(rename = "session", skip_serializing_if = "Option::is_none")]
    pub session: Option<AgentSessionInteractiveSession<'a>>,
    #[serde(rename = "hostRequirement")]
    pub host_requirement: AgentSessionHostRequirement<'a>,
    pub trace: Vec<AgentSessionLoopTraceStep<'a>>,
    pub choices: Vec<AgentSessionInteractiveChoice<'a>>,
    pub receipt: AgentSessionInteractiveReceipt<'a>,
}

#[derive(Serialize)]
pub struct AgentSessionInteractiveSession<'a> {
    #[serde(rename = "childSessionId")]
    pub child_session_id: &'a str,
    pub status: &'a str,
    pub role: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<&'a str>,
    #[serde(
        rename = "modelObservationSource",
        skip_serializing_if = "Option::is_none"
    )]
    pub model_observation_source: Option<&'a str>,
    #[serde(rename = "modelObservedAt", skip_serializing_if = "Option::is_none")]
    pub model_observed_at: Option<i64>,
    #[serde(rename = "modelEvidenceRef", skip_serializing_if = "Option::is_none")]
    pub model_evidence_ref: Option<&'a str>,
    #[serde(rename = "messageTargetStatus")]
    pub message_target_status: &'a str,
    #[serde(rename = "messageTargetId", skip_serializing_if = "Option::is_none")]
    pub message_target_id: Option<&'a str>,
}

pub type AgentSessionInteractiveChoice<'a> = Choice<'a, AgentSessionLoopState>;
pub type AgentSessionHostRequirement<'a> = HostRequirement<'a>;
pub type AgentSessionLoopTraceStep<'a> = TraceStep<'a, AgentSessionLoopState>;
pub type AgentSessionInteractiveReceipt<'a> = LoopReceipt<'a>;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
pub enum AgentSessionLoopState {
    Audit,
    Classify,
    Recover,
    RebindExistingChildTarget,
    Repair,
    Blocked,
    Adopt,
    Cleanup,
    Create,
    Register,
    Validate,
    Ready,
    SendDeniedCommand,
    WaitReceipt,
    RetryOriginal,
}

pub struct ResidentChildBootstrapMenuInput<'a> {
    pub platform: &'a str,
    pub name: &'a str,
    pub root_session_id: Option<&'a str>,
    pub record: Option<&'a AgentSessionRecord>,
    pub expected_model: Option<&'a str>,
    pub expected_reasoning_effort: Option<&'a str>,
    pub rollout_history_status: Option<&'a str>,
    pub rollout_history_action: Option<&'a str>,
    pub now: i64,
}
