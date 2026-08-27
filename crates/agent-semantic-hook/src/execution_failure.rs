//! Typed terminal evidence for Hook failures that happen before a decision can be emitted.

use serde::Serialize;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum HookExecutionPhase {
    Bootstrap,
    Classification,
    DecisionEmission,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum HookExecutionFailureKind {
    InvalidInvocation,
    RuntimeError,
    RuntimePanic,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HookExecutionFailure {
    pub schema_id: &'static str,
    pub schema_version: u32,
    pub state: &'static str,
    pub phase: HookExecutionPhase,
    pub failure_kind: HookExecutionFailureKind,
    pub event: Option<String>,
    pub client: Option<String>,
    pub runtime_artifact_fingerprint: String,
    pub executable_path: Option<String>,
    pub launcher_binary_path: Option<String>,
    pub launcher_path: Option<String>,
    pub launcher_state_home: Option<String>,
    pub plugin_payload_version: Option<String>,
    pub message: String,
}

impl HookExecutionFailure {
    pub fn new(
        phase: HookExecutionPhase,
        failure_kind: HookExecutionFailureKind,
        event: Option<String>,
        client: Option<String>,
        message: impl Into<String>,
    ) -> Self {
        let launcher_path = std::env::var("ASP_HOOK_LAUNCHER_PATH").ok();
        let plugin_payload_version = launcher_path.as_ref().and_then(|path| {
            std::path::Path::new(path)
                .parent()
                .and_then(std::path::Path::parent)
                .and_then(std::path::Path::file_name)
                .and_then(std::ffi::OsStr::to_str)
                .map(str::to_owned)
        });
        Self {
            schema_id: "agent.semantic-protocols.hook.execution-failure",
            schema_version: 1,
            state: "failed",
            phase,
            failure_kind,
            event,
            client,
            runtime_artifact_fingerprint: crate::hook_runtime_artifact_fingerprint(),
            executable_path: std::env::current_exe()
                .ok()
                .map(|path| path.display().to_string()),
            launcher_binary_path: std::env::var("ASP_HOOK_LAUNCHER_BINARY").ok(),
            launcher_path,
            launcher_state_home: std::env::var("ASP_HOOK_LAUNCHER_STATE_HOME").ok(),
            plugin_payload_version,
            message: message.into(),
        }
    }
}

impl std::fmt::Display for HookExecutionFailure {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match serde_json::to_string(self) {
            Ok(receipt) => formatter.write_str(&receipt),
            Err(_) => formatter.write_str(
                r#"{"schemaId":"agent.semantic-protocols.hook.execution-failure","schemaVersion":1,"state":"failed","failureKind":"receipt-serialization-failed"}"#,
            ),
        }
    }
}

#[cfg(test)]
#[path = "../tests/unit/execution_failure.rs"]
mod tests;
