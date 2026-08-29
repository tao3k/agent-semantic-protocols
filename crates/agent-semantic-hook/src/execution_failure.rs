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
    LauncherProcessExited,
    LauncherTargetUnavailable,
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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub exit_code: Option<i32>,
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
            exit_code: None,
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

    #[must_use]
    pub fn with_exit_code(mut self, exit_code: i32) -> Self {
        self.exit_code = Some(exit_code);
        self
    }
}

/// Render a typed execution failure through the exact Codex event envelope.
///
/// The fixed plugin launcher owns no policy serialization. The single Rust
/// HookGeneration binary renders every event-specific terminal.
pub fn render_codex_execution_failure(failure: &HookExecutionFailure) -> serde_json::Value {
    let message = failure.message.as_str();
    match failure.event.as_deref() {
        Some("pre-tool") => {
            let receipt = serde_json::to_value(failure).unwrap_or_else(|_| {
                serde_json::json!({
                    "schemaId": "agent.semantic-protocols.hook.execution-failure",
                    "schemaVersion": 1,
                    "state": "failed",
                    "phase": "decision-emission",
                    "failureKind": "runtime-error",
                    "runtimeArtifactFingerprint": "unavailable",
                    "message": "failed to serialize Hook execution failure",
                })
            });
            crate::render_codex_pre_tool_deny(&receipt, message)
        }
        Some("permission-request") => crate::render_codex_permission_request("deny", Some(message)),
        _ => serde_json::json!({}),
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
