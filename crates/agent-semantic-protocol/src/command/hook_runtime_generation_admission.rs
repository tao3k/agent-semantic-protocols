use std::path::Path;

const WORKSPACE_ADMISSION_EVENTS: [&str; 2] = ["session-start", "user-prompt"];

#[derive(Debug, Default)]
pub(super) struct HookGenerationAdmissionObservation {
    pub(super) receipt: Option<serde_json::Value>,
    pub(super) error: Option<String>,
}

pub(crate) fn request(project_root: &Path) -> Result<serde_json::Value, String> {
    let project_root = project_root.to_path_buf();
    crate::command::runtime_server::block_on_runtime_server_client(async move {
        let session =
            crate::command::runtime_server::runtime_server_workspace_session_for_admission_async(
                &project_root,
            )
            .await?;
        let receipt = session.admit_runtime_generation().await?;
        receipt.validate()?;
        serde_json::to_value(receipt)
            .map_err(|error| format!("failed to encode runtime generation admission: {error}"))
    })?
}

pub(super) fn hook_event_requires_generation_admission(
    args: &[String],
    workspace_mutated: bool,
    explicit_asp_workspace: bool,
) -> bool {
    args.iter().any(|argument| {
        WORKSPACE_ADMISSION_EVENTS.contains(&argument.as_str())
            || (argument == "post-tool" && workspace_mutated)
            || (argument == "pre-tool" && explicit_asp_workspace)
    })
}

pub(super) fn decision_mutates_workspace(decision: &agent_semantic_hook::HookDecision) -> bool {
    decision
        .fields
        .get("operationIntent")
        .and_then(serde_json::Value::as_str)
        == Some("apply-patch")
        || decision
            .fields
            .get("normalizedActions")
            .and_then(serde_json::Value::as_array)
            .is_some_and(|actions| {
                actions.iter().any(|action| {
                    action
                        .get("operationIntent")
                        .and_then(serde_json::Value::as_str)
                        == Some("apply-patch")
                })
            })
}

pub(super) fn observe(
    args: &[String],
    workspace_mutated: bool,
    explicit_asp_workspace: bool,
    request: impl FnOnce() -> Result<serde_json::Value, String>,
) -> HookGenerationAdmissionObservation {
    if !hook_event_requires_generation_admission(args, workspace_mutated, explicit_asp_workspace) {
        return HookGenerationAdmissionObservation::default();
    }

    match request() {
        Ok(receipt) => HookGenerationAdmissionObservation {
            receipt: Some(receipt),
            error: None,
        },
        Err(error) => HookGenerationAdmissionObservation {
            receipt: None,
            error: Some(error),
        },
    }
}

#[cfg(test)]
#[path = "../../tests/unit/hook_runtime_generation_admission.rs"]
mod hook_runtime_generation_admission_tests;
