//! Validation report shared by agent-session registry logic and renderers.

use serde::Serialize;

use crate::agent_session_status::RuntimeSessionId;

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

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(transparent)]
struct AgentSessionValidationReason(String);

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(transparent)]
struct AgentSessionValidationAgentPath(String);

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(transparent)]
struct AgentSessionValidationRole(String);

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(transparent)]
struct AgentSessionValidationModel(String);

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(transparent)]
struct AgentSessionValidationReasoningEffort(String);

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(transparent)]
struct AgentSessionValidationSandbox(String);

/// Validation result for routing one registered agent session.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentSessionValidationReport {
    /// Validation status such as `passed`, `warning`, `failed`, or `skipped`.
    status: AgentSessionValidationStatus,
    /// Human-readable validation reason.
    reason: AgentSessionValidationReason,
    /// Canonical agent config path used for validation.
    #[serde(rename = "configPath", skip_serializing_if = "Option::is_none")]
    config_path: Option<std::path::PathBuf>,
    /// Codex rollout path used for validation.
    #[serde(rename = "rolloutPath", skip_serializing_if = "Option::is_none")]
    rollout_path: Option<std::path::PathBuf>,
    /// Expected root session id.
    #[serde(
        rename = "expectedRootSessionId",
        skip_serializing_if = "Option::is_none"
    )]
    expected_root_session_id: Option<RuntimeSessionId>,
    /// Actual root session id from rollout metadata.
    #[serde(
        rename = "actualRootSessionId",
        skip_serializing_if = "Option::is_none"
    )]
    actual_root_session_id: Option<RuntimeSessionId>,
    /// Expected Codex parent thread id.
    #[serde(
        rename = "expectedParentThreadId",
        skip_serializing_if = "Option::is_none"
    )]
    expected_parent_thread_id: Option<RuntimeSessionId>,
    /// Actual Codex parent thread id.
    #[serde(
        rename = "actualParentThreadId",
        skip_serializing_if = "Option::is_none"
    )]
    actual_parent_thread_id: Option<RuntimeSessionId>,
    /// Expected configured agent path.
    #[serde(rename = "expectedAgentPath", skip_serializing_if = "Option::is_none")]
    expected_agent_path: Option<AgentSessionValidationAgentPath>,
    /// Actual configured agent path.
    #[serde(rename = "actualAgentPath", skip_serializing_if = "Option::is_none")]
    actual_agent_path: Option<AgentSessionValidationAgentPath>,
    /// Expected agent role.
    #[serde(rename = "expectedRole", skip_serializing_if = "Option::is_none")]
    expected_role: Option<AgentSessionValidationRole>,
    /// Actual agent role.
    #[serde(rename = "actualRole", skip_serializing_if = "Option::is_none")]
    actual_role: Option<AgentSessionValidationRole>,
    /// Expected model.
    #[serde(rename = "expectedModel", skip_serializing_if = "Option::is_none")]
    expected_model: Option<AgentSessionValidationModel>,
    /// Actual model.
    #[serde(rename = "actualModel", skip_serializing_if = "Option::is_none")]
    actual_model: Option<AgentSessionValidationModel>,
    /// Expected model reasoning effort.
    #[serde(
        rename = "expectedReasoningEffort",
        skip_serializing_if = "Option::is_none"
    )]
    expected_reasoning_effort: Option<AgentSessionValidationReasoningEffort>,
    /// Actual model reasoning effort from rollout metadata.
    #[serde(
        rename = "actualReasoningEffort",
        skip_serializing_if = "Option::is_none"
    )]
    actual_reasoning_effort: Option<AgentSessionValidationReasoningEffort>,
    /// Expected sandbox policy.
    #[serde(rename = "expectedSandbox", skip_serializing_if = "Option::is_none")]
    expected_sandbox: Option<AgentSessionValidationSandbox>,
    /// Actual sandbox policy.
    #[serde(rename = "actualSandbox", skip_serializing_if = "Option::is_none")]
    actual_sandbox: Option<AgentSessionValidationSandbox>,
}

impl AgentSessionValidationReport {
    pub fn new(status: AgentSessionValidationStatus, reason: impl Into<String>) -> Self {
        Self {
            status,
            reason: AgentSessionValidationReason(reason.into()),
            config_path: None,
            rollout_path: None,
            expected_root_session_id: None,
            actual_root_session_id: None,
            expected_parent_thread_id: None,
            actual_parent_thread_id: None,
            expected_agent_path: None,
            actual_agent_path: None,
            expected_role: None,
            actual_role: None,
            expected_model: None,
            actual_model: None,
            expected_reasoning_effort: None,
            actual_reasoning_effort: None,
            expected_sandbox: None,
            actual_sandbox: None,
        }
    }

