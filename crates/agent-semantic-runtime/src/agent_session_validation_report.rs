//! Validation report shared by agent-session registry logic and renderers.

use serde::Serialize;

/// Validation result for routing one registered agent session.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentSessionValidationReport {
    /// Validation status such as `passed`, `warning`, `failed`, or `skipped`.
    pub status: String,
    /// Human-readable validation reason.
    pub reason: String,
    /// Canonical agent config path used for validation.
    #[serde(rename = "configPath", skip_serializing_if = "Option::is_none")]
    pub config_path: Option<String>,
    /// Codex rollout path used for validation.
    #[serde(rename = "rolloutPath", skip_serializing_if = "Option::is_none")]
    pub rollout_path: Option<String>,
    /// Expected root session id.
    #[serde(
        rename = "expectedRootSessionId",
        skip_serializing_if = "Option::is_none"
    )]
    pub expected_root_session_id: Option<String>,
    /// Actual root session id from rollout metadata.
    #[serde(
        rename = "actualRootSessionId",
        skip_serializing_if = "Option::is_none"
    )]
    pub actual_root_session_id: Option<String>,
    /// Expected Codex parent thread id.
    #[serde(
        rename = "expectedParentThreadId",
        skip_serializing_if = "Option::is_none"
    )]
    pub expected_parent_thread_id: Option<String>,
    /// Actual Codex parent thread id.
    #[serde(
        rename = "actualParentThreadId",
        skip_serializing_if = "Option::is_none"
    )]
    pub actual_parent_thread_id: Option<String>,
    /// Expected configured agent path.
    #[serde(rename = "expectedAgentPath", skip_serializing_if = "Option::is_none")]
    pub expected_agent_path: Option<String>,
    /// Actual configured agent path.
    #[serde(rename = "actualAgentPath", skip_serializing_if = "Option::is_none")]
    pub actual_agent_path: Option<String>,
    /// Expected agent role.
    #[serde(rename = "expectedRole", skip_serializing_if = "Option::is_none")]
    pub expected_role: Option<String>,
    /// Actual agent role.
    #[serde(rename = "actualRole", skip_serializing_if = "Option::is_none")]
    pub actual_role: Option<String>,
    /// Expected model.
    #[serde(rename = "expectedModel", skip_serializing_if = "Option::is_none")]
    pub expected_model: Option<String>,
    /// Actual model.
    #[serde(rename = "actualModel", skip_serializing_if = "Option::is_none")]
    pub actual_model: Option<String>,
    /// Expected sandbox policy.
    #[serde(rename = "expectedSandbox", skip_serializing_if = "Option::is_none")]
    pub expected_sandbox: Option<String>,
    /// Actual sandbox policy.
    #[serde(rename = "actualSandbox", skip_serializing_if = "Option::is_none")]
    pub actual_sandbox: Option<String>,
}
