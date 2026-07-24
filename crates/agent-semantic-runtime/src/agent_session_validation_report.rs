//! Validation report shared by agent-session registry logic and renderers.

use serde::Serialize;

macro_rules! validation_report_text {
    ($(#[$meta:meta])* $name:ident) => {
        $(#[$meta])*
        #[derive(Clone, Debug, Eq, PartialEq, Serialize)]
        #[serde(transparent)]
        pub struct $name(String);

        impl $name {
            #[allow(dead_code)]
            pub fn as_str(&self) -> &str {
                &self.0
            }
        }

        impl From<String> for $name {
            fn from(value: String) -> Self {
                Self(value)
            }
        }

        impl From<&str> for $name {
            fn from(value: &str) -> Self {
                Self(value.to_owned())
            }
        }
    };
}

validation_report_text!(AgentSessionValidationStatus);

/// Validation result for routing one registered agent session.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentSessionValidationReport {
    /// Validation status such as `passed`, `warning`, `failed`, or `skipped`.
    status: AgentSessionValidationStatus,
    /// Human-readable validation reason.
    reason: String,
    /// Canonical agent config path used for validation.
    #[serde(rename = "configPath", skip_serializing_if = "Option::is_none")]
    config_path: Option<String>,
    /// Codex rollout path used for validation.
    #[serde(rename = "rolloutPath", skip_serializing_if = "Option::is_none")]
    rollout_path: Option<String>,
    /// Expected root session id.
    #[serde(
        rename = "expectedRootSessionId",
        skip_serializing_if = "Option::is_none"
    )]
    expected_root_session_id: Option<String>,
    /// Actual root session id from rollout metadata.
    #[serde(
        rename = "actualRootSessionId",
        skip_serializing_if = "Option::is_none"
    )]
    actual_root_session_id: Option<String>,
    /// Expected Codex parent thread id.
    #[serde(
        rename = "expectedParentThreadId",
        skip_serializing_if = "Option::is_none"
    )]
    expected_parent_thread_id: Option<String>,
    /// Actual Codex parent thread id.
    #[serde(
        rename = "actualParentThreadId",
        skip_serializing_if = "Option::is_none"
    )]
    actual_parent_thread_id: Option<String>,
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
    /// Expected model reasoning effort.
    #[serde(
        rename = "expectedReasoningEffort",
        skip_serializing_if = "Option::is_none"
    )]
    pub expected_reasoning_effort: Option<String>,
    /// Actual model reasoning effort from rollout metadata.
    #[serde(
        rename = "actualReasoningEffort",
        skip_serializing_if = "Option::is_none"
    )]
    pub actual_reasoning_effort: Option<String>,
    /// Expected sandbox policy.
    #[serde(rename = "expectedSandbox", skip_serializing_if = "Option::is_none")]
    pub expected_sandbox: Option<String>,
    /// Actual sandbox policy.
    #[serde(rename = "actualSandbox", skip_serializing_if = "Option::is_none")]
    pub actual_sandbox: Option<String>,
}