    pub fn with_config_path(mut self, value: impl Into<std::path::PathBuf>) -> Self {
        self.config_path = Some(value.into());
        self
    }

    pub fn with_rollout_path(mut self, value: impl Into<std::path::PathBuf>) -> Self {
        self.rollout_path = Some(value.into());
        self
    }

    pub fn with_root_sessions(
        mut self,
        expected: Option<RuntimeSessionId>,
        actual: Option<RuntimeSessionId>,
    ) -> Self {
        self.expected_root_session_id = expected;
        self.actual_root_session_id = actual;
        self
    }

    pub fn with_parent_threads(
        mut self,
        expected: Option<RuntimeSessionId>,
        actual: Option<RuntimeSessionId>,
    ) -> Self {
        self.expected_parent_thread_id = expected;
        self.actual_parent_thread_id = actual;
        self
    }

    pub fn with_agent_paths(mut self, expected: Option<String>, actual: Option<String>) -> Self {
        self.expected_agent_path = expected.map(AgentSessionValidationAgentPath);
        self.actual_agent_path = actual.map(AgentSessionValidationAgentPath);
        self
    }

    pub fn with_roles(mut self, expected: Option<String>, actual: Option<String>) -> Self {
        self.expected_role = expected.map(AgentSessionValidationRole);
        self.actual_role = actual.map(AgentSessionValidationRole);
        self
    }

    pub fn with_models(mut self, expected: Option<String>, actual: Option<String>) -> Self {
        self.expected_model = expected.map(AgentSessionValidationModel);
        self.actual_model = actual.map(AgentSessionValidationModel);
        self
    }

    pub fn with_reasoning_efforts(
        mut self,
        expected: Option<String>,
        actual: Option<String>,
    ) -> Self {
        self.expected_reasoning_effort = expected.map(AgentSessionValidationReasoningEffort);
        self.actual_reasoning_effort = actual.map(AgentSessionValidationReasoningEffort);
        self
    }

    pub fn with_sandboxes(mut self, expected: Option<String>, actual: Option<String>) -> Self {
        self.expected_sandbox = expected.map(AgentSessionValidationSandbox);
        self.actual_sandbox = actual.map(AgentSessionValidationSandbox);
        self
    }

    pub fn status(&self) -> &AgentSessionValidationStatus {
        &self.status
    }

    pub fn reason(&self) -> &str {
        &self.reason.0
    }

    pub fn expected_model(&self) -> Option<&str> {
        self.expected_model.as_ref().map(|value| value.0.as_str())
    }

    pub fn actual_model(&self) -> Option<&str> {
        self.actual_model.as_ref().map(|value| value.0.as_str())
    }

    pub fn config_path(&self) -> Option<&std::path::Path> {
        self.config_path.as_deref()
    }

    pub fn rollout_path(&self) -> Option<&std::path::Path> {
        self.rollout_path.as_deref()
    }

    pub fn expected_root_session_id(&self) -> Option<&RuntimeSessionId> {
        self.expected_root_session_id.as_ref()
    }

    pub fn actual_root_session_id(&self) -> Option<&RuntimeSessionId> {
        self.actual_root_session_id.as_ref()
    }

    pub fn expected_parent_thread_id(&self) -> Option<&RuntimeSessionId> {
        self.expected_parent_thread_id.as_ref()
    }

    pub fn actual_parent_thread_id(&self) -> Option<&RuntimeSessionId> {
        self.actual_parent_thread_id.as_ref()
    }

    pub fn expected_agent_path(&self) -> Option<&str> {
        self.expected_agent_path
            .as_ref()
            .map(|value| value.0.as_str())
    }

    pub fn actual_agent_path(&self) -> Option<&str> {
        self.actual_agent_path
            .as_ref()
            .map(|value| value.0.as_str())
    }

    pub fn expected_role(&self) -> Option<&str> {
        self.expected_role.as_ref().map(|value| value.0.as_str())
    }

    pub fn actual_role(&self) -> Option<&str> {
        self.actual_role.as_ref().map(|value| value.0.as_str())
    }

    pub fn expected_reasoning_effort(&self) -> Option<&str> {
        self.expected_reasoning_effort
            .as_ref()
            .map(|value| value.0.as_str())
    }

    pub fn actual_reasoning_effort(&self) -> Option<&str> {
        self.actual_reasoning_effort
            .as_ref()
            .map(|value| value.0.as_str())
    }

    pub fn expected_sandbox(&self) -> Option<&str> {
        self.expected_sandbox.as_ref().map(|value| value.0.as_str())
    }

    pub fn actual_sandbox(&self) -> Option<&str> {
        self.actual_sandbox.as_ref().map(|value| value.0.as_str())
    }
}
